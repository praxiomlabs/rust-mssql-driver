# Production Configuration Examples

This document provides ready-to-use configuration examples for common production scenarios.

---

## Azure SQL Database

### Standard Configuration

```rust
use mssql_client::{Client, Config};
use mssql_driver_pool::{Pool, PoolConfig};
use std::time::Duration;

async fn azure_sql_config() -> Result<Pool, Error> {
    // Azure SQL Database connection string
    let config = Config::from_connection_string(
        "Server=your-server.database.windows.net,1433;\
         Database=your-database;\
         User Id=your-user@your-server;\
         Password=YourStrongPassword!;\
         Encrypt=strict;\
         TrustServerCertificate=false;\
         Connect Timeout=30;\
         Application Name=MyApp-v1.0.0"
    )?;

    // Production pool settings for Azure SQL
    let pool = Pool::builder()
        .min_size(2)
        .max_size(30)                              // Azure SQL default max is 100
        .acquire_timeout(Duration::from_secs(30))
        .idle_timeout(Duration::from_secs(300))   // Azure closes idle after 30 min
        .max_lifetime(Duration::from_secs(1800))  // Recycle before Azure timeout
        .test_on_borrow(true)
        .build(config)
        .await?;

    Ok(pool)
}
```

### Azure SQL with Managed Identity

```rust
// Requires the `azure-identity` feature:
// mssql-auth = { version = "0.5", features = ["azure-identity"] }

use azure_identity::DefaultAzureCredential;
use mssql_auth::AzureIdentityAuth;

async fn azure_sql_managed_identity() -> Result<Pool, Error> {
    // Use Azure Default Credential (Managed Identity, CLI, etc.)
    let credential = DefaultAzureCredential::default();

    let config = Config::new()
        .host("your-server.database.windows.net")
        .database("your-database")
        .authentication(AzureIdentityAuth::new(credential))
        .encryption(Encryption::Strict);

    Pool::builder()
        .max_size(30)
        .build(config)
        .await
}
```

### Azure SQL Serverless

```rust
async fn azure_sql_serverless() -> Result<Pool, Error> {
    // Azure SQL Serverless may have cold start delays
    let config = Config::from_connection_string(
        "Server=your-server.database.windows.net;\
         Database=your-serverless-db;\
         User Id=your-user;\
         Password=YourPassword!;\
         Encrypt=strict;\
         Connect Timeout=60"  // Longer timeout for cold start
    )?;

    // Smaller pool for serverless (auto-pause consideration)
    let pool = Pool::builder()
        .min_size(0)                              // Allow full scale-down
        .max_size(10)
        .acquire_timeout(Duration::from_secs(60)) // Cold start can be slow
        .idle_timeout(Duration::from_secs(60))    // Quick release for serverless
        .test_on_borrow(true)
        .build(config)
        .await?;

    Ok(pool)
}
```

---

## On-Premises SQL Server

### Standard Configuration

```rust
async fn onprem_sql_config() -> Result<Pool, Error> {
    let config = Config::from_connection_string(
        "Server=sql-server.internal.company.com,1433;\
         Database=production;\
         User Id=app_user;\
         Password=YourStrongPassword!;\
         Encrypt=true;\
         TrustServerCertificate=false;\
         Connect Timeout=15;\
         Command Timeout=60;\
         Application Name=MyApp-v1.0.0"
    )?;

    let pool = Pool::builder()
        .min_size(5)
        .max_size(50)
        .acquire_timeout(Duration::from_secs(15))
        .idle_timeout(Duration::from_secs(600))
        .max_lifetime(Duration::from_secs(3600))
        .test_on_borrow(true)
        .build(config)
        .await?;

    Ok(pool)
}
```

### SQL Server with TDS 8.0 Strict Mode

```rust
async fn sql_2022_strict_mode() -> Result<Pool, Error> {
    // SQL Server 2022+ with strict TLS encryption
    let config = Config::from_connection_string(
        "Server=sql2022.internal.company.com,1433;\
         Database=production;\
         User Id=app_user;\
         Password=YourStrongPassword!;\
         Encrypt=strict;\
         TrustServerCertificate=false;\
         Connect Timeout=15;\
         Application Name=MyApp-v1.0.0"
    )?;

    Pool::builder()
        .max_size(50)
        .build(config)
        .await
}
```

### Internal Development Server

```rust
async fn dev_sql_config() -> Result<Pool, Error> {
    // Development only - relaxed security settings
    let config = Config::from_connection_string(
        "Server=localhost,1433;\
         Database=devdb;\
         User Id=sa;\
         Password=DevPassword123!;\
         Encrypt=true;\
         TrustServerCertificate=true;\
         Connect Timeout=5;\
         Application Name=MyApp-dev"
    )?;

    // Smaller pool for development
    let pool = Pool::builder()
        .min_size(1)
        .max_size(5)
        .build(config)
        .await?;

    Ok(pool)
}
```

---

## High Availability Configurations

### Always On Availability Group

```rust
async fn always_on_config() -> Result<Pool, Error> {
    // Connect to AG listener for automatic failover
    let config = Config::from_connection_string(
        "Server=ag-listener.company.com,1433;\
         Database=production;\
         User Id=app_user;\
         Password=YourStrongPassword!;\
         Encrypt=true;\
         TrustServerCertificate=false;\
         Connect Timeout=30;\
         MultiSubnetFailover=true;\
         Application Name=MyApp-v1.0.0"
    )?;

    // Larger pool with faster recycling for HA
    let pool = Pool::builder()
        .min_size(5)
        .max_size(50)
        .max_lifetime(Duration::from_secs(1800))  // More frequent recycling
        .test_on_borrow(true)                     // Always verify after failover
        .build(config)
        .await?;

    Ok(pool)
}
```

### Read Scale-Out (Read Replicas)

```rust
async fn read_replica_pools() -> Result<(Pool, Pool), Error> {
    // Primary pool for writes
    let primary_config = Config::from_connection_string(
        "Server=ag-listener.company.com;\
         Database=production;\
         User Id=app_user;\
         Password=YourStrongPassword!;\
         Encrypt=true;\
         ApplicationIntent=ReadWrite;\
         Application Name=MyApp-Primary"
    )?;

    let primary_pool = Pool::builder()
        .min_size(2)
        .max_size(20)
        .build(primary_config)
        .await?;

    // Secondary pool for reads
    let secondary_config = Config::from_connection_string(
        "Server=ag-listener.company.com;\
         Database=production;\
         User Id=app_readonly;\
         Password=YourStrongPassword!;\
         Encrypt=true;\
         ApplicationIntent=ReadOnly;\
         Application Name=MyApp-ReadOnly"
    )?;

    let secondary_pool = Pool::builder()
        .min_size(5)
        .max_size(50)  // More capacity for reads
        .build(secondary_config)
        .await?;

    Ok((primary_pool, secondary_pool))
}
```

---

## Microservice Configurations

### Web API Service

```rust
async fn web_api_config() -> Result<Pool, Error> {
    let config = Config::from_connection_string(&std::env::var("DATABASE_URL")?)?;

    // Sized for typical web API workload
    // Assumes 4 CPU cores, ~100 concurrent requests
    let pool = Pool::builder()
        .min_size(4)
        .max_size(20)
        .acquire_timeout(Duration::from_secs(5))  // Fail fast for web requests
        .idle_timeout(Duration::from_secs(300))
        .build(config)
        .await?;

    Ok(pool)
}
```

### Background Job Worker

```rust
async fn worker_config() -> Result<Pool, Error> {
    let config = Config::from_connection_string(&std::env::var("DATABASE_URL")?)?;

    // Background workers typically need fewer connections
    // but longer timeouts for batch operations
    let pool = Pool::builder()
        .min_size(1)
        .max_size(5)
        .acquire_timeout(Duration::from_secs(30))
        .idle_timeout(Duration::from_secs(600))
        .build(config)
        .await?;

    Ok(pool)
}
```

### Batch Processing Service

```rust
async fn batch_config() -> Result<Pool, Error> {
    let config = Config::from_connection_string(
        &format!(
            "{}Command Timeout=300",  // 5 minute query timeout for batch
            std::env::var("DATABASE_URL")?
        )
    )?;

    // Batch processing needs more connections for parallel work
    let pool = Pool::builder()
        .min_size(5)
        .max_size(30)
        .acquire_timeout(Duration::from_secs(60))
        .max_lifetime(Duration::from_secs(7200))  // Longer lifetime for batch
        .build(config)
        .await?;

    Ok(pool)
}
```

---

## Docker / Kubernetes Configuration

### Environment Variable Configuration

```rust
use std::env;

#[derive(Debug)]
struct DbConfig {
    host: String,
    port: u16,
    database: String,
    user: String,
    password: String,
    pool_min: u32,
    pool_max: u32,
    encrypt: bool,
}

impl DbConfig {
    fn from_env() -> Result<Self, env::VarError> {
        Ok(Self {
            host: env::var("DB_HOST")?,
            port: env::var("DB_PORT")
                .unwrap_or_else(|_| "1433".to_string())
                .parse()
                .unwrap_or(1433),
            database: env::var("DB_NAME")?,
            user: env::var("DB_USER")?,
            password: env::var("DB_PASSWORD")?,
            pool_min: env::var("DB_POOL_MIN")
                .unwrap_or_else(|_| "2".to_string())
                .parse()
                .unwrap_or(2),
            pool_max: env::var("DB_POOL_MAX")
                .unwrap_or_else(|_| "20".to_string())
                .parse()
                .unwrap_or(20),
            encrypt: env::var("DB_ENCRYPT")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
        })
    }

    fn connection_string(&self) -> String {
        format!(
            "Server={},{};\
             Database={};\
             User Id={};\
             Password={};\
             Encrypt={};\
             TrustServerCertificate=false",
            self.host,
            self.port,
            self.database,
            self.user,
            self.password,
            if self.encrypt { "true" } else { "false" }
        )
    }
}

async fn kubernetes_config() -> Result<Pool, Error> {
    let db_config = DbConfig::from_env()
        .expect("Database configuration missing");

    let config = Config::from_connection_string(&db_config.connection_string())?;

    Pool::builder()
        .min_size(db_config.pool_min)
        .max_size(db_config.pool_max)
        .build(config)
        .await
}
```

### Kubernetes ConfigMap Example

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: db-config
data:
  DB_HOST: "sql-server.database.svc.cluster.local"
  DB_PORT: "1433"
  DB_NAME: "production"
  DB_POOL_MIN: "2"
  DB_POOL_MAX: "20"
  DB_ENCRYPT: "true"
---
apiVersion: v1
kind: Secret
metadata:
  name: db-secrets
type: Opaque
stringData:
  DB_USER: "app_user"
  DB_PASSWORD: "YourStrongPassword!"
```

---

## Testing Configuration

### Integration Test Configuration

```rust
#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> Pool {
        let config = Config::from_connection_string(
            &std::env::var("TEST_DATABASE_URL")
                .unwrap_or_else(|_| {
                    "Server=localhost;Database=test;\
                     User Id=sa;Password=Test123!;\
                     TrustServerCertificate=true".to_string()
                })
        ).expect("Invalid test connection string");

        Pool::builder()
            .min_size(1)
            .max_size(5)
            .build(config)
            .await
            .expect("Failed to create test pool")
    }

    #[tokio::test]
    async fn test_query() {
        let pool = test_pool().await;
        let mut conn = pool.get().await.unwrap();

        let rows = conn.query("SELECT 1 AS value", &[]).await.unwrap();
        // ... assertions
    }
}
```

---

## Configuration Checklist

Before deploying to production, verify:

- [ ] `Encrypt` is set to `true` or `strict`
- [ ] `TrustServerCertificate` is `false`
- [ ] `Connect Timeout` is set appropriately
- [ ] `Command Timeout` matches your SLA requirements
- [ ] `Application Name` is set for monitoring
- [ ] Pool `min_size` and `max_size` are tuned for your workload
- [ ] `test_on_borrow` is `true` for high availability
- [ ] Credentials are stored securely (environment variables, secrets manager)
- [ ] Connection string does not contain secrets in logs
