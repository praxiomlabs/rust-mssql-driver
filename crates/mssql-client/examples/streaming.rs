//! Streaming query results example.
//!
//! This example demonstrates memory-efficient processing of large result sets
//! using the streaming API based on the Arc<Bytes> pattern from ADR-004.
//!
//! # Running
//!
//! ```bash
//! cargo run --example streaming
//! ```

// Allow common patterns in example code
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mssql_client::{Client, Config, Error, Ready};
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    let host = std::env::var("MSSQL_HOST").unwrap_or_else(|_| "localhost".into());
    let database = std::env::var("MSSQL_DATABASE").unwrap_or_else(|_| "master".into());
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "Password123!".into());

    let conn_str = format!(
        "Server={};Database={};User Id={};Password={};TrustServerCertificate=true",
        host, database, user, password
    );

    let config = Config::from_connection_string(&conn_str)?;
    let mut client = Client::connect(config).await?;
    println!("Connected to SQL Server");

    // Generate test data using a numbers table pattern
    println!("\n=== Streaming Large Result Set ===");
    streaming_example(&mut client).await?;

    println!("\n=== Memory-Efficient Row Access ===");
    memory_efficient_access_example(&mut client).await?;

    println!("\n=== Processing With Aggregation ===");
    aggregation_example(&mut client).await?;

    client.close().await?;
    println!("\nDone!");

    Ok(())
}

/// Demonstrates streaming a large result set
async fn streaming_example(client: &mut Client<Ready>) -> Result<(), Error> {
    // Generate a sequence of numbers using a CTE
    let query = r#"
        WITH Numbers AS (
            SELECT 1 AS n
            UNION ALL
            SELECT n + 1 FROM Numbers WHERE n < 10000
        )
        SELECT n, 'Item_' + CAST(n AS VARCHAR(10)) AS name
        FROM Numbers
        OPTION (MAXRECURSION 10000)
    "#;

    println!("Executing query that returns 10,000 rows...");
    let start = Instant::now();

    let rows = client.query(query, &[]).await?;

    let elapsed = start.elapsed();
    println!("Query executed in {:?}", elapsed);

    // Process rows one at a time - QueryStream yields Result<Row, Error>
    let mut count = 0;
    let mut sum: i64 = 0;

    for result in rows {
        let row = result?;
        let n: i32 = row.get(0)?;
        sum += n as i64;
        count += 1;

        // Print progress every 1000 rows
        if count % 1000 == 0 {
            print!(".");
            std::io::Write::flush(&mut std::io::stdout()).unwrap();
        }
    }

    println!();
    println!("Processed {} rows, sum = {}", count, sum);
    println!("Expected sum: {}", (10000 * 10001) / 2);

    Ok(())
}

/// Demonstrates zero-copy byte access for memory efficiency
async fn memory_efficient_access_example(client: &mut Client<Ready>) -> Result<(), Error> {
    // Query returns binary and string data
    let query = r#"
        SELECT
            CAST(123456 AS VARBINARY(8)) AS binary_data,
            N'Hello, World!' AS text_data,
            0x48454C4C4F AS raw_bytes
    "#;

    let rows = client.query(query, &[]).await?;

    for result in rows {
        let row = result?;

        // Zero-copy byte access - borrows directly from the packet buffer
        if let Some(bytes) = row.get_bytes(0) {
            println!("Binary data (borrowed slice): {:02X?}", bytes);
            println!("  Length: {} bytes", bytes.len());
        }

        // String access with Cow - borrowed when possible
        if let Some(text) = row.get_str(1) {
            println!("Text data: '{}'", text);
            println!(
                "  Is borrowed: {}",
                matches!(text, std::borrow::Cow::Borrowed(_))
            );
        }

        // Raw bytes access
        if let Some(raw) = row.get_bytes(2) {
            println!("Raw bytes: {:02X?}", raw);
            // Convert to string if it's ASCII
            if let Ok(s) = std::str::from_utf8(raw) {
                println!("  As string: '{}'", s);
            }
        }
    }

    Ok(())
}

/// Demonstrates efficient aggregation over streaming results
async fn aggregation_example(client: &mut Client<Ready>) -> Result<(), Error> {
    // Generate sample data for aggregation
    let query = r#"
        WITH Data AS (
            SELECT 1 AS category, ABS(CHECKSUM(NEWID())) % 100 AS value
            UNION ALL
            SELECT category + 1, ABS(CHECKSUM(NEWID())) % 100
            FROM Data WHERE category < 1000
        )
        SELECT
            category % 10 AS group_id,
            value
        FROM Data
        OPTION (MAXRECURSION 1000)
    "#;

    println!("Aggregating 1,000 rows into 10 groups...");

    let rows = client.query(query, &[]).await?;

    // Aggregate in a single pass
    let mut groups: std::collections::HashMap<i32, (i64, i32)> = std::collections::HashMap::new();

    for result in rows {
        let row = result?;
        let group_id: i32 = row.get(0)?;
        let value: i32 = row.get(1)?;

        let entry = groups.entry(group_id).or_insert((0, 0));
        entry.0 += value as i64;
        entry.1 += 1;
    }

    println!("\nGroup Statistics:");
    println!(
        "{:>8} {:>10} {:>8} {:>10}",
        "Group", "Sum", "Count", "Average"
    );
    println!("{}", "-".repeat(40));

    let mut group_ids: Vec<_> = groups.keys().collect();
    group_ids.sort();

    for group_id in group_ids {
        let (sum, count) = groups[group_id];
        let avg = sum as f64 / count as f64;
        println!("{:>8} {:>10} {:>8} {:>10.2}", group_id, sum, count, avg);
    }

    Ok(())
}

/// Example: Processing rows with early termination
#[allow(dead_code)]
async fn early_termination_example(client: &mut Client<Ready>) -> Result<(), Error> {
    let rows = client.query("SELECT * FROM large_table", &[]).await?;

    // Process until we find what we're looking for
    for result in rows {
        let row = result?;
        let status: String = row.get_by_name("status")?;

        if status == "found" {
            println!("Found the row we were looking for!");
            // Early return - the iterator will be dropped
            // This is efficient because rows are processed on-demand
            break;
        }
    }

    Ok(())
}
