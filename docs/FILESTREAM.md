# FILESTREAM BLOB Access

SQL Server FILESTREAM stores `VARBINARY(MAX)` data directly on the NTFS filesystem while maintaining transactional consistency with the database. This driver provides async read/write access to FILESTREAM BLOBs via the Win32 `OpenSqlFilestream` API.

## Requirements

- **Windows client machine** — FILESTREAM uses Win32 file handles
- **SQL Server with FILESTREAM enabled** (access level 2)
- **Windows Authentication** — FILESTREAM requires integrated auth (not SQL auth)
- **Microsoft OLE DB Driver for SQL Server** — `msoledbsql19.dll` or `msoledbsql.dll` ([free download](https://learn.microsoft.com/en-us/sql/connect/oledb/download-oledb-driver-for-sql-server))
- **Active transaction** — FILESTREAM handles are bound to a SQL transaction

## Quick Start

```toml
[dependencies]
mssql-client = { version = "0.8", features = ["sspi-auth", "filestream"] }
```

```rust
use mssql_client::{Client, Config, FileStreamAccess};
use tokio::io::AsyncReadExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_connection_string(
        "Server=localhost;Database=MyDb;Integrated Security=true;TrustServerCertificate=true"
    )?;

    let client = Client::connect(config).await?;
    let mut tx = client.begin_transaction().await?;

    // Step 1: Get the FILESTREAM path for the BLOB
    let rows = tx.query(
        "SELECT Content.PathName() FROM dbo.Documents WHERE Id = @p1",
        &[&doc_id],
    ).await?;
    let path: String = rows.into_iter().next().unwrap()?.get(0)?;

    // Step 2: Open and read the BLOB
    let mut stream = tx.open_filestream(&path, FileStreamAccess::Read).await?;
    let mut data = Vec::new();
    stream.read_to_end(&mut data).await?;

    // Step 3: Drop the stream before committing
    drop(stream);
    tx.commit().await?;

    Ok(())
}
```

## Writing FILESTREAM Data

```rust
use mssql_client::FileStreamAccess;
use tokio::io::AsyncWriteExt;

let mut tx = client.begin_transaction().await?;

// Insert a row with an empty FILESTREAM placeholder
tx.execute(
    "INSERT INTO dbo.Documents (Name, Content) VALUES (@p1, CAST('' AS VARBINARY(MAX)))",
    &[&"my_file.bin"],
).await?;

// Get the path for the new row
let rows = tx.query(
    "SELECT Content.PathName() FROM dbo.Documents WHERE Name = @p1",
    &[&"my_file.bin"],
).await?;
let path: String = rows.into_iter().next().unwrap()?.get(0)?;

// Write data to the FILESTREAM BLOB
let mut stream = tx.open_filestream(&path, FileStreamAccess::Write).await?;
stream.write_all(b"binary content here").await?;
stream.shutdown().await?;
drop(stream);

tx.commit().await?;
```

## SQL Server Setup

### 1. Enable FILESTREAM via SQL Server Configuration Manager

Open **SQL Server Configuration Manager** (search Start menu for `SQLServerManager*.msc`):

1. Click **SQL Server Services** in the left pane
2. Right-click your SQL Server instance -> **Properties**
3. Go to the **FILESTREAM** tab
4. Check **Enable FILESTREAM for Transact-SQL access**
5. Check **Enable FILESTREAM for file I/O streaming access**
6. Click OK and restart the SQL Server service when prompted

### 2. Enable FILESTREAM at the SQL Server level

```sql
EXEC sp_configure 'filestream access level', 2;
RECONFIGURE;

-- Verify: run_value should be 2
EXEC sp_configure 'filestream access level';
```

### 3. Create a database with a FILESTREAM filegroup

```sql
CREATE DATABASE MyFilestreamDb
ON PRIMARY (
    NAME = MyDb_data,
    FILENAME = 'C:\SQLData\MyDb.mdf'
),
FILEGROUP FStreamFG CONTAINS FILESTREAM (
    NAME = MyDb_fs,
    FILENAME = 'C:\SQLData\MyDb_fs'
)
LOG ON (
    NAME = MyDb_log,
    FILENAME = 'C:\SQLData\MyDb_log.ldf'
);
```

### 4. Create a table with a FILESTREAM column

```sql
USE MyFilestreamDb;

CREATE TABLE dbo.Documents (
    Id UNIQUEIDENTIFIER ROWGUIDCOL NOT NULL DEFAULT NEWID() PRIMARY KEY,
    Name NVARCHAR(256),
    Content VARBINARY(MAX) FILESTREAM NULL
);
```

Note: A `UNIQUEIDENTIFIER ROWGUIDCOL` column is required for FILESTREAM tables.

## API Reference

### `Client<InTransaction>::open_filestream(path, access)`

Convenience method that automatically fetches the transaction context and opens the FILESTREAM handle.

- `path` — UNC path from `column.PathName()` (e.g., `\\server\instance\...\guid`)
- `access` — `FileStreamAccess::Read`, `Write`, or `ReadWrite`
- Returns `FileStream` which implements `AsyncRead + AsyncWrite`

### `FileStream::open(path, access, txn_context)`

Low-level API for when you need to manage the transaction context yourself.

### `FileStream::open_with_options(path, access, txn_context, options)`

Low-level API with custom Win32 open flags. See `mssql_client::filestream_options` for available flags.

## Limitations

- **Windows only** — the `filestream` feature is gated behind `#[cfg(windows)]`
- **Requires Windows Authentication** — SQL Server does not support FILESTREAM access with SQL authentication
- **Requires OLE DB Driver** — `msoledbsql19.dll` or `msoledbsql.dll` must be installed on the client machine. The driver provides a clear error message if neither is found.
- **Transaction-scoped** — the `FileStream` handle must be dropped before calling `commit()` or `rollback()` on the transaction
- **Async I/O via blocking pool** — the current implementation wraps the Win32 handle in `tokio::fs::File`, which uses `spawn_blocking` for I/O operations. This works correctly but dispatches each operation to tokio's blocking thread pool. A future optimization could use IOCP for true completion-based async I/O (see `filestream_options::ASYNC`).

## Troubleshooting

### "FILESTREAM driver not found"

The Microsoft OLE DB Driver for SQL Server is not installed. Download it from:
https://learn.microsoft.com/en-us/sql/connect/oledb/download-oledb-driver-for-sql-server

### "OpenSqlFilestream failed: Access is denied"

- Ensure you're using Windows Authentication (not SQL auth)
- Ensure the Windows user has access to the FILESTREAM file share
- Ensure FILESTREAM access level is 2 (not just 1): `EXEC sp_configure 'filestream access level'`

### "OpenSqlFilestream failed: The system cannot find the path specified"

- Verify the path from `column.PathName()` is not NULL
- Ensure FILESTREAM is enabled and the database has a FILESTREAM filegroup
- Check that the SQL Server Browser service is running if using named instances
