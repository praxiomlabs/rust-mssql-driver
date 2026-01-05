# SQL Server Compatibility Matrix

This document provides compatibility information for the rust-mssql-driver across different SQL Server versions.

## Tested SQL Server Versions

| SQL Server | Version String | TDS Protocol | TLS Support | Status |
|------------|----------------|--------------|-------------|--------|
| 2008 | 10.x | TDS 7.3A | No (use `no_tls`) | Supported |
| 2008 R2 | 10.50.x | TDS 7.3B | No (use `no_tls`) | Supported |
| 2012 SP4 | 11.0.7001.0 | TDS 7.4 | Limited | Tested |
| 2014 RTM | 12.0.2000.8 | TDS 7.4 | Limited | Tested |
| 2016 SP3 | 13.0.6404.1 | TDS 7.4 | Limited | Tested |
| 2017 | 14.x | TDS 7.4 | Yes | Supported |
| 2019 | 15.x | TDS 7.4 | Yes | Supported |
| 2022 CU22 | 16.0.4225.2 | TDS 7.4/8.0 | Yes | Tested |

## Connection Configuration by Version

### SQL Server 2008-2016 (Legacy)

These versions require special configuration due to TLS limitations:

```rust
// Connection string
let conn_str = "Server=host,1433;Database=mydb;User Id=sa;Password=pwd;Encrypt=no_tls";

// Or programmatically
let config = Config::new("host", "mydb", "sa", "pwd")
    .port(1433)
    .no_tls();
```

> **Important:** Legacy SQL Server versions (pre-2017) often don't support TLS 1.2,
> which is required by rustls. Use `Encrypt=no_tls` for these servers.

### SQL Server 2017+

Modern versions support TLS natively:

```rust
// Standard TLS connection
let conn_str = "Server=host,1433;Database=mydb;User Id=sa;Password=pwd;Encrypt=true;TrustServerCertificate=true";

// Or programmatically
let config = Config::new("host", "mydb", "sa", "pwd")
    .port(1433)
    .encrypt(true)
    .trust_server_certificate(true);
```

### SQL Server 2022+ (TDS 8.0)

SQL Server 2022 supports TDS 8.0 with strict encryption mode:

```rust
// TDS 8.0 strict mode
let config = Config::new("host", "mydb", "sa", "pwd")
    .port(1433)
    .tds_version(TdsVersion::V8_0)
    .strict_mode(true);
```

## Feature Availability by Version

| Feature | 2008 | 2012 | 2014 | 2016 | 2017 | 2019 | 2022 |
|---------|------|------|------|------|------|------|------|
| Basic queries | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Parameterized queries | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Transactions | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| DATE/TIME types | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| DATETIME2/DATETIMEOFFSET | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Table-Valued Parameters | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| STRING_AGG | No | No | No | No | Yes | Yes | Yes |
| APPROX_COUNT_DISTINCT | No | No | No | No | No | Yes | Yes |
| TLS/SSL encryption | No* | No* | No* | No* | Yes | Yes | Yes |
| TDS 8.0 strict mode | No | No | No | No | No | No | Yes |

\* These versions may support TLS with older TLS versions (1.0/1.1), but rustls requires TLS 1.2+.
  Use `Encrypt=no_tls` for these servers.

## SQL Server Version Detection

The driver reports SQL Server versions through the PreLogin response:

| Major Version | SQL Server Product |
|---------------|-------------------|
| 10 | SQL Server 2008 |
| 10.50 | SQL Server 2008 R2 |
| 11 | SQL Server 2012 |
| 12 | SQL Server 2014 |
| 13 | SQL Server 2016 |
| 14 | SQL Server 2017 |
| 15 | SQL Server 2019 |
| 16 | SQL Server 2022 |

## Known Issues

### ProductMajorVersion NULL in SQL Server 2014

`SERVERPROPERTY('ProductMajorVersion')` returns NULL in SQL Server 2014 RTM.
The driver parses version from `SERVERPROPERTY('ProductVersion')` instead.

### TLS on Legacy Servers

SQL Server 2008-2016 typically use TLS 1.0 or 1.1, which rustls does not support.
Connection attempts with `Encrypt=true` will fail with "TLS handshake eof".

**Solution:** Use `Encrypt=no_tls` for these servers:

```rust
let conn_str = "Server=host,1433;...;Encrypt=no_tls";
```

> **Security Note:** Disabling TLS means data travels unencrypted. Only use this
> option on trusted networks or for legacy system compatibility.

## Test Environment

To test against different SQL Server versions, set these environment variables:

```bash
# For SQL Server on non-default port
MSSQL_HOST=10.0.20.79
MSSQL_PORT=1434
MSSQL_USER=sa
MSSQL_PASSWORD=MySecurePassword123
MSSQL_ENCRYPT=no_tls  # For legacy servers

# Run tests
cargo test -p mssql-client --test version_compatibility -- --ignored
```

## Azure SQL Database

Azure SQL Database uses TLS by default and supports modern TLS versions:

```rust
let conn_str = "Server=myserver.database.windows.net;Database=mydb;User Id=user;Password=pwd;Encrypt=true";
```

Azure SQL also supports redirect handling for high availability - the driver handles
this automatically via the `ENVCHANGE` routing token.
