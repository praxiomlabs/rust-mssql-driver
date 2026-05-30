# Connection Pool Metrics

This document describes the metrics and status information available from the rust-mssql-driver connection pool.

## Overview

The pool provides two types of observability data:

1. **Status** - Current snapshot of pool state
2. **Metrics** - Cumulative counters since pool creation

## Pool Status

Get the current pool state with `pool.status()`:

```rust,no_run
use mssql_driver_pool::Pool;
use mssql_client::Config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_connection_string("Server=localhost;Database=db;User Id=sa;Password=Password123")?;
    let pool = Pool::builder().client_config(config).max_connections(10).build().await?;

    let status = pool.status();

    println!("Available connections: {}", status.available);
    println!("In-use connections: {}", status.in_use);
    println!("Total connections: {}", status.total);
    println!("Maximum connections: {}", status.max);
    println!("Utilization: {:.1}%", status.utilization());
    Ok(())
}
```

### Status Fields

| Field | Type | Description |
|-------|------|-------------|
| `available` | `u32` | Idle connections ready for checkout |
| `in_use` | `u32` | Connections currently checked out |
| `total` | `u32` | Total managed connections (available + in_use) |
| `max` | `u32` | Maximum allowed connections |

### Computed Values

```text
impl PoolStatus {
    /// Calculate utilization as a percentage (0.0 to 100.0)
    pub fn utilization(&self) -> f64;
}
```

## Pool Metrics

Get cumulative metrics with `pool.metrics()`:

```rust,no_run
use mssql_driver_pool::Pool;
use mssql_client::Config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_connection_string("Server=localhost;Database=db;User Id=sa;Password=Password123")?;
    let pool = Pool::builder().client_config(config).max_connections(10).build().await?;

    let metrics = pool.metrics();

    println!("Connections created: {}", metrics.connections_created);
    println!("Checkout success rate: {:.2}%", metrics.checkout_success_rate() * 100.0);
    println!("Health check success rate: {:.2}%", metrics.health_check_success_rate() * 100.0);
    println!("Pool uptime: {:?}", metrics.uptime);
    Ok(())
}
```

### Metrics Fields

| Field | Type | Description |
|-------|------|-------------|
| `connections_created` | `u64` | Total connections created since pool start |
| `connections_closed` | `u64` | Total connections closed since pool start |
| `checkouts_successful` | `u64` | Successful connection checkouts |
| `checkouts_failed` | `u64` | Failed checkouts (timeout, pool closed, errors) |
| `health_checks_performed` | `u64` | Total health checks executed |
| `health_checks_failed` | `u64` | Health checks that detected unhealthy connections |
| `resets_performed` | `u64` | sp_reset_connection calls performed |
| `resets_failed` | `u64` | sp_reset_connection calls that failed |
| `uptime` | `Duration` | Time since pool creation |

### Computed Values

```text
impl PoolMetrics {
    /// Calculate checkout success rate (0.0 to 1.0)
    pub fn checkout_success_rate(&self) -> f64;

    /// Calculate health check success rate (0.0 to 1.0)
    pub fn health_check_success_rate(&self) -> f64;

    /// Average connection acquisition time as a Duration
    pub fn avg_acquisition_time(&self) -> Duration;
}
```

## Monitoring Integration

### Prometheus/OpenMetrics

Export metrics in Prometheus format:

```rust,no_run
use std::sync::Arc;
use mssql_driver_pool::Pool;

struct MetricsExporter {
    pool: Arc<Pool>,
}

impl MetricsExporter {
    fn export_prometheus(&self) -> String {
        let status = self.pool.status();
        let metrics = self.pool.metrics();

        format!(
            r#"# HELP mssql_pool_connections_available Number of idle connections
# TYPE mssql_pool_connections_available gauge
mssql_pool_connections_available {}

# HELP mssql_pool_connections_in_use Number of connections in use
# TYPE mssql_pool_connections_in_use gauge
mssql_pool_connections_in_use {}

# HELP mssql_pool_connections_total Total managed connections
# TYPE mssql_pool_connections_total gauge
mssql_pool_connections_total {}

# HELP mssql_pool_connections_max Maximum allowed connections
# TYPE mssql_pool_connections_max gauge
mssql_pool_connections_max {}

# HELP mssql_pool_utilization Pool utilization ratio
# TYPE mssql_pool_utilization gauge
mssql_pool_utilization {}

# HELP mssql_pool_checkouts_total Total checkout attempts
# TYPE mssql_pool_checkouts_total counter
mssql_pool_checkouts_total{{result="success"}} {}
mssql_pool_checkouts_total{{result="failure"}} {}

# HELP mssql_pool_health_checks_total Total health checks
# TYPE mssql_pool_health_checks_total counter
mssql_pool_health_checks_total{{result="success"}} {}
mssql_pool_health_checks_total{{result="failure"}} {}

# HELP mssql_pool_uptime_seconds Pool uptime in seconds
# TYPE mssql_pool_uptime_seconds counter
mssql_pool_uptime_seconds {}
"#,
            status.available,
            status.in_use,
            status.total,
            status.max,
            status.utilization(),
            metrics.checkouts_successful,
            metrics.checkouts_failed,
            metrics.health_checks_performed - metrics.health_checks_failed,
            metrics.health_checks_failed,
            metrics.uptime.as_secs_f64(),
        )
    }
}
```

### Logging Periodic Status

```text
use tokio::time::{interval, Duration};
use tracing::info;

async fn log_pool_status(pool: Arc<Pool>) {
    let mut interval = interval(Duration::from_secs(60));

    loop {
        interval.tick().await;

        let status = pool.status();
        let metrics = pool.metrics();

        info!(
            available = status.available,
            in_use = status.in_use,
            total = status.total,
            utilization = %format!("{:.1}%", status.utilization() * 100.0),
            checkouts = metrics.checkouts_successful,
            checkout_failures = metrics.checkouts_failed,
            "Pool status"
        );
    }
}
```

## Alerting Recommendations

### Critical Alerts

| Condition | Threshold | Action |
|-----------|-----------|--------|
| Utilization >= 90% | > 5 minutes | Scale up pool or reduce connection hold time |
| Checkout failures > 0 | Any sustained | Investigate connection issues |
| Health check failure rate > 10% | Sustained | Check SQL Server health |
| Reset failure rate > 5% | Sustained | Investigate connection state issues |

### Warning Alerts

| Condition | Threshold | Action |
|-----------|-----------|--------|
| Utilization >= 70% | > 15 minutes | Consider increasing pool size |
| Available connections = 0 | > 1 minute | May indicate pool exhaustion |
| Connection churn high | connections_created growing fast | Check for connection leaks |

### Example Alert Rules (Prometheus)

```yaml
groups:
  - name: mssql_pool
    rules:
      - alert: PoolHighUtilization
        expr: mssql_pool_utilization > 0.9
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "Connection pool utilization is very high"
          description: "Pool utilization is {{ $value | humanizePercentage }}"

      - alert: PoolCheckoutFailures
        expr: rate(mssql_pool_checkouts_total{result="failure"}[5m]) > 0
        for: 2m
        labels:
          severity: warning
        annotations:
          summary: "Connection checkout failures detected"

      - alert: PoolHealthCheckFailures
        expr: >
          rate(mssql_pool_health_checks_total{result="failure"}[5m]) /
          rate(mssql_pool_health_checks_total[5m]) > 0.1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High rate of health check failures"
```

## Interpreting Metrics

### Healthy Pool Indicators

- Utilization between 20-70% (headroom for traffic spikes)
- Checkout success rate > 99.9%
- Health check success rate > 99%
- Reset success rate > 99%
- Stable `connections_created` (not growing unboundedly)

### Unhealthy Pool Indicators

- Sustained high utilization (> 90%)
- Checkout failures increasing
- Health check failures > 1%
- Connection churn (rapid create/close cycles)
- `in_use` stuck at `max` for extended periods

### Troubleshooting by Metric

| Symptom | Possible Causes | Investigation |
|---------|-----------------|---------------|
| High checkout failures | Pool exhausted, timeouts too short | Check utilization, increase pool size or timeout |
| High health check failures | Network issues, SQL Server problems | Check SQL Server logs, network connectivity |
| High reset failures | Connection corruption, SQL Server issues | Review SQL Server error logs |
| Connection churn | Short connection lifetimes, errors | Check `max_lifetime`, error logs |
| Zero available connections | Pool exhausted, connection leaks | Profile application connection hold times |

## Dashboard Example

Key metrics for a monitoring dashboard:

```text
┌─────────────────────────────────────────────────────────────┐
│ Connection Pool Health                                       │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Available: 15    In Use: 5     Total: 20    Max: 50       │
│  ████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░  Utilization: 25% │
│                                                             │
│  Checkout Success Rate: 99.98%                              │
│  Health Check Success Rate: 100%                            │
│  Reset Success Rate: 100%                                   │
│                                                             │
│  Uptime: 4h 32m 15s                                        │
│  Total Checkouts: 45,231                                   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Best Practices

1. **Monitor utilization trends** - Gradual increase indicates growing demand
2. **Alert on anomalies** - Sudden changes in metrics indicate problems
3. **Correlate with application metrics** - Match pool metrics with request latency
4. **Set baseline alerts** - Establish normal ranges before production
5. **Log metrics on shutdown** - Capture final metrics for debugging
