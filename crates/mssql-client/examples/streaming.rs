//! Streaming examples.
//!
//! Three ways to read results:
//! - `query` — buffers the whole response, then decodes rows lazily as you
//!   iterate synchronously (convenient for small/medium results).
//! - `query_stream` — reads TDS packets on demand and yields rows without
//!   buffering the whole response (peak memory ~one row; for large results).
//! - `query_stream_blob` — sub-streams a row's trailing MAX/BLOB column from
//!   the socket (peak ~one chunk; for multi-GB cells).
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
        "Server={host};Database={database};User Id={user};Password={password};TrustServerCertificate=true"
    );

    let config = Config::from_connection_string(&conn_str)?;
    let mut client = Client::connect(config).await?;
    println!("Connected to SQL Server");

    // Generate test data using a numbers table pattern
    println!("\n=== Buffered query (lazy row decode) ===");
    streaming_example(&mut client).await?;

    println!("\n=== Incremental streaming (query_stream) ===");
    incremental_streaming_example(&mut client).await?;

    println!("\n=== BLOB sub-streaming (query_stream_blob) ===");
    blob_streaming_example(&mut client).await?;

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
    println!("Query executed in {elapsed:?}");

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
    println!("Processed {count} rows, sum = {sum}");
    println!("Expected sum: {}", (10000 * 10001) / 2);

    Ok(())
}

/// Demonstrates true incremental streaming: rows are read from the socket on
/// demand, so peak memory stays at roughly one row regardless of result size.
async fn incremental_streaming_example(client: &mut Client<Ready>) -> Result<(), Error> {
    let query = r#"
        WITH Numbers AS (
            SELECT 1 AS n
            UNION ALL
            SELECT n + 1 FROM Numbers WHERE n < 100000
        )
        SELECT n FROM Numbers OPTION (MAXRECURSION 0)
    "#;

    println!("Streaming 100,000 rows (nothing but ~one row is buffered)...");
    let start = Instant::now();

    let mut stream = client.query_stream(query, &[]).await?;
    let mut count = 0u64;
    let mut sum: i64 = 0;
    while let Some(row) = stream.try_next().await? {
        let n: i32 = row.get(0)?;
        sum += n as i64;
        count += 1;
    }

    println!(
        "Streamed {count} rows in {:?}, sum = {sum}",
        start.elapsed()
    );
    Ok(())
}

/// Demonstrates BLOB sub-streaming: a row's trailing MAX column is read in
/// chunks from the socket and copied to a writer, never fully materialized.
async fn blob_streaming_example(client: &mut Client<Ready>) -> Result<(), Error> {
    // A 1 MB VARBINARY(MAX) cell alongside a scalar id.
    let query = "SELECT 1 AS id, \
        CAST(REPLICATE(CAST('A' AS VARCHAR(MAX)), 1000000) AS VARBINARY(MAX)) AS doc";

    let mut stream = client.query_stream_blob(query, &[]).await?;
    if let Some(row) = stream.next().await? {
        let id: i32 = row.get_by_name("id")?;
        // Stream straight to a sink (use a tokio::fs::File in real code).
        let mut sink = tokio::io::sink();
        let bytes = stream.copy_blob_to(&mut sink).await?;
        println!("Row id={id}: streamed {bytes} blob bytes to the sink");
    }
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
            println!("Binary data (borrowed slice): {bytes:02X?}");
            println!("  Length: {} bytes", bytes.len());
        }

        // String access with Cow - borrowed when possible
        if let Some(text) = row.get_str(1) {
            println!("Text data: '{text}'");
            println!(
                "  Is borrowed: {}",
                matches!(text, std::borrow::Cow::Borrowed(_))
            );
        }

        // Raw bytes access
        if let Some(raw) = row.get_bytes(2) {
            println!("Raw bytes: {raw:02X?}");
            // Convert to string if it's ASCII
            if let Ok(s) = std::str::from_utf8(raw) {
                println!("  As string: '{s}'");
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
        println!("{group_id:>8} {sum:>10} {count:>8} {avg:>10.2}");
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
