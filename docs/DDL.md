# DDL Execution Guide

This driver handles DDL (Data Definition Language) statements correctly out of the box.

## How It Works

The driver automatically routes SQL to the correct TDS mechanism based on whether parameters are present:

- **Zero parameters** → SQL batch (raw SQL, supports DDL)
- **With parameters** → `sp_executesql` via RPC (parameterized, does not support DDL)

This means DDL works naturally when you pass an empty parameter slice:

```rust
// CREATE TABLE — just pass no parameters
client.execute("CREATE TABLE dbo.Users (id INT PRIMARY KEY, name NVARCHAR(100))", &[]).await?;

// ALTER TABLE
client.execute("ALTER TABLE dbo.Users ADD email NVARCHAR(255)", &[]).await?;

// DROP TABLE
client.execute("DROP TABLE IF EXISTS dbo.TempData", &[]).await?;
```

## `simple_query` for Fire-and-Forget DDL

If you don't need the affected row count, use `simple_query`:

```rust
client.simple_query("CREATE INDEX IX_Users_Email ON dbo.Users (email)").await?;
```

`simple_query` sends a SQL batch and discards the response. It is available on `Client<Ready>`.

## Multi-Statement Batches

You can send multiple statements separated by semicolons:

```rust
client.simple_query("
    CREATE TABLE dbo.Orders (id INT PRIMARY KEY);
    CREATE TABLE dbo.OrderItems (id INT, order_id INT);
    ALTER TABLE dbo.OrderItems ADD CONSTRAINT FK_Order
        FOREIGN KEY (order_id) REFERENCES dbo.Orders(id);
").await?;
```

## Why DDL Fails With Parameters

This is a TDS protocol limitation, not specific to this driver. When parameters are present, the SQL is sent via `sp_executesql`, which wraps the SQL in a stored procedure context. SQL Server restricts certain DDL operations inside `sp_executesql`.

If you accidentally pass parameters with DDL:

```rust
// This MAY fail or behave unexpectedly
client.execute("CREATE TABLE dbo.Test (id INT)", &[&1i32]).await?;
```

The solution is always to pass an empty parameter slice `&[]`.

## Comparison With Tiberius

In Tiberius, users must know to call `simple_query()` for DDL — the regular `query()` method sends everything via RPC, causing silent failures. This driver's automatic routing eliminates that footgun: `execute("CREATE TABLE ...", &[])` just works.
