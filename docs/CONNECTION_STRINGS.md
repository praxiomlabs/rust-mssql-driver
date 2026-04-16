# Connection String Reference

This document provides a complete reference for connection string format and supported keywords.

## Overview

rust-mssql-driver uses ADO.NET-compatible connection strings for configuration. This ensures familiarity for developers coming from .NET and compatibility with many existing tools.

## Basic Format

```
Server=hostname;Database=dbname;User Id=username;Password=password;
```

**Rules:**
- Key-value pairs separated by semicolons (`;`)
- Keys and values separated by equals (`=`)
- Keys are case-insensitive
- Whitespace around keys and values is trimmed
- Unknown keys are ignored (logged at debug level)
- Known ADO.NET keywords we don't support are logged at info level
- Pool-related keywords (Max Pool Size, etc.) are logged with guidance to use PoolConfig

### Quoted Values

Values containing semicolons must be enclosed in double or single quotes per the ADO.NET specification:

```
Password="my;complex;password";
Password='my;complex;password';
```

Doubled quotes inside are escapes: `""` → `"`, `''` → `'`:

```
Password="has ""quotes""";
Password='it''s complex';
```

## Complete Keyword Reference

### Server Configuration

| Keyword | Aliases | Type | Default | Description |
|---------|---------|------|---------|-------------|
| `Server` | `Data Source`, `Addr`, `Address`, `Network Address`, `Host` | String | `localhost` | SQL Server hostname or IP |
| `Port` | — | Number | `1433` | TCP port number |

#### Server Format Options

**Simple hostname:**
```
Server=db.example.com
```

**Hostname with port (comma-separated):**
```
Server=db.example.com,1434
```

**Hostname with named instance (backslash-separated):**
```
Server=db.example.com\SQLEXPRESS
```

**Azure Portal format (tcp: prefix automatically stripped):**
```
Server=tcp:yourserver.database.windows.net,1433
```

**Local host aliases:**
```
Server=.\SQLEXPRESS
Server=(local)\SQLEXPRESS
Server=localhost\SQLEXPRESS
```

Both `.` and `(local)` are normalized to `127.0.0.1`, matching ADO.NET behavior.

**Unsupported protocols (return an error with guidance):**
```
Server=np:\\host\pipe\sql\query   ← Named Pipes not supported
Server=lpc:host                    ← Shared Memory not supported
```

**Azure SQL Database:**
```
Server=yourserver.database.windows.net
```

### Authentication

| Keyword | Aliases | Type | Default | Description |
|---------|---------|------|---------|-------------|
| `User Id` | `UID`, `User` | String | (empty) | SQL Server login username |
| `Password` | `PWD` | String | (empty) | SQL Server login password |

**SQL Server Authentication:**
```
User Id=app_user;Password=YourStrongPassword!;
```

### Database

| Keyword | Aliases | Type | Default | Description |
|---------|---------|------|---------|-------------|
| `Database` | `Initial Catalog` | String | `None` | Target database name |

```
Database=production;
```

### Security

| Keyword | Aliases | Type | Default | Description |
|---------|---------|------|---------|-------------|
| `Encrypt` | — | String | `false` | Encryption mode (`true`, `false`, `strict`) |
| `TrustServerCertificate` | `Trust Server Certificate` | Boolean | `false` | Skip certificate validation |

**Production (recommended):**
```
Encrypt=strict;TrustServerCertificate=false;
```

**Development only:**
```
Encrypt=true;TrustServerCertificate=true;
```

#### Encryption Modes

| Value | Behavior |
|-------|----------|
| `false` | No encryption (not recommended) |
| `true` | TLS encryption with certificate validation |
| `strict` | TDS 8.0 strict mode (SQL Server 2022+) |

### Timeouts

| Keyword | Aliases | Type | Default | Description |
|---------|---------|------|---------|-------------|
| `Connect Timeout` | `Connection Timeout`, `Timeout` | Seconds | `15` | TCP connection timeout |
| `Command Timeout` | — | Seconds | `30` | Query execution timeout |

```
Connect Timeout=30;Command Timeout=60;
```

**Value `0` means no timeout** (not recommended for production).

### Application Identification

| Keyword | Aliases | Type | Default | Description |
|---------|---------|------|---------|-------------|
| `Application Name` | `App` | String | `mssql-client` | Application identifier |
| `ApplicationIntent` | `Application Intent` | String | `ReadWrite` | AlwaysOn AG routing (`ReadOnly` or `ReadWrite`) |
| `Workstation ID` | `WSID` | String | (machine hostname) | Client workstation name for audit trails |
| `Current Language` | `Language` | String | (server default) | Session language for server messages |

```
Application Name=MyApp-v1.2.3;ApplicationIntent=ReadOnly;
```

`Application Name` appears in SQL Server's `sys.dm_exec_sessions` for monitoring.

`ApplicationIntent=ReadOnly` routes the connection to a readable secondary in AlwaysOn Availability Group configurations.

`Workstation ID` is sent in the LOGIN7 HostName field and appears in `sys.dm_exec_sessions.host_name`. When not specified, the driver sends the machine hostname automatically.

### Connection Resiliency

| Keyword | Aliases | Type | Default | Description |
|---------|---------|------|---------|-------------|
| `ConnectRetryCount` | `Connect Retry Count` | Number | `3` | Number of reconnect attempts on idle connection failure |
| `ConnectRetryInterval` | `Connect Retry Interval` | Seconds | `0` | Seconds between reconnect attempts |
| `MultiSubnetFailover` | `Multi Subnet Failover` | Boolean | `false` | Race parallel TCP connections to all resolved IPs |

`ConnectRetryCount`/`ConnectRetryInterval` wire to the driver's `RetryPolicy.max_retries` and `RetryPolicy.initial_backoff` respectively.

`MultiSubnetFailover=True` resolves the server hostname to all IP addresses and attempts parallel TCP connections simultaneously. The first successful connection wins and all others are cancelled. Use this when connecting to AlwaysOn AG listeners that span multiple subnets.

### Advanced Options

| Keyword | Aliases | Type | Default | Description |
|---------|---------|------|---------|-------------|
| `MultipleActiveResultSets` | `MARS` | Boolean | `false` | Enable MARS (not fully supported) |
| `Packet Size` | — | Number | `4096` | TDS packet size in bytes |

### Recognized but Not Supported

The following ADO.NET keywords are recognized (logged at info level) but not processed by this driver:

| Keyword | Guidance |
|---------|----------|
| `Max Pool Size`, `Min Pool Size`, `Pooling`, `Connection Lifetime`, `Load Balance Timeout` | Use `PoolConfig` instead of connection string |
| `Failover Partner` | Database mirroring failover not implemented |
| `Persist Security Info` | Password is never returned in connection strings |
| `Network Library`, `Enlist`, `Replication`, `Transaction Binding`, `Type System Version`, `User Instance`, `AttachDbFilename`, `Context Connection`, `Asynchronous Processing` | .NET-specific features not applicable |

## Boolean Values

Boolean keywords accept:

| True Values | False Values |
|-------------|--------------|
| `true` | `false` |
| `yes` | `no` |
| `1` | `0` |
| | (empty string) |

Case-insensitive.

## Complete Examples

### Local Development

```
Server=localhost;Database=devdb;User Id=sa;Password=DevPassword123!;TrustServerCertificate=true;
```

### On-Premises Production

```
Server=sql-prod.internal.company.com,1433;Database=production;User Id=app_user;Password=StrongP@ssw0rd!;Encrypt=true;TrustServerCertificate=false;Connect Timeout=30;Command Timeout=60;Application Name=OrderService-v2.1.0;
```

### Azure SQL Database

```
Server=yourserver.database.windows.net;Database=yourdb;User Id=app_user@yourserver;Password=YourPassword!;Encrypt=strict;TrustServerCertificate=false;Connect Timeout=60;
```

### SQL Server Named Instance

```
Server=localhost\SQLEXPRESS;Database=localdb;User Id=sa;Password=Express123!;TrustServerCertificate=true;
```

### High-Security Environment

```
Server=secure-db.internal;Database=sensitive;User Id=restricted_user;Password=VeryStr0ng!;Encrypt=strict;TrustServerCertificate=false;Connect Timeout=15;Command Timeout=30;Application Name=SecureApp;
```

## Programmatic Usage

### From Connection String

```rust
use mssql_client::{Client, Config};

let config = Config::from_connection_string(
    "Server=localhost;Database=mydb;User Id=sa;Password=Password123!"
)?;

let mut client = Client::connect(config).await?;
```

### Builder Pattern (Alternative)

```rust
use mssql_client::Config;
use std::time::Duration;

let config = Config::builder()
    .host("localhost")
    .port(1433)
    .database("mydb")
    .username("sa")
    .password("Password123!")
    .connect_timeout(Duration::from_secs(30))
    .trust_server_certificate(true)  // Dev only!
    .build()?;
```

### From Environment Variable

```rust
use std::env;

let conn_str = env::var("DATABASE_URL")
    .expect("DATABASE_URL must be set");

let config = Config::from_connection_string(&conn_str)?;
```

## Common Mistakes

### Missing Port with Non-Standard Port

**Wrong:**
```
Server=db.example.com;Port=1434;
```

**Correct (using comma syntax):**
```
Server=db.example.com,1434;
```

### Special Characters in Password

**Problem:** Password contains `;` or `=`

**Solution:** Currently, special characters in passwords may cause parsing issues. Use the builder API for passwords with special characters:

```rust
let config = Config::builder()
    .host("localhost")
    .password("Pass;word=123!")  // Contains special chars
    .build()?;
```

### Azure SQL User Format

**Wrong (may work, but not recommended):**
```
User Id=myuser;
```

**Correct for Azure SQL:**
```
User Id=myuser@yourserver;
```

### Forgetting Encryption for Production

**Insecure:**
```
Server=production-db;User Id=app;Password=secret;
```

**Secure:**
```
Server=production-db;User Id=app;Password=secret;Encrypt=true;TrustServerCertificate=false;
```

## Comparison with ADO.NET

| Feature | rust-mssql-driver | ADO.NET |
|---------|-------------------|---------|
| Basic keywords | ✅ | ✅ |
| Integrated Security | ❌ | ✅ |
| AttachDbFilename | ❌ | ✅ |
| Pooling keywords | ❌ (use Pool config) | ✅ |
| Failover Partner | ❌ | ✅ |
| Encrypt=strict | ✅ | ✅ (SQL Server 2022+) |

## Comparison with Tiberius

| Feature | rust-mssql-driver | Tiberius |
|---------|-------------------|----------|
| ADO.NET format | ✅ | ✅ |
| Comma port syntax | ✅ | ✅ |
| Named instances | ✅ | ✅ |
| JDBC format | ❌ | ❌ |
| Builder alternative | ✅ | ✅ |

## Debugging Connection Strings

Enable debug logging to see parsed connection string details:

```rust
// Set log level
std::env::set_var("RUST_LOG", "mssql_client=debug");

// Unknown keywords are logged at debug level
// Connection string parsing details are logged
```

## Security Considerations

1. **Never log connection strings** - They contain credentials
2. **Use environment variables** - Don't hardcode in source
3. **Prefer Encrypt=strict** - For SQL Server 2022+
4. **Never use TrustServerCertificate=true in production**
5. **Use strong passwords** - Follow SQL Server password policy

```rust
// Safe: Load from environment
let conn_str = env::var("DATABASE_URL")?;

// Unsafe: Hardcoded credentials
let conn_str = "Server=prod;Password=hardcoded!";  // DON'T DO THIS
```
