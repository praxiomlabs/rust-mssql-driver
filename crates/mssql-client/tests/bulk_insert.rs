//! Bulk insert integration tests.
//!
//! These tests exercise the `BulkWriter` API against a live SQL Server instance.
//! They are ignored by default and can be run with:
//!
//! ```bash
//! export MSSQL_HOST=localhost
//! export MSSQL_USER=sa
//! export MSSQL_PASSWORD=YourPassword
//! export MSSQL_ENCRYPT=false
//!
//! cargo test -p mssql-client --test bulk_insert -- --ignored
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::manual_flatten,
    clippy::approx_constant,
    clippy::bool_assert_comparison
)]

use mssql_client::{BulkColumn, BulkInsertBuilder, BulkOptions, Client, Config, SqlValue};

/// Helper to get test configuration from environment variables.
fn get_test_config() -> Option<Config> {
    let host = std::env::var("MSSQL_HOST").ok()?;
    let port = std::env::var("MSSQL_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(1433);
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "MyStrongPassw0rd".into());
    let database = std::env::var("MSSQL_DATABASE").unwrap_or_else(|_| "master".into());
    let encrypt = std::env::var("MSSQL_ENCRYPT").unwrap_or_else(|_| "false".into());

    let conn_str = format!(
        "Server={host},{port};Database={database};User Id={user};Password={password};\
         TrustServerCertificate=true;Encrypt={encrypt}"
    );

    Config::from_connection_string(&conn_str).ok()
}

// =============================================================================
// Basic Bulk Insert Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_int_and_nvarchar() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "CREATE TABLE #BulkTest1 (id INT NOT NULL, name NVARCHAR(100) NOT NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    let builder = BulkInsertBuilder::new("#BulkTest1").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0).unwrap(),
        BulkColumn::new("name", "NVARCHAR(100)", 1).unwrap(),
    ]);

    let mut writer = client
        .bulk_insert(&builder)
        .await
        .expect("Failed to start bulk insert");

    writer
        .send_row_values(&[SqlValue::Int(1), SqlValue::String("Alice".into())])
        .expect("Failed to send row 1");
    writer
        .send_row_values(&[SqlValue::Int(2), SqlValue::String("Bob".into())])
        .expect("Failed to send row 2");
    writer
        .send_row_values(&[SqlValue::Int(3), SqlValue::String("Charlie".into())])
        .expect("Failed to send row 3");

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, 3);

    // Verify data round-trip
    let rows = client
        .query("SELECT id, name FROM #BulkTest1 ORDER BY id", &[])
        .await
        .expect("Query failed");

    let data: Vec<(i32, String)> = rows
        .filter_map(|r| r.ok())
        .map(|row| (row.get(0).unwrap(), row.get(1).unwrap()))
        .collect();

    assert_eq!(data.len(), 3);
    assert_eq!(data[0], (1, "Alice".to_string()));
    assert_eq!(data[1], (2, "Bob".to_string()));
    assert_eq!(data[2], (3, "Charlie".to_string()));

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_zero_rows() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute("CREATE TABLE #BulkEmpty (id INT NOT NULL)", &[])
        .await
        .expect("Failed to create table");

    let builder = BulkInsertBuilder::new("#BulkEmpty")
        .with_typed_columns(vec![BulkColumn::new("id", "INT", 0).unwrap()]);

    let writer = client
        .bulk_insert(&builder)
        .await
        .expect("Failed to start bulk insert");
    let result = writer.finish().await.expect("Failed to finish bulk insert");

    assert_eq!(result.rows_affected, 0);

    let rows = client
        .query("SELECT COUNT(*) FROM #BulkEmpty", &[])
        .await
        .expect("Count query failed");

    let count: i32 = rows
        .filter_map(|r| r.ok())
        .next()
        .map(|row| row.get(0).unwrap())
        .unwrap();

    assert_eq!(count, 0);

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_large_batch() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "CREATE TABLE #BulkLarge (id INT NOT NULL, value NVARCHAR(200) NOT NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    let builder = BulkInsertBuilder::new("#BulkLarge").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0).unwrap(),
        BulkColumn::new("value", "NVARCHAR(200)", 1).unwrap(),
    ]);

    let mut writer = client
        .bulk_insert(&builder)
        .await
        .expect("Failed to start bulk insert");

    let row_count = 1000;
    for i in 1..=row_count {
        writer
            .send_row_values(&[
                SqlValue::Int(i),
                SqlValue::String(format!("Row number {i} with some padding text")),
            ])
            .expect("Failed to send row");
    }

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, row_count as u64);

    // Verify count
    let rows = client
        .query("SELECT COUNT(*) FROM #BulkLarge", &[])
        .await
        .expect("Count query failed");

    let count: i32 = rows
        .filter_map(|r| r.ok())
        .next()
        .map(|row| row.get(0).unwrap())
        .unwrap();

    assert_eq!(count, row_count);

    // Spot-check first and last rows
    let rows = client
        .query(
            "SELECT id, value FROM #BulkLarge WHERE id IN (1, 1000) ORDER BY id",
            &[],
        )
        .await
        .expect("Spot check query failed");

    let data: Vec<(i32, String)> = rows
        .filter_map(|r| r.ok())
        .map(|row| (row.get(0).unwrap(), row.get(1).unwrap()))
        .collect();

    assert_eq!(data.len(), 2);
    assert_eq!(data[0].0, 1);
    assert!(data[0].1.contains("Row number 1"));
    assert_eq!(data[1].0, 1000);
    assert!(data[1].1.contains("Row number 1000"));

    client.close().await.expect("Failed to close");
}

// =============================================================================
// NULL Handling
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_null_values() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "CREATE TABLE #BulkNulls (id INT NOT NULL, name NVARCHAR(100) NULL, age INT NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    let builder = BulkInsertBuilder::new("#BulkNulls").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("name", "NVARCHAR(100)", 1).unwrap(),
        BulkColumn::new("age", "INT", 2).unwrap(),
    ]);

    let mut writer = client
        .bulk_insert(&builder)
        .await
        .expect("Failed to start bulk insert");

    // Row with all values
    writer
        .send_row_values(&[
            SqlValue::Int(1),
            SqlValue::String("Alice".into()),
            SqlValue::Int(30),
        ])
        .expect("Failed to send row 1");

    // Row with NULL name
    writer
        .send_row_values(&[SqlValue::Int(2), SqlValue::Null, SqlValue::Int(25)])
        .expect("Failed to send row 2");

    // Row with NULL age
    writer
        .send_row_values(&[
            SqlValue::Int(3),
            SqlValue::String("Charlie".into()),
            SqlValue::Null,
        ])
        .expect("Failed to send row 3");

    // Row with both NULLs
    writer
        .send_row_values(&[SqlValue::Int(4), SqlValue::Null, SqlValue::Null])
        .expect("Failed to send row 4");

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, 4);

    // Verify NULL handling
    let rows = client
        .query("SELECT id, name, age FROM #BulkNulls ORDER BY id", &[])
        .await
        .expect("Query failed");

    let data: Vec<(i32, Option<String>, Option<i32>)> = rows
        .filter_map(|r| r.ok())
        .map(|row| (row.get(0).unwrap(), row.try_get(1), row.try_get(2)))
        .collect();

    assert_eq!(data.len(), 4);
    assert_eq!(data[0], (1, Some("Alice".to_string()), Some(30)));
    assert_eq!(data[1], (2, None, Some(25)));
    assert_eq!(data[2], (3, Some("Charlie".to_string()), None));
    assert_eq!(data[3], (4, None, None));

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Multiple Data Types
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_multiple_types() {
    use chrono::NaiveDate;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "CREATE TABLE #BulkTypes (
                id INT NOT NULL,
                tiny TINYINT NOT NULL,
                small SMALLINT NOT NULL,
                big BIGINT NOT NULL,
                flag BIT NOT NULL,
                price DECIMAL(10,2) NOT NULL,
                ratio FLOAT NOT NULL,
                label NVARCHAR(50) NOT NULL,
                created DATE NOT NULL
            )",
            &[],
        )
        .await
        .expect("Failed to create table");

    let builder = BulkInsertBuilder::new("#BulkTypes").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0).unwrap(),
        BulkColumn::new("tiny", "TINYINT", 1).unwrap(),
        BulkColumn::new("small", "SMALLINT", 2).unwrap(),
        BulkColumn::new("big", "BIGINT", 3).unwrap(),
        BulkColumn::new("flag", "BIT", 4).unwrap(),
        BulkColumn::new("price", "DECIMAL(10,2)", 5).unwrap(),
        BulkColumn::new("ratio", "FLOAT", 6).unwrap(),
        BulkColumn::new("label", "NVARCHAR(50)", 7).unwrap(),
        BulkColumn::new("created", "DATE", 8).unwrap(),
    ]);

    let mut writer = client
        .bulk_insert(&builder)
        .await
        .expect("Failed to start bulk insert");

    writer
        .send_row_values(&[
            SqlValue::Int(1),
            SqlValue::TinyInt(255),
            SqlValue::SmallInt(-100),
            SqlValue::BigInt(9_000_000_000),
            SqlValue::Bool(true),
            SqlValue::Decimal(Decimal::from_str("123.45").unwrap()),
            SqlValue::Double(3.14159),
            SqlValue::String("Hello World".into()),
            SqlValue::Date(NaiveDate::from_ymd_opt(2025, 6, 15).unwrap()),
        ])
        .expect("Failed to send row");

    writer
        .send_row_values(&[
            SqlValue::Int(2),
            SqlValue::TinyInt(0),
            SqlValue::SmallInt(32767),
            SqlValue::BigInt(-1),
            SqlValue::Bool(false),
            SqlValue::Decimal(Decimal::from_str("-999.99").unwrap()),
            SqlValue::Double(0.0),
            SqlValue::String("".into()),
            SqlValue::Date(NaiveDate::from_ymd_opt(1900, 1, 1).unwrap()),
        ])
        .expect("Failed to send row");

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, 2);

    // Verify round-trip
    let rows = client
        .query(
            "SELECT id, tiny, small, big, flag, price, ratio, label, created \
             FROM #BulkTypes ORDER BY id",
            &[],
        )
        .await
        .expect("Query failed");

    let data: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    assert_eq!(data.len(), 2);

    // Row 1
    let row = &data[0];
    assert_eq!(row.get::<i32>(0).unwrap(), 1);
    assert_eq!(row.get::<u8>(1).unwrap(), 255);
    assert_eq!(row.get::<i16>(2).unwrap(), -100);
    assert_eq!(row.get::<i64>(3).unwrap(), 9_000_000_000);
    assert_eq!(row.get::<bool>(4).unwrap(), true);
    assert_eq!(
        row.get::<Decimal>(5).unwrap(),
        Decimal::from_str("123.45").unwrap()
    );
    assert!((row.get::<f64>(6).unwrap() - 3.14159).abs() < 1e-10);
    assert_eq!(row.get::<String>(7).unwrap(), "Hello World");
    assert_eq!(
        row.get::<NaiveDate>(8).unwrap(),
        NaiveDate::from_ymd_opt(2025, 6, 15).unwrap()
    );

    // Row 2
    let row = &data[1];
    assert_eq!(row.get::<i32>(0).unwrap(), 2);
    assert_eq!(row.get::<u8>(1).unwrap(), 0);
    assert_eq!(row.get::<i16>(2).unwrap(), 32767);
    assert_eq!(row.get::<i64>(3).unwrap(), -1);
    assert_eq!(row.get::<bool>(4).unwrap(), false);
    assert_eq!(
        row.get::<Decimal>(5).unwrap(),
        Decimal::from_str("-999.99").unwrap()
    );
    assert_eq!(row.get::<f64>(6).unwrap(), 0.0);
    assert_eq!(row.get::<String>(7).unwrap(), "");
    assert_eq!(
        row.get::<NaiveDate>(8).unwrap(),
        NaiveDate::from_ymd_opt(1900, 1, 1).unwrap()
    );

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_binary_and_guid() {
    use uuid::Uuid;

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "CREATE TABLE #BulkBin (id INT NOT NULL, uid UNIQUEIDENTIFIER NOT NULL, data VARBINARY(200) NOT NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    let builder = BulkInsertBuilder::new("#BulkBin").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0).unwrap(),
        BulkColumn::new("uid", "UNIQUEIDENTIFIER", 1).unwrap(),
        BulkColumn::new("data", "VARBINARY(200)", 2).unwrap(),
    ]);

    let test_uuid = Uuid::parse_str("12345678-1234-5678-1234-567812345678").unwrap();
    let test_bytes: bytes::Bytes = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04].into();

    let mut writer = client
        .bulk_insert(&builder)
        .await
        .expect("Failed to start bulk insert");

    writer
        .send_row_values(&[
            SqlValue::Int(1),
            SqlValue::Uuid(test_uuid),
            SqlValue::Binary(test_bytes.clone()),
        ])
        .expect("Failed to send row");

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, 1);

    let rows = client
        .query("SELECT id, uid, data FROM #BulkBin", &[])
        .await
        .expect("Query failed");

    let data: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    assert_eq!(data.len(), 1);

    let row = &data[0];
    assert_eq!(row.get::<i32>(0).unwrap(), 1);
    let uid: Uuid = row.get(1).unwrap();
    assert_eq!(
        uid, test_uuid,
        "UUID round-trip should preserve exact value"
    );
    assert_eq!(row.get::<Vec<u8>>(2).unwrap(), &test_bytes[..]);

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_nvarchar_max() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "CREATE TABLE #BulkMax (id INT NOT NULL, content NVARCHAR(MAX) NOT NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    let builder = BulkInsertBuilder::new("#BulkMax").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0).unwrap(),
        BulkColumn::new("content", "NVARCHAR(MAX)", 1).unwrap(),
    ]);

    // Large string covers multi-packet PLP framing (each packet is 4096 bytes
    // by default; a 4,000-char UTF-16 string is 8,000 bytes and spans two).
    let large_string: String = "ABCDEFGHIJ".repeat(400); // 4,000 chars

    let mut writer = client
        .bulk_insert(&builder)
        .await
        .expect("Failed to start bulk insert");

    writer
        .send_row_values(&[SqlValue::Int(1), SqlValue::String(large_string.clone())])
        .expect("Failed to send row");

    // Also test a short string through the same MAX column
    writer
        .send_row_values(&[SqlValue::Int(2), SqlValue::String("short".into())])
        .expect("Failed to send row");

    // And an empty string — exercises the zero-chunk PLP path
    writer
        .send_row_values(&[SqlValue::Int(3), SqlValue::String(String::new())])
        .expect("Failed to send row");

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, 3);

    // LEN() on NVARCHAR(MAX) returns BIGINT, not INT.
    let rows = client
        .query(
            "SELECT id, LEN(content), content FROM #BulkMax ORDER BY id",
            &[],
        )
        .await
        .expect("Query failed");

    let data: Vec<(i32, i64, String)> = rows
        .filter_map(|r| r.ok())
        .map(|row| {
            (
                row.get(0).unwrap(),
                row.get(1).unwrap(),
                row.get(2).unwrap(),
            )
        })
        .collect();

    assert_eq!(data.len(), 3);
    assert_eq!(data[0].0, 1);
    assert_eq!(data[0].1, 4_000);
    assert_eq!(data[0].2, large_string);
    assert_eq!(data[1].0, 2);
    assert_eq!(data[1].1, 5);
    assert_eq!(data[1].2, "short");
    assert_eq!(data[2].0, 3);
    assert_eq!(data[2].1, 0);
    assert_eq!(data[2].2, "");

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_varbinary_max() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "CREATE TABLE #BulkVbMax (id INT NOT NULL, payload VARBINARY(MAX) NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    let builder = BulkInsertBuilder::new("#BulkVbMax").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0).unwrap(),
        BulkColumn::new("payload", "VARBINARY(MAX)", 1)
            .unwrap()
            .with_nullable(true),
    ]);

    let big_blob: Vec<u8> = (0u8..=255).cycle().take(10_000).collect();
    let small_blob: Vec<u8> = vec![0xAA, 0xBB, 0xCC, 0xDD];

    let mut writer = client
        .bulk_insert(&builder)
        .await
        .expect("Failed to start bulk insert");

    writer
        .send_row_values(&[SqlValue::Int(1), SqlValue::Binary(big_blob.clone().into())])
        .expect("send row 1");
    writer
        .send_row_values(&[
            SqlValue::Int(2),
            SqlValue::Binary(small_blob.clone().into()),
        ])
        .expect("send row 2");
    writer
        .send_row_values(&[SqlValue::Int(3), SqlValue::Binary(Vec::<u8>::new().into())])
        .expect("send empty row");
    writer
        .send_row_values(&[SqlValue::Int(4), SqlValue::Null])
        .expect("send null row");

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, 4);

    // DATALENGTH on VARBINARY(MAX) returns BIGINT.
    let rows = client
        .query(
            "SELECT id, DATALENGTH(payload), payload FROM #BulkVbMax ORDER BY id",
            &[],
        )
        .await
        .expect("Query failed");

    let data: Vec<(i32, Option<i64>, Option<Vec<u8>>)> = rows
        .filter_map(|r| r.ok())
        .map(|row| {
            (
                row.get(0).unwrap(),
                row.get(1).unwrap(),
                row.get(2).unwrap(),
            )
        })
        .collect();

    assert_eq!(data.len(), 4);
    assert_eq!(data[0].0, 1);
    assert_eq!(data[0].1, Some(10_000));
    assert_eq!(data[0].2.as_deref(), Some(big_blob.as_slice()));
    assert_eq!(data[1].0, 2);
    assert_eq!(data[1].1, Some(4));
    assert_eq!(data[1].2.as_deref(), Some(small_blob.as_slice()));
    assert_eq!(data[2].0, 3);
    assert_eq!(data[2].1, Some(0));
    assert_eq!(data[2].2.as_deref(), Some(&[][..]));
    assert_eq!(data[3].0, 4);
    assert_eq!(data[3].1, None);
    assert_eq!(data[3].2, None);

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Bulk Options
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_with_table_lock() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "CREATE TABLE #BulkLock (id INT NOT NULL, val NVARCHAR(50) NOT NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    let builder = BulkInsertBuilder::new("#BulkLock")
        .with_typed_columns(vec![
            BulkColumn::new("id", "INT", 0).unwrap(),
            BulkColumn::new("val", "NVARCHAR(50)", 1).unwrap(),
        ])
        .table_lock(true);

    let mut writer = client
        .bulk_insert(&builder)
        .await
        .expect("Failed to start bulk insert");

    for i in 1..=10 {
        writer
            .send_row_values(&[SqlValue::Int(i), SqlValue::String(format!("val_{i}"))])
            .expect("Failed to send row");
    }

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, 10);

    let rows = client
        .query("SELECT COUNT(*) FROM #BulkLock", &[])
        .await
        .expect("Count query failed");

    let count: i32 = rows
        .filter_map(|r| r.ok())
        .next()
        .map(|row| row.get(0).unwrap())
        .unwrap();

    assert_eq!(count, 10);

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_with_check_constraints() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Create a persistent table with a CHECK constraint (temp tables support constraints)
    client
        .execute(
            "CREATE TABLE #BulkCheck (id INT NOT NULL, age INT NOT NULL CHECK (age >= 0 AND age <= 150))",
            &[],
        )
        .await
        .expect("Failed to create table");

    // Insert valid data with CHECK_CONSTRAINTS enabled (default)
    let builder = BulkInsertBuilder::new("#BulkCheck").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0).unwrap(),
        BulkColumn::new("age", "INT", 1).unwrap(),
    ]);

    let mut writer = client
        .bulk_insert(&builder)
        .await
        .expect("Failed to start bulk insert");

    writer
        .send_row_values(&[SqlValue::Int(1), SqlValue::Int(25)])
        .expect("Failed to send row");

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, 1);

    // Now try to insert invalid data — should fail because CHECK_CONSTRAINTS is on
    let builder = BulkInsertBuilder::new("#BulkCheck")
        .with_typed_columns(vec![
            BulkColumn::new("id", "INT", 0).unwrap(),
            BulkColumn::new("age", "INT", 1).unwrap(),
        ])
        .with_options(BulkOptions {
            check_constraints: true,
            ..BulkOptions::default()
        });

    let mut writer = client
        .bulk_insert(&builder)
        .await
        .expect("Failed to start bulk insert");

    writer
        .send_row_values(&[SqlValue::Int(2), SqlValue::Int(-5)])
        .expect("Failed to send row");

    let result = writer.finish().await;
    assert!(
        result.is_err(),
        "Should fail CHECK constraint with negative age"
    );

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Schema-Qualified Table Names
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_schema_qualified() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Use tempdb for persistent table with schema qualifier
    client
        .execute("USE tempdb", &[])
        .await
        .expect("Failed to switch to tempdb");

    client
        .execute(
            "IF OBJECT_ID('dbo.BulkSchemaTest', 'U') IS NOT NULL DROP TABLE dbo.BulkSchemaTest",
            &[],
        )
        .await
        .expect("Failed to drop table");

    client
        .execute(
            "CREATE TABLE dbo.BulkSchemaTest (id INT NOT NULL, name NVARCHAR(50) NOT NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    let builder = BulkInsertBuilder::new("dbo.BulkSchemaTest").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0).unwrap(),
        BulkColumn::new("name", "NVARCHAR(50)", 1).unwrap(),
    ]);

    let mut writer = client
        .bulk_insert(&builder)
        .await
        .expect("Failed to start bulk insert");

    writer
        .send_row_values(&[SqlValue::Int(1), SqlValue::String("test".into())])
        .expect("Failed to send row");

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, 1);

    // Cleanup
    client
        .execute("DROP TABLE dbo.BulkSchemaTest", &[])
        .await
        .expect("Failed to drop table");

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Bulk Insert Within Transaction
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_in_transaction() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "CREATE TABLE #BulkTxn (id INT NOT NULL, val NVARCHAR(50) NOT NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    // Begin transaction
    let mut client = client
        .begin_transaction()
        .await
        .expect("Failed to begin txn");

    let builder = BulkInsertBuilder::new("#BulkTxn").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0).unwrap(),
        BulkColumn::new("val", "NVARCHAR(50)", 1).unwrap(),
    ]);

    let mut writer = client
        .bulk_insert(&builder)
        .await
        .expect("Failed to start bulk insert");

    writer
        .send_row_values(&[SqlValue::Int(1), SqlValue::String("inside txn".into())])
        .expect("Failed to send row");

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, 1);

    // Commit
    let mut client = client.commit().await.expect("Failed to commit");

    let rows = client
        .query("SELECT COUNT(*) FROM #BulkTxn", &[])
        .await
        .expect("Count query failed");

    let count: i32 = rows
        .filter_map(|r| r.ok())
        .next()
        .map(|row| row.get(0).unwrap())
        .unwrap();

    assert_eq!(count, 1);

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_transaction_rollback() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "CREATE TABLE #BulkRollback (id INT NOT NULL, val NVARCHAR(50) NOT NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    // Insert data in a transaction, then rollback
    let mut client = client
        .begin_transaction()
        .await
        .expect("Failed to begin txn");

    let builder = BulkInsertBuilder::new("#BulkRollback").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0).unwrap(),
        BulkColumn::new("val", "NVARCHAR(50)", 1).unwrap(),
    ]);

    let mut writer = client
        .bulk_insert(&builder)
        .await
        .expect("Failed to start bulk insert");

    for i in 1..=5 {
        writer
            .send_row_values(&[SqlValue::Int(i), SqlValue::String(format!("row_{i}"))])
            .expect("Failed to send row");
    }

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, 5);

    // Rollback
    let mut client = client.rollback().await.expect("Failed to rollback");

    let rows = client
        .query("SELECT COUNT(*) FROM #BulkRollback", &[])
        .await
        .expect("Count query failed");

    let count: i32 = rows
        .filter_map(|r| r.ok())
        .next()
        .map(|row| row.get(0).unwrap())
        .unwrap();

    assert_eq!(count, 0, "Rows should be rolled back");

    client.close().await.expect("Failed to close");
}

// =============================================================================
// FIRE_TRIGGERS Option
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_fire_triggers() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute("USE tempdb", &[])
        .await
        .expect("Failed to switch to tempdb");

    // Create source table
    client
        .execute(
            "IF OBJECT_ID('dbo.BulkTriggerSrc', 'U') IS NOT NULL DROP TABLE dbo.BulkTriggerSrc",
            &[],
        )
        .await
        .expect("Cleanup failed");

    client
        .execute(
            "IF OBJECT_ID('dbo.BulkTriggerLog', 'U') IS NOT NULL DROP TABLE dbo.BulkTriggerLog",
            &[],
        )
        .await
        .expect("Cleanup failed");

    client
        .execute(
            "CREATE TABLE dbo.BulkTriggerSrc (id INT NOT NULL, name NVARCHAR(50) NOT NULL)",
            &[],
        )
        .await
        .expect("Failed to create source table");

    client
        .execute(
            "CREATE TABLE dbo.BulkTriggerLog (src_id INT NOT NULL, logged_at DATETIME2 NOT NULL DEFAULT GETDATE())",
            &[],
        )
        .await
        .expect("Failed to create log table");

    // Create trigger
    client
        .execute(
            "CREATE TRIGGER trg_BulkTriggerSrc_Insert ON dbo.BulkTriggerSrc \
             AFTER INSERT AS \
             INSERT INTO dbo.BulkTriggerLog (src_id) SELECT id FROM inserted",
            &[],
        )
        .await
        .expect("Failed to create trigger");

    // Bulk insert WITH fire_triggers
    let builder = BulkInsertBuilder::new("dbo.BulkTriggerSrc")
        .with_typed_columns(vec![
            BulkColumn::new("id", "INT", 0).unwrap(),
            BulkColumn::new("name", "NVARCHAR(50)", 1).unwrap(),
        ])
        .fire_triggers(true);

    let mut writer = client
        .bulk_insert(&builder)
        .await
        .expect("Failed to start bulk insert");

    for i in 1..=3 {
        writer
            .send_row_values(&[SqlValue::Int(i), SqlValue::String(format!("name_{i}"))])
            .expect("Failed to send row");
    }

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    // rows_affected may include trigger-inserted rows — that's the investigation in 3.8
    assert!(
        result.rows_affected >= 3,
        "Should insert at least 3 rows, got {}",
        result.rows_affected
    );

    // Verify source data
    let rows = client
        .query("SELECT COUNT(*) FROM dbo.BulkTriggerSrc", &[])
        .await
        .expect("Count query failed");

    let src_count: i32 = rows
        .filter_map(|r| r.ok())
        .next()
        .map(|row| row.get(0).unwrap())
        .unwrap();

    assert_eq!(src_count, 3, "Source table should have 3 rows");

    // Verify trigger fired
    let rows = client
        .query("SELECT COUNT(*) FROM dbo.BulkTriggerLog", &[])
        .await
        .expect("Count query failed");

    let log_count: i32 = rows
        .filter_map(|r| r.ok())
        .next()
        .map(|row| row.get(0).unwrap())
        .unwrap();

    assert_eq!(log_count, 3, "Trigger should have logged 3 rows");

    // Cleanup
    client
        .execute("DROP TABLE dbo.BulkTriggerSrc", &[])
        .await
        .expect("Cleanup failed");
    client
        .execute("DROP TABLE dbo.BulkTriggerLog", &[])
        .await
        .expect("Cleanup failed");

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_triggers_not_fired_by_default() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute("USE tempdb", &[])
        .await
        .expect("Failed to switch to tempdb");

    client
        .execute(
            "IF OBJECT_ID('dbo.BulkNoTrigSrc', 'U') IS NOT NULL DROP TABLE dbo.BulkNoTrigSrc",
            &[],
        )
        .await
        .expect("Cleanup failed");

    client
        .execute(
            "IF OBJECT_ID('dbo.BulkNoTrigLog', 'U') IS NOT NULL DROP TABLE dbo.BulkNoTrigLog",
            &[],
        )
        .await
        .expect("Cleanup failed");

    client
        .execute("CREATE TABLE dbo.BulkNoTrigSrc (id INT NOT NULL)", &[])
        .await
        .expect("Failed to create table");

    client
        .execute("CREATE TABLE dbo.BulkNoTrigLog (src_id INT NOT NULL)", &[])
        .await
        .expect("Failed to create table");

    client
        .execute(
            "CREATE TRIGGER trg_BulkNoTrigSrc ON dbo.BulkNoTrigSrc \
             AFTER INSERT AS \
             INSERT INTO dbo.BulkNoTrigLog (src_id) SELECT id FROM inserted",
            &[],
        )
        .await
        .expect("Failed to create trigger");

    // Bulk insert WITHOUT fire_triggers (default: false)
    let builder = BulkInsertBuilder::new("dbo.BulkNoTrigSrc")
        .with_typed_columns(vec![BulkColumn::new("id", "INT", 0).unwrap()]);

    let mut writer = client
        .bulk_insert(&builder)
        .await
        .expect("Failed to start bulk insert");

    for i in 1..=3 {
        writer
            .send_row_values(&[SqlValue::Int(i)])
            .expect("Failed to send row");
    }

    writer.finish().await.expect("Failed to finish bulk insert");

    // Verify trigger did NOT fire
    let rows = client
        .query("SELECT COUNT(*) FROM dbo.BulkNoTrigLog", &[])
        .await
        .expect("Count query failed");

    let log_count: i32 = rows
        .filter_map(|r| r.ok())
        .next()
        .map(|row| row.get(0).unwrap())
        .unwrap();

    assert_eq!(log_count, 0, "Trigger should NOT have fired");

    // Cleanup
    client
        .execute("DROP TABLE dbo.BulkNoTrigSrc", &[])
        .await
        .expect("Cleanup failed");
    client
        .execute("DROP TABLE dbo.BulkNoTrigLog", &[])
        .await
        .expect("Cleanup failed");

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Connection Reuse After Bulk Insert
// =============================================================================

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_connection_usable_after_bulk_insert() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute("CREATE TABLE #BulkReuse (id INT NOT NULL)", &[])
        .await
        .expect("Failed to create table");

    // First bulk insert
    let builder = BulkInsertBuilder::new("#BulkReuse")
        .with_typed_columns(vec![BulkColumn::new("id", "INT", 0).unwrap()]);

    let mut writer = client
        .bulk_insert(&builder)
        .await
        .expect("Failed to start bulk insert");
    writer
        .send_row_values(&[SqlValue::Int(1)])
        .expect("Failed to send row");
    writer.finish().await.expect("Failed to finish");

    // Regular query should work
    let rows = client
        .query("SELECT 42 AS answer", &[])
        .await
        .expect("Query after bulk should work");

    let val: i32 = rows
        .filter_map(|r| r.ok())
        .next()
        .map(|row| row.get(0).unwrap())
        .unwrap();
    assert_eq!(val, 42);

    // Second bulk insert should also work
    let mut writer = client
        .bulk_insert(&builder)
        .await
        .expect("Failed to start second bulk insert");
    writer
        .send_row_values(&[SqlValue::Int(2)])
        .expect("Failed to send row");
    writer
        .finish()
        .await
        .expect("Failed to finish second bulk insert");

    let rows = client
        .query("SELECT COUNT(*) FROM #BulkReuse", &[])
        .await
        .expect("Count query failed");

    let count: i32 = rows
        .filter_map(|r| r.ok())
        .next()
        .map(|row| row.get(0).unwrap())
        .unwrap();

    assert_eq!(count, 2, "Both bulk inserts should have persisted");

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Hand-Crafted COLMETADATA Tests (no schema discovery)
// =============================================================================

/// Test bulk insert using hand-crafted COLMETADATA (no SELECT TOP 0 round-trip).
/// Exercises the `write_colmetadata()` code path directly. If the hand-crafted
/// wire format is wrong, the server will reject the BulkLoad packet.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_without_schema_discovery() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    // Mix of NOT NULL (exercises fixed type ID path: INT→0x38, BIT→0x32)
    // and NULL (exercises nullable type ID path: DECIMAL→0x6C, NVARCHAR→0xE7).
    client
        .execute(
            "CREATE TABLE #BulkNoDiscovery (\
                id INT NOT NULL, \
                name NVARCHAR(100) NOT NULL, \
                amount DECIMAL(18,2) NULL, \
                flag BIT NOT NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    let builder = BulkInsertBuilder::new("#BulkNoDiscovery").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("name", "NVARCHAR(100)", 1)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("amount", "DECIMAL(18,2)", 2).unwrap(),
        BulkColumn::new("flag", "BIT", 3)
            .unwrap()
            .with_nullable(false),
    ]);

    let mut writer = client
        .bulk_insert_without_schema_discovery(&builder)
        .await
        .expect("Failed to start bulk insert");

    writer
        .send_row_values(&[
            SqlValue::Int(1),
            SqlValue::String("Alice".into()),
            SqlValue::Decimal(rust_decimal::Decimal::new(12345, 2)), // 123.45
            SqlValue::Bool(true),
        ])
        .expect("Failed to send row 1");

    writer
        .send_row_values(&[
            SqlValue::Int(2),
            SqlValue::String("Bob".into()),
            SqlValue::Null,
            SqlValue::Bool(false),
        ])
        .expect("Failed to send row 2");

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, 2);

    // Verify data round-trip
    let rows = client
        .query(
            "SELECT id, name, amount, flag FROM #BulkNoDiscovery ORDER BY id",
            &[],
        )
        .await
        .expect("Query failed");

    let data: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    assert_eq!(data.len(), 2);

    assert_eq!(data[0].get::<i32>(0).unwrap(), 1);
    assert_eq!(data[0].get::<String>(1).unwrap(), "Alice");
    assert_eq!(
        data[0].get::<rust_decimal::Decimal>(2).unwrap(),
        rust_decimal::Decimal::new(12345, 2)
    );
    assert!(data[0].get::<bool>(3).unwrap());

    assert_eq!(data[1].get::<i32>(0).unwrap(), 2);
    assert_eq!(data[1].get::<String>(1).unwrap(), "Bob");
    assert_eq!(
        data[1].get::<Option<rust_decimal::Decimal>>(2).unwrap(),
        None
    );
    assert!(!data[1].get::<bool>(3).unwrap());

    client.close().await.expect("Failed to close");
}

/// MONEY / SMALLMONEY / DATETIME / SMALLDATETIME through the hand-crafted
/// COLMETADATA path. Exercises the nullable→fixed-type-ID mapping for each of
/// these types (item 1.9):
/// - MONEY NOT NULL   → 0x6E w/ length 8 collapses to 0x3C
/// - SMALLMONEY NOT NULL → 0x6E w/ length 4 collapses to 0x7A
/// - DATETIME NOT NULL   → 0x6F w/ length 8 collapses to 0x3D
/// - SMALLDATETIME NOT NULL → 0x6F w/ length 4 collapses to 0x3A
/// Plus a nullable column for each type to exercise the non-fixed path.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_without_schema_discovery_money_datetime() {
    use chrono::{NaiveDate, NaiveDateTime};
    use rust_decimal::Decimal;
    use std::str::FromStr;

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "CREATE TABLE #BulkNoDiscMoney (\
                id INT NOT NULL, \
                price MONEY NOT NULL, \
                tip SMALLMONEY NOT NULL, \
                ts DATETIME NOT NULL, \
                ts_short SMALLDATETIME NOT NULL, \
                price_nullable MONEY NULL, \
                ts_nullable DATETIME NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    let builder = BulkInsertBuilder::new("#BulkNoDiscMoney").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("price", "MONEY", 1)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("tip", "SMALLMONEY", 2)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("ts", "DATETIME", 3)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("ts_short", "SMALLDATETIME", 4)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("price_nullable", "MONEY", 5).unwrap(),
        BulkColumn::new("ts_nullable", "DATETIME", 6).unwrap(),
    ]);

    let mut writer = client
        .bulk_insert_without_schema_discovery(&builder)
        .await
        .expect("Failed to start bulk insert");

    let dt = NaiveDate::from_ymd_opt(2026, 4, 16)
        .unwrap()
        .and_hms_milli_opt(12, 34, 56, 789)
        .unwrap();
    let dt_short = NaiveDate::from_ymd_opt(2026, 4, 16)
        .unwrap()
        .and_hms_opt(12, 30, 0)
        .unwrap();

    writer
        .send_row_values(&[
            SqlValue::Int(1),
            SqlValue::Decimal(Decimal::from_str("123.4500").unwrap()),
            SqlValue::Decimal(Decimal::from_str("-7.8900").unwrap()),
            SqlValue::DateTime(dt),
            SqlValue::DateTime(dt_short),
            SqlValue::Decimal(Decimal::from_str("999.9900").unwrap()),
            SqlValue::DateTime(dt),
        ])
        .expect("row 1");

    writer
        .send_row_values(&[
            SqlValue::Int(2),
            SqlValue::Decimal(Decimal::from_str("0").unwrap()),
            SqlValue::Decimal(Decimal::from_str("0").unwrap()),
            SqlValue::DateTime(dt),
            SqlValue::DateTime(dt_short),
            SqlValue::Null,
            SqlValue::Null,
        ])
        .expect("row 2");

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, 2);

    let rows = client
        .query(
            "SELECT id, price, tip, ts, ts_short, price_nullable, ts_nullable \
             FROM #BulkNoDiscMoney ORDER BY id",
            &[],
        )
        .await
        .expect("Query failed");

    let data: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    assert_eq!(data.len(), 2);

    assert_eq!(data[0].get::<i32>(0).unwrap(), 1);
    assert_eq!(
        data[0].get::<Decimal>(1).unwrap(),
        Decimal::from_str("123.4500").unwrap()
    );
    assert_eq!(
        data[0].get::<Decimal>(2).unwrap(),
        Decimal::from_str("-7.8900").unwrap()
    );
    // DATETIME rounds to nearest 1/300th second (~3.33ms), so 789ms → 790ms
    let dt_read = data[0].get::<NaiveDateTime>(3).unwrap();
    let delta = (dt_read - dt).num_milliseconds().abs();
    assert!(delta <= 4, "DATETIME rounding >4ms: got delta={delta}");
    assert_eq!(data[0].get::<NaiveDateTime>(4).unwrap(), dt_short);
    assert_eq!(
        data[0].get::<Decimal>(5).unwrap(),
        Decimal::from_str("999.9900").unwrap()
    );
    let dt_null_read = data[0].get::<Option<NaiveDateTime>>(6).unwrap().unwrap();
    let delta_null = (dt_null_read - dt).num_milliseconds().abs();
    assert!(
        delta_null <= 4,
        "nullable DATETIME rounding >4ms: got delta={delta_null}"
    );

    assert_eq!(data[1].get::<Option<Decimal>>(5).unwrap(), None);
    assert_eq!(data[1].get::<Option<NaiveDateTime>>(6).unwrap(), None);

    client.close().await.expect("Failed to close");
}

/// DATE / TIME / DATETIME2 / DATETIMEOFFSET through the hand-crafted COLMETADATA
/// path (item 1.9). These types have no fixed-width variant in
/// `nullable_to_fixed_type`, so NOT NULL columns stay on the nullable type IDs
/// (0x28, 0x29, 0x2A, 0x2B) in COLMETADATA — verify the server accepts that.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_without_schema_discovery_temporal() {
    use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime};

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "CREATE TABLE #BulkNoDiscTemporal (\
                id INT NOT NULL, \
                d DATE NOT NULL, \
                t TIME(7) NOT NULL, \
                dt2 DATETIME2(7) NOT NULL, \
                dto DATETIMEOFFSET(7) NOT NULL, \
                d_null DATE NULL, \
                dt2_null DATETIME2(3) NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    let builder = BulkInsertBuilder::new("#BulkNoDiscTemporal").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("d", "DATE", 1)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("t", "TIME(7)", 2)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("dt2", "DATETIME2(7)", 3)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("dto", "DATETIMEOFFSET(7)", 4)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("d_null", "DATE", 5).unwrap(),
        BulkColumn::new("dt2_null", "DATETIME2(3)", 6).unwrap(),
    ]);

    let mut writer = client
        .bulk_insert_without_schema_discovery(&builder)
        .await
        .expect("Failed to start bulk insert");

    let date = NaiveDate::from_ymd_opt(2026, 4, 16).unwrap();
    let time = NaiveTime::from_hms_nano_opt(12, 34, 56, 123_456_700).unwrap();
    let dt2 = NaiveDateTime::new(date, time);
    let offset = FixedOffset::east_opt(5 * 3600).unwrap();
    let dto: DateTime<FixedOffset> = DateTime::from_naive_utc_and_offset(dt2, offset);

    writer
        .send_row_values(&[
            SqlValue::Int(1),
            SqlValue::Date(date),
            SqlValue::Time(time),
            SqlValue::DateTime(dt2),
            SqlValue::DateTimeOffset(dto),
            SqlValue::Date(date),
            SqlValue::DateTime(dt2),
        ])
        .expect("row 1");

    writer
        .send_row_values(&[
            SqlValue::Int(2),
            SqlValue::Date(NaiveDate::from_ymd_opt(1753, 1, 1).unwrap()),
            SqlValue::Time(NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
            SqlValue::DateTime(
                NaiveDate::from_ymd_opt(1, 1, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap(),
            ),
            SqlValue::DateTimeOffset(DateTime::from_naive_utc_and_offset(
                NaiveDateTime::new(
                    NaiveDate::from_ymd_opt(9999, 12, 31).unwrap(),
                    NaiveTime::from_hms_nano_opt(23, 59, 59, 999_999_900).unwrap(),
                ),
                FixedOffset::east_opt(0).unwrap(),
            )),
            SqlValue::Null,
            SqlValue::Null,
        ])
        .expect("row 2");

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, 2);

    let rows = client
        .query(
            "SELECT id, d, t, dt2, dto, d_null, dt2_null \
             FROM #BulkNoDiscTemporal ORDER BY id",
            &[],
        )
        .await
        .expect("Query failed");

    let data: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    assert_eq!(data.len(), 2);

    assert_eq!(data[0].get::<NaiveDate>(1).unwrap(), date);
    assert_eq!(data[0].get::<NaiveTime>(2).unwrap(), time);
    assert_eq!(data[0].get::<NaiveDateTime>(3).unwrap(), dt2);
    assert_eq!(data[0].get::<DateTime<FixedOffset>>(4).unwrap(), dto);
    assert_eq!(data[0].get::<Option<NaiveDate>>(5).unwrap(), Some(date));

    assert_eq!(
        data[1].get::<NaiveDate>(1).unwrap(),
        NaiveDate::from_ymd_opt(1753, 1, 1).unwrap()
    );
    assert_eq!(data[1].get::<Option<NaiveDate>>(5).unwrap(), None);
    assert_eq!(data[1].get::<Option<NaiveDateTime>>(6).unwrap(), None);

    client.close().await.expect("Failed to close");
}

/// UNIQUEIDENTIFIER through the hand-crafted COLMETADATA path (item 1.9).
/// Uses an asymmetric-bytes UUID to detect the mixed-endian byte-swap bug
/// class caught in item 1.8. Covers NOT NULL + nullable paths.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_without_schema_discovery_uuid() {
    use uuid::Uuid;

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "CREATE TABLE #BulkNoDiscUuid (\
                id INT NOT NULL, \
                ident UNIQUEIDENTIFIER NOT NULL, \
                ident_null UNIQUEIDENTIFIER NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    let builder = BulkInsertBuilder::new("#BulkNoDiscUuid").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("ident", "UNIQUEIDENTIFIER", 1)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("ident_null", "UNIQUEIDENTIFIER", 2).unwrap(),
    ]);

    let mut writer = client
        .bulk_insert_without_schema_discovery(&builder)
        .await
        .expect("Failed to start bulk insert");

    // Asymmetric bytes across all 16 positions — catches mixed-endian swap bugs
    let uid = Uuid::parse_str("12345678-1234-5678-9abc-def012345678").unwrap();

    writer
        .send_row_values(&[SqlValue::Int(1), SqlValue::Uuid(uid), SqlValue::Uuid(uid)])
        .expect("row 1");

    writer
        .send_row_values(&[
            SqlValue::Int(2),
            SqlValue::Uuid(Uuid::nil()),
            SqlValue::Null,
        ])
        .expect("row 2");

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, 2);

    let rows = client
        .query(
            "SELECT id, ident, ident_null FROM #BulkNoDiscUuid ORDER BY id",
            &[],
        )
        .await
        .expect("Query failed");

    let data: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    assert_eq!(data.len(), 2);

    assert_eq!(data[0].get::<Uuid>(1).unwrap(), uid);
    assert_eq!(data[0].get::<Option<Uuid>>(2).unwrap(), Some(uid));
    assert_eq!(data[1].get::<Uuid>(1).unwrap(), Uuid::nil());
    assert_eq!(data[1].get::<Option<Uuid>>(2).unwrap(), None);

    client.close().await.expect("Failed to close");
}

/// VARBINARY through the hand-crafted COLMETADATA path (item 1.9). Tests:
/// - VARBINARY(100) NOT NULL — small payload (100 bytes) and near-boundary (1000 bytes)
/// - VARBINARY(MAX) NULL — empty, medium (100 bytes), large 10,000 bytes (multi-packet PLP)
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_without_schema_discovery_varbinary() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "CREATE TABLE #BulkNoDiscVarbin (\
                id INT NOT NULL, \
                data_small VARBINARY(1000) NOT NULL, \
                data_max VARBINARY(MAX) NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    let builder = BulkInsertBuilder::new("#BulkNoDiscVarbin").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("data_small", "VARBINARY(1000)", 1)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("data_max", "VARBINARY(MAX)", 2).unwrap(),
    ]);

    let mut writer = client
        .bulk_insert_without_schema_discovery(&builder)
        .await
        .expect("Failed to start bulk insert");

    let small: Vec<u8> = (0u8..100u8).collect();
    let medium: Vec<u8> = (0u8..100u8).cycle().take(100).collect();
    let large: Vec<u8> = (0u8..=255u8).cycle().take(10_000).collect();

    writer
        .send_row_values(&[
            SqlValue::Int(1),
            SqlValue::Binary(small.clone().into()),
            SqlValue::Binary(medium.clone().into()),
        ])
        .expect("row 1");

    writer
        .send_row_values(&[
            SqlValue::Int(2),
            SqlValue::Binary(vec![0xFF; 1000].into()), // boundary
            SqlValue::Binary(large.clone().into()),    // PLP multi-chunk
        ])
        .expect("row 2");

    writer
        .send_row_values(&[
            SqlValue::Int(3),
            SqlValue::Binary(vec![0x00].into()),
            SqlValue::Binary(Vec::<u8>::new().into()), // empty PLP
        ])
        .expect("row 3");

    writer
        .send_row_values(&[
            SqlValue::Int(4),
            SqlValue::Binary(vec![0xAA; 42].into()),
            SqlValue::Null,
        ])
        .expect("row 4");

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, 4);

    // DATALENGTH returns INT for VARBINARY(n), BIGINT for VARBINARY(MAX)
    let rows = client
        .query(
            "SELECT id, data_small, data_max, \
                DATALENGTH(data_small) AS len_small, \
                CAST(DATALENGTH(data_max) AS INT) AS len_max \
             FROM #BulkNoDiscVarbin ORDER BY id",
            &[],
        )
        .await
        .expect("Query failed");

    let data: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    assert_eq!(data.len(), 4);

    assert_eq!(data[0].get::<Vec<u8>>(1).unwrap(), small);
    assert_eq!(data[0].get::<Vec<u8>>(2).unwrap(), medium);
    assert_eq!(data[0].get::<i32>(3).unwrap(), 100);
    assert_eq!(data[0].get::<i32>(4).unwrap(), 100);

    assert_eq!(data[1].get::<Vec<u8>>(1).unwrap(), vec![0xFF; 1000]);
    assert_eq!(data[1].get::<Vec<u8>>(2).unwrap(), large);
    assert_eq!(data[1].get::<i32>(4).unwrap(), 10_000);

    assert_eq!(data[2].get::<Vec<u8>>(1).unwrap(), vec![0x00]);
    assert_eq!(data[2].get::<Vec<u8>>(2).unwrap(), Vec::<u8>::new());
    assert_eq!(data[2].get::<i32>(4).unwrap(), 0);

    assert_eq!(data[3].get::<Vec<u8>>(1).unwrap(), vec![0xAA; 42]);
    assert_eq!(data[3].get::<Option<Vec<u8>>>(2).unwrap(), None);

    client.close().await.expect("Failed to close");
}

/// VARCHAR through the hand-crafted COLMETADATA path (item 1.9). Hand-crafted
/// COLMETADATA writes the default Latin1_General_CI_AS collation bytes —
/// verify ASCII / Latin-1 extended characters round-trip on a server whose
/// default collation is Latin-compatible. Non-Latin collation coverage belongs
/// to item 3.9's schema-discovery path; the hand-crafted path does not yet
/// propagate `with_collation()` into the COLMETADATA token (see remaining
/// work in item 1.9).
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_without_schema_discovery_varchar_latin() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "CREATE TABLE #BulkNoDiscVarchar (\
                id INT NOT NULL, \
                name VARCHAR(100) NOT NULL, \
                note VARCHAR(MAX) NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    let builder = BulkInsertBuilder::new("#BulkNoDiscVarchar").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("name", "VARCHAR(100)", 1)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("note", "VARCHAR(MAX)", 2).unwrap(),
    ]);

    let mut writer = client
        .bulk_insert_without_schema_discovery(&builder)
        .await
        .expect("Failed to start bulk insert");

    writer
        .send_row_values(&[
            SqlValue::Int(1),
            SqlValue::String("Alice".into()),
            SqlValue::String("ascii only".into()),
        ])
        .expect("row 1");

    // Latin-1 extended (é, ñ, ü, ß all present in Windows-1252)
    writer
        .send_row_values(&[
            SqlValue::Int(2),
            SqlValue::String("naïve résumé".into()),
            SqlValue::String("grüße über straße".repeat(200)),
        ])
        .expect("row 2");

    writer
        .send_row_values(&[
            SqlValue::Int(3),
            SqlValue::String("".into()),
            SqlValue::Null,
        ])
        .expect("row 3");

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, 3);

    let rows = client
        .query(
            "SELECT id, name, note, DATALENGTH(name) AS name_len \
             FROM #BulkNoDiscVarchar ORDER BY id",
            &[],
        )
        .await
        .expect("Query failed");

    let data: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    assert_eq!(data.len(), 3);

    assert_eq!(data[0].get::<String>(1).unwrap(), "Alice");
    assert_eq!(data[0].get::<String>(2).unwrap(), "ascii only");
    assert_eq!(data[0].get::<i32>(3).unwrap(), 5);

    assert_eq!(data[1].get::<String>(1).unwrap(), "naïve résumé");
    // "naïve résumé" is 12 bytes in Windows-1252 (each of é, ï is 1 byte)
    assert_eq!(data[1].get::<i32>(3).unwrap(), 12);

    assert_eq!(data[2].get::<String>(1).unwrap(), "");
    assert_eq!(data[2].get::<Option<String>>(2).unwrap(), None);
    assert_eq!(data[2].get::<i32>(3).unwrap(), 0);

    client.close().await.expect("Failed to close");
}

// =============================================================================
// Expanded Type Coverage (work item 3.5)
// =============================================================================

/// MONEY and SMALLMONEY round-trip. MONEY uses a distinct on-wire format
/// (signed int scaled by 10_000) that is NOT the same as DECIMAL encoding.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_money_and_smallmoney() {
    use rust_decimal::Decimal;
    use std::str::FromStr;

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "CREATE TABLE #BulkMoney (\
                id INT NOT NULL, \
                price MONEY NOT NULL, \
                tip SMALLMONEY NOT NULL, \
                adjustment MONEY NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    let builder = BulkInsertBuilder::new("#BulkMoney").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("price", "MONEY", 1)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("tip", "SMALLMONEY", 2)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("adjustment", "MONEY", 3).unwrap(),
    ]);

    let mut writer = client
        .bulk_insert(&builder)
        .await
        .expect("Failed to start bulk insert");

    // Positive values with fractional cents (MONEY supports 4 decimal places)
    writer
        .send_row_values(&[
            SqlValue::Int(1),
            SqlValue::Decimal(Decimal::from_str("123.4500").unwrap()),
            SqlValue::Decimal(Decimal::from_str("12.3400").unwrap()),
            SqlValue::Decimal(Decimal::from_str("-7.8900").unwrap()),
        ])
        .expect("row 1");

    // Large MONEY value (fits in i64 / 10_000)
    writer
        .send_row_values(&[
            SqlValue::Int(2),
            SqlValue::Decimal(Decimal::from_str("999999999.9999").unwrap()),
            SqlValue::Decimal(Decimal::from_str("214748.3647").unwrap()), // SMALLMONEY max
            SqlValue::Null,
        ])
        .expect("row 2");

    // Zero and negative edge cases
    writer
        .send_row_values(&[
            SqlValue::Int(3),
            SqlValue::Decimal(Decimal::from_str("0").unwrap()),
            SqlValue::Decimal(Decimal::from_str("-214748.3648").unwrap()), // SMALLMONEY min
            SqlValue::Decimal(Decimal::from_str("-922337203685477.5808").unwrap()), // MONEY min
        ])
        .expect("row 3");

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, 3);

    let rows = client
        .query(
            "SELECT id, price, tip, adjustment FROM #BulkMoney ORDER BY id",
            &[],
        )
        .await
        .expect("Query failed");

    let data: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    assert_eq!(data.len(), 3);

    assert_eq!(data[0].get::<i32>(0).unwrap(), 1);
    assert_eq!(
        data[0].get::<Decimal>(1).unwrap(),
        Decimal::from_str("123.4500").unwrap()
    );
    assert_eq!(
        data[0].get::<Decimal>(2).unwrap(),
        Decimal::from_str("12.3400").unwrap()
    );
    assert_eq!(
        data[0].get::<Decimal>(3).unwrap(),
        Decimal::from_str("-7.8900").unwrap()
    );

    assert_eq!(
        data[1].get::<Decimal>(1).unwrap(),
        Decimal::from_str("999999999.9999").unwrap()
    );
    assert_eq!(
        data[1].get::<Decimal>(2).unwrap(),
        Decimal::from_str("214748.3647").unwrap()
    );
    assert_eq!(data[1].get::<Option<Decimal>>(3).unwrap(), None);

    assert_eq!(data[2].get::<Decimal>(1).unwrap(), Decimal::ZERO);
    assert_eq!(
        data[2].get::<Decimal>(2).unwrap(),
        Decimal::from_str("-214748.3648").unwrap()
    );
    assert_eq!(
        data[2].get::<Decimal>(3).unwrap(),
        Decimal::from_str("-922337203685477.5808").unwrap()
    );

    client.close().await.expect("Failed to close");
}

/// SMALLDATETIME round-trip. Unblocked by fix for work item 1.2 (SMALLDATETIME
/// type ID collision). SMALLDATETIME has minute precision and range 1900-2079.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_smalldatetime() {
    use chrono::{NaiveDate, NaiveDateTime};

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "CREATE TABLE #BulkSmallDt (\
                id INT NOT NULL, \
                ts SMALLDATETIME NOT NULL, \
                maybe SMALLDATETIME NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    let builder = BulkInsertBuilder::new("#BulkSmallDt").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("ts", "SMALLDATETIME", 1)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("maybe", "SMALLDATETIME", 2).unwrap(),
    ]);

    let mut writer = client
        .bulk_insert(&builder)
        .await
        .expect("Failed to start bulk insert");

    // SMALLDATETIME has minute precision — use minute-aligned times
    let dt1 = NaiveDate::from_ymd_opt(2026, 4, 15)
        .unwrap()
        .and_hms_opt(10, 30, 0)
        .unwrap();
    let dt2 = NaiveDate::from_ymd_opt(1900, 1, 1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();

    writer
        .send_row_values(&[
            SqlValue::Int(1),
            SqlValue::DateTime(dt1),
            SqlValue::DateTime(dt2),
        ])
        .expect("row 1");

    writer
        .send_row_values(&[SqlValue::Int(2), SqlValue::DateTime(dt2), SqlValue::Null])
        .expect("row 2");

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, 2);

    let rows = client
        .query("SELECT id, ts, maybe FROM #BulkSmallDt ORDER BY id", &[])
        .await
        .expect("Query failed");

    let data: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    assert_eq!(data.len(), 2);

    assert_eq!(data[0].get::<NaiveDateTime>(1).unwrap(), dt1);
    assert_eq!(data[0].get::<NaiveDateTime>(2).unwrap(), dt2);
    assert_eq!(data[1].get::<NaiveDateTime>(1).unwrap(), dt2);
    assert_eq!(data[1].get::<Option<NaiveDateTime>>(2).unwrap(), None);

    client.close().await.expect("Failed to close");
}

/// DATETIME vs DATETIME2 precision handling. DATETIME has ~3.33ms precision
/// (actually 1/300th second rounding) while DATETIME2(7) has 100ns precision.
/// This test verifies that sub-second values are preserved through bulk insert.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_datetime_vs_datetime2_precision() {
    use chrono::{NaiveDate, NaiveDateTime};

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "CREATE TABLE #BulkDtPrecision (\
                id INT NOT NULL, \
                dt DATETIME NOT NULL, \
                dt2 DATETIME2(7) NOT NULL, \
                dt2_3 DATETIME2(3) NOT NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    let builder = BulkInsertBuilder::new("#BulkDtPrecision").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("dt", "DATETIME", 1)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("dt2", "DATETIME2(7)", 2)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("dt2_3", "DATETIME2(3)", 3)
            .unwrap()
            .with_nullable(false),
    ]);

    let mut writer = client
        .bulk_insert(&builder)
        .await
        .expect("Failed to start bulk insert");

    // DATETIME rounds to nearest 3.33ms — use 123ms which SQL Server rounds to 123
    let dt = NaiveDate::from_ymd_opt(2026, 4, 15)
        .unwrap()
        .and_hms_milli_opt(10, 30, 45, 123)
        .unwrap();

    // DATETIME2(7) supports 100ns resolution
    let dt2 = NaiveDate::from_ymd_opt(2026, 4, 15)
        .unwrap()
        .and_hms_nano_opt(10, 30, 45, 123_456_700)
        .unwrap();

    // DATETIME2(3) rounds to millisecond
    let dt2_3 = NaiveDate::from_ymd_opt(2026, 4, 15)
        .unwrap()
        .and_hms_milli_opt(10, 30, 45, 789)
        .unwrap();

    writer
        .send_row_values(&[
            SqlValue::Int(1),
            SqlValue::DateTime(dt),
            SqlValue::DateTime(dt2),
            SqlValue::DateTime(dt2_3),
        ])
        .expect("row 1");

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, 1);

    let rows = client
        .query(
            "SELECT id, dt, dt2, dt2_3 FROM #BulkDtPrecision ORDER BY id",
            &[],
        )
        .await
        .expect("Query failed");

    let data: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    assert_eq!(data.len(), 1);

    // DATETIME rounds to 3.33ms grid — 123ms should round to 123ms
    let got_dt: NaiveDateTime = data[0].get(1).unwrap();
    assert_eq!(got_dt.date(), dt.date());
    assert_eq!(
        (got_dt.time() - dt.time()).num_milliseconds().abs(),
        0,
        "DATETIME should preserve 123ms exactly"
    );

    // DATETIME2(7) preserves 100ns resolution — exact match
    let got_dt2: NaiveDateTime = data[0].get(2).unwrap();
    assert_eq!(got_dt2, dt2, "DATETIME2(7) should round-trip exactly");

    // DATETIME2(3) truncates to millisecond
    let got_dt2_3: NaiveDateTime = data[0].get(3).unwrap();
    assert_eq!(
        got_dt2_3, dt2_3,
        "DATETIME2(3) at ms precision should round-trip"
    );

    client.close().await.expect("Failed to close");
}

/// Mixed temporal column orderings — DATE → TIME → DATETIME2 → DATETIMEOFFSET
/// with non-temporal columns interleaved. Catches bugs where column ordering
/// affects the encoding offsets.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_mixed_column_ordering() {
    use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime};

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "CREATE TABLE #BulkMixedOrder (\
                id INT NOT NULL, \
                d DATE NOT NULL, \
                label NVARCHAR(50) NOT NULL, \
                t TIME(3) NOT NULL, \
                flag BIT NOT NULL, \
                dt2 DATETIME2(5) NOT NULL, \
                qty INT NOT NULL, \
                dto DATETIMEOFFSET(4) NOT NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    let builder = BulkInsertBuilder::new("#BulkMixedOrder").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("d", "DATE", 1)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("label", "NVARCHAR(50)", 2)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("t", "TIME(3)", 3)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("flag", "BIT", 4)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("dt2", "DATETIME2(5)", 5)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("qty", "INT", 6)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("dto", "DATETIMEOFFSET(4)", 7)
            .unwrap()
            .with_nullable(false),
    ]);

    let mut writer = client
        .bulk_insert(&builder)
        .await
        .expect("Failed to start bulk insert");

    let d = NaiveDate::from_ymd_opt(2026, 4, 15).unwrap();
    let t = NaiveTime::from_hms_milli_opt(14, 30, 15, 500).unwrap();
    // DATETIME2(5) has 10µs precision — use a 10µs-aligned nanosecond value.
    let dt2 = NaiveDateTime::new(d, NaiveTime::from_hms_nano_opt(14, 30, 15, 20_000).unwrap());
    let offset = FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap(); // +05:30
    // DATETIMEOFFSET(4) has 100µs precision — use 100µs-aligned microseconds.
    let dto: DateTime<FixedOffset> = DateTime::from_naive_utc_and_offset(
        NaiveDateTime::new(d, NaiveTime::from_hms_micro_opt(9, 0, 0, 123_400).unwrap()),
        offset,
    );

    writer
        .send_row_values(&[
            SqlValue::Int(42),
            SqlValue::Date(d),
            SqlValue::String("hello".into()),
            SqlValue::Time(t),
            SqlValue::Bool(true),
            SqlValue::DateTime(dt2),
            SqlValue::Int(-100),
            SqlValue::DateTimeOffset(dto),
        ])
        .expect("row 1");

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, 1);

    let rows = client
        .query(
            "SELECT id, d, label, t, flag, dt2, qty, dto FROM #BulkMixedOrder",
            &[],
        )
        .await
        .expect("Query failed");

    let data: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    assert_eq!(data.len(), 1);

    assert_eq!(data[0].get::<i32>(0).unwrap(), 42);
    assert_eq!(data[0].get::<NaiveDate>(1).unwrap(), d);
    assert_eq!(data[0].get::<String>(2).unwrap(), "hello");
    assert_eq!(data[0].get::<NaiveTime>(3).unwrap(), t);
    assert!(data[0].get::<bool>(4).unwrap());
    assert_eq!(data[0].get::<NaiveDateTime>(5).unwrap(), dt2);
    assert_eq!(data[0].get::<i32>(6).unwrap(), -100);
    assert_eq!(data[0].get::<DateTime<FixedOffset>>(7).unwrap(), dto);

    client.close().await.expect("Failed to close");
}

/// NULL values across every supported bulk type. Catches encoding gaps where
/// a particular type's NULL path is wrong. Items from the existing null test
/// only covered INT and NVARCHAR.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_all_types_null() {
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
    use rust_decimal::Decimal;

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "CREATE TABLE #BulkAllNulls (\
                id INT NOT NULL, \
                b BIT NULL, \
                ti TINYINT NULL, \
                si SMALLINT NULL, \
                i INT NULL, \
                bi BIGINT NULL, \
                rl REAL NULL, \
                fl FLOAT NULL, \
                dec_c DECIMAL(10,2) NULL, \
                mny MONEY NULL, \
                smny SMALLMONEY NULL, \
                vc VARCHAR(50) NULL, \
                nvc NVARCHAR(50) NULL, \
                vb VARBINARY(50) NULL, \
                d DATE NULL, \
                t TIME(3) NULL, \
                dt DATETIME NULL, \
                sdt SMALLDATETIME NULL, \
                dt2 DATETIME2(7) NULL, \
                dto DATETIMEOFFSET(7) NULL, \
                g UNIQUEIDENTIFIER NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    let builder = BulkInsertBuilder::new("#BulkAllNulls").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("b", "BIT", 1).unwrap(),
        BulkColumn::new("ti", "TINYINT", 2).unwrap(),
        BulkColumn::new("si", "SMALLINT", 3).unwrap(),
        BulkColumn::new("i", "INT", 4).unwrap(),
        BulkColumn::new("bi", "BIGINT", 5).unwrap(),
        BulkColumn::new("rl", "REAL", 6).unwrap(),
        BulkColumn::new("fl", "FLOAT", 7).unwrap(),
        BulkColumn::new("dec_c", "DECIMAL(10,2)", 8).unwrap(),
        BulkColumn::new("mny", "MONEY", 9).unwrap(),
        BulkColumn::new("smny", "SMALLMONEY", 10).unwrap(),
        BulkColumn::new("vc", "VARCHAR(50)", 11).unwrap(),
        BulkColumn::new("nvc", "NVARCHAR(50)", 12).unwrap(),
        BulkColumn::new("vb", "VARBINARY(50)", 13).unwrap(),
        BulkColumn::new("d", "DATE", 14).unwrap(),
        BulkColumn::new("t", "TIME(3)", 15).unwrap(),
        BulkColumn::new("dt", "DATETIME", 16).unwrap(),
        BulkColumn::new("sdt", "SMALLDATETIME", 17).unwrap(),
        BulkColumn::new("dt2", "DATETIME2(7)", 18).unwrap(),
        BulkColumn::new("dto", "DATETIMEOFFSET(7)", 19).unwrap(),
        BulkColumn::new("g", "UNIQUEIDENTIFIER", 20).unwrap(),
    ]);

    let mut writer = client
        .bulk_insert(&builder)
        .await
        .expect("Failed to start bulk insert");

    // Row with all NULLs except id
    let nulls = [
        SqlValue::Int(1),
        SqlValue::Null,
        SqlValue::Null,
        SqlValue::Null,
        SqlValue::Null,
        SqlValue::Null,
        SqlValue::Null,
        SqlValue::Null,
        SqlValue::Null,
        SqlValue::Null,
        SqlValue::Null,
        SqlValue::Null,
        SqlValue::Null,
        SqlValue::Null,
        SqlValue::Null,
        SqlValue::Null,
        SqlValue::Null,
        SqlValue::Null,
        SqlValue::Null,
        SqlValue::Null,
        SqlValue::Null,
    ];
    writer.send_row_values(&nulls).expect("row 1 all nulls");

    // Row with all values set
    let d = NaiveDate::from_ymd_opt(2026, 4, 15).unwrap();
    let t = NaiveTime::from_hms_opt(12, 0, 0).unwrap();
    let dt = NaiveDateTime::new(d, NaiveTime::from_hms_milli_opt(12, 0, 0, 500).unwrap());
    let sdt = NaiveDateTime::new(d, NaiveTime::from_hms_opt(12, 0, 0).unwrap());
    let dt2 = NaiveDateTime::new(d, NaiveTime::from_hms_nano_opt(12, 0, 0, 1234567).unwrap());
    let offset = chrono::FixedOffset::east_opt(0).unwrap();
    let dto = chrono::DateTime::<chrono::FixedOffset>::from_naive_utc_and_offset(dt2, offset);
    let guid = uuid::Uuid::from_u128(0x1234_5678_1234_5678_1234_5678_1234_5678);

    writer
        .send_row_values(&[
            SqlValue::Int(2),
            SqlValue::Bool(true),
            SqlValue::TinyInt(42),
            SqlValue::SmallInt(-42),
            SqlValue::Int(42),
            SqlValue::BigInt(42_000_000_000),
            SqlValue::Float(std::f32::consts::PI),
            SqlValue::Double(std::f64::consts::E),
            SqlValue::Decimal(Decimal::new(1234, 2)),
            SqlValue::Decimal(Decimal::new(98765432, 4)),
            SqlValue::Decimal(Decimal::new(12345, 4)),
            SqlValue::String("vc".into()),
            SqlValue::String("nvc".into()),
            SqlValue::Binary(bytes::Bytes::from(vec![0xDE, 0xAD, 0xBE, 0xEF])),
            SqlValue::Date(d),
            SqlValue::Time(t),
            SqlValue::DateTime(dt),
            SqlValue::DateTime(sdt),
            SqlValue::DateTime(dt2),
            SqlValue::DateTimeOffset(dto),
            SqlValue::Uuid(guid),
        ])
        .expect("row 2 values");

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, 2);

    let rows = client
        .query(
            "SELECT id, b, ti, si, i, bi, rl, fl, dec_c, mny, smny, \
                    vc, nvc, vb, d, t, dt, sdt, dt2, dto, g \
             FROM #BulkAllNulls ORDER BY id",
            &[],
        )
        .await
        .expect("Query failed");

    let data: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    assert_eq!(data.len(), 2);

    // Row 1: all NULL except id
    assert_eq!(data[0].get::<i32>(0).unwrap(), 1);
    for idx in 1..=20 {
        assert!(data[0].is_null(idx), "column {idx} should be NULL in row 1");
    }

    // Row 2: all values set
    assert_eq!(data[1].get::<i32>(0).unwrap(), 2);
    assert!(data[1].get::<bool>(1).unwrap());
    assert_eq!(data[1].get::<u8>(2).unwrap(), 42);
    assert_eq!(data[1].get::<i16>(3).unwrap(), -42);
    assert_eq!(data[1].get::<i32>(4).unwrap(), 42);
    assert_eq!(data[1].get::<i64>(5).unwrap(), 42_000_000_000);

    client.close().await.expect("Failed to close");
}

/// DATETIME (legacy 8-byte format) round-trip covering pre-1900 dates and
/// sub-second precision that DATETIME2 handles differently.
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_datetime_legacy_edge_cases() {
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "CREATE TABLE #BulkDtLegacy (\
                id INT NOT NULL, \
                dt DATETIME NOT NULL, \
                dtn DATETIME NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    let builder = BulkInsertBuilder::new("#BulkDtLegacy").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("dt", "DATETIME", 1)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("dtn", "DATETIME", 2).unwrap(),
    ]);

    let mut writer = client
        .bulk_insert(&builder)
        .await
        .expect("Failed to start bulk insert");

    // DATETIME range: 1753-01-01 to 9999-12-31, 3.33ms resolution
    let dt_min = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(1753, 1, 1).unwrap(),
        NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
    );
    // 500ms lands exactly on a 3.33ms tick boundary (150 ticks)
    let dt_with_ms = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2026, 4, 15).unwrap(),
        NaiveTime::from_hms_milli_opt(14, 30, 45, 500).unwrap(),
    );
    let dt_midnight = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2000, 1, 1).unwrap(),
        NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
    );

    writer
        .send_row_values(&[
            SqlValue::Int(1),
            SqlValue::DateTime(dt_min),
            SqlValue::DateTime(dt_with_ms),
        ])
        .expect("row 1");
    writer
        .send_row_values(&[
            SqlValue::Int(2),
            SqlValue::DateTime(dt_midnight),
            SqlValue::Null,
        ])
        .expect("row 2");

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, 2);

    let rows = client
        .query("SELECT id, dt, dtn FROM #BulkDtLegacy ORDER BY id", &[])
        .await
        .expect("Query failed");

    let data: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    assert_eq!(data.len(), 2);

    assert_eq!(data[0].get::<NaiveDateTime>(1).unwrap(), dt_min);
    assert_eq!(data[0].get::<NaiveDateTime>(2).unwrap(), dt_with_ms);
    assert_eq!(data[1].get::<NaiveDateTime>(1).unwrap(), dt_midnight);
    assert_eq!(data[1].get::<Option<NaiveDateTime>>(2).unwrap(), None);

    client.close().await.expect("Failed to close");
}

/// VARCHAR columns must encode values in the server's collation code page, not
/// UTF-16. Before 1.10 was fixed, `SqlValue::String("abc")` would be written as
/// the six-byte UTF-16 sequence `"a\0b\0c\0"` into a single-byte VARCHAR column,
/// doubling DATALENGTH and inserting NUL padding chars. This test verifies the
/// plain ASCII case (DATALENGTH(col) == char count) for VARCHAR, CHAR and
/// VARCHAR(MAX).
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_varchar_ascii() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "CREATE TABLE #BulkVc (\
                id INT NOT NULL, \
                vc VARCHAR(100) NOT NULL, \
                ch CHAR(5) NOT NULL, \
                vcm VARCHAR(MAX) NOT NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    let builder = BulkInsertBuilder::new("#BulkVc").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("vc", "VARCHAR(100)", 1)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("ch", "CHAR(5)", 2)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("vcm", "VARCHAR(MAX)", 3)
            .unwrap()
            .with_nullable(false),
    ]);

    // A multi-packet VARCHAR(MAX) exercise: 5000 ASCII bytes spans two 4096-byte
    // packets so we catch PLP chunk boundaries as well.
    let long = "abcdefghij".repeat(500);

    let mut writer = client
        .bulk_insert(&builder)
        .await
        .expect("Failed to start bulk insert");

    writer
        .send_row_values(&[
            SqlValue::Int(1),
            SqlValue::String("hello".into()),
            SqlValue::String("world".into()),
            SqlValue::String(long.clone()),
        ])
        .expect("row 1");
    writer
        .send_row_values(&[
            SqlValue::Int(2),
            SqlValue::String(String::new()),
            SqlValue::String("     ".into()),
            SqlValue::String("x".into()),
        ])
        .expect("row 2");

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, 2);

    // DATALENGTH returns INT for non-MAX types and BIGINT for VARCHAR(MAX);
    // cast the MAX column to INT so both round-trip as i32.
    let rows = client
        .query(
            "SELECT id, vc, DATALENGTH(vc), ch, DATALENGTH(ch), vcm, CAST(DATALENGTH(vcm) AS INT) \
             FROM #BulkVc ORDER BY id",
            &[],
        )
        .await
        .expect("Query failed");

    let data: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    assert_eq!(data.len(), 2);

    // Row 1 — ASCII strings, DATALENGTH equals char count for VARCHAR/CHAR,
    // and the VARCHAR(MAX) column round-trips the multi-packet payload byte-for-byte.
    assert_eq!(data[0].get::<i32>(0).unwrap(), 1);
    assert_eq!(data[0].get::<String>(1).unwrap(), "hello");
    assert_eq!(
        data[0].get::<i32>(2).unwrap(),
        5,
        "VARCHAR 'hello' should be 5 bytes, not 10"
    );
    assert_eq!(data[0].get::<String>(3).unwrap(), "world");
    assert_eq!(
        data[0].get::<i32>(4).unwrap(),
        5,
        "CHAR(5) 'world' should be 5 bytes"
    );
    assert_eq!(data[0].get::<String>(5).unwrap(), long);
    assert_eq!(
        data[0].get::<i32>(6).unwrap(),
        5_000,
        "VARCHAR(MAX) 5000-byte payload"
    );

    // Row 2 — empty string and single char exercise the edge cases that
    // previously dropped into the UTF-16 path.
    assert_eq!(data[1].get::<String>(1).unwrap(), "");
    assert_eq!(data[1].get::<i32>(2).unwrap(), 0);
    // CHAR(5) is blank-padded; server returns "     ".
    assert_eq!(data[1].get::<String>(3).unwrap(), "     ");
    assert_eq!(data[1].get::<i32>(4).unwrap(), 5);
    assert_eq!(data[1].get::<String>(5).unwrap(), "x");
    assert_eq!(data[1].get::<i32>(6).unwrap(), 1);

    client.close().await.expect("Failed to close");
}

/// VARCHAR under a non-ASCII server collation must transcode through the
/// collation's code page. The default mssql-test container collation is
/// SQL_Latin1_General_CP1_CI_AS (Windows-1252), so characters in the 0x80–0xFF
/// range have distinct single-byte encodings that differ from UTF-16. This test
/// verifies they round-trip intact (0xE9 'é' → byte 0xE9 on wire → decoded back
/// to 'é' on read).
#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_varchar_latin1_extended() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "CREATE TABLE #BulkVcExt (id INT NOT NULL, vc VARCHAR(50) NOT NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    let builder = BulkInsertBuilder::new("#BulkVcExt").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("vc", "VARCHAR(50)", 1)
            .unwrap()
            .with_nullable(false),
    ]);

    // "café" — 4 chars, 4 bytes in Windows-1252, 5 bytes in UTF-8, 8 bytes in UTF-16.
    // The wrong-encoding bug would blow up the length prefix.
    let accented = "café";
    let german = "grüße";
    let mixed = "naïve résumé";

    let mut writer = client
        .bulk_insert(&builder)
        .await
        .expect("Failed to start bulk insert");

    writer
        .send_row_values(&[SqlValue::Int(1), SqlValue::String(accented.into())])
        .expect("row 1");
    writer
        .send_row_values(&[SqlValue::Int(2), SqlValue::String(german.into())])
        .expect("row 2");
    writer
        .send_row_values(&[SqlValue::Int(3), SqlValue::String(mixed.into())])
        .expect("row 3");

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, 3);

    let rows = client
        .query(
            "SELECT id, vc, DATALENGTH(vc) FROM #BulkVcExt ORDER BY id",
            &[],
        )
        .await
        .expect("Query failed");

    let data: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    assert_eq!(data.len(), 3);

    assert_eq!(data[0].get::<String>(1).unwrap(), accented);
    assert_eq!(
        data[0].get::<i32>(2).unwrap(),
        4,
        "'café' is 4 bytes in Windows-1252"
    );
    assert_eq!(data[1].get::<String>(1).unwrap(), german);
    assert_eq!(
        data[1].get::<i32>(2).unwrap(),
        5,
        "'grüße' is 5 bytes in Windows-1252"
    );
    assert_eq!(data[2].get::<String>(1).unwrap(), mixed);
    assert_eq!(
        data[2].get::<i32>(2).unwrap(),
        12,
        "'naïve résumé' is 12 bytes in Windows-1252"
    );

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_rejects_text_column_from_server_metadata() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "CREATE TABLE #BulkRejText (id INT NOT NULL, body TEXT NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    let builder = BulkInsertBuilder::new("#BulkRejText").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("body", "VARCHAR(MAX)", 1).unwrap(),
    ]);

    let msg = match client.bulk_insert(&builder).await {
        Ok(_writer) => panic!("bulk_insert should reject TEXT column reported by the server"),
        Err(e) => e.to_string(),
    };
    assert!(
        msg.contains("TEXT"),
        "error should mention TEXT, got: {msg}"
    );
    assert!(
        msg.contains("VARCHAR(MAX)"),
        "error should redirect to VARCHAR(MAX), got: {msg}"
    );
    assert!(
        msg.contains("deprecated"),
        "error should mention deprecation, got: {msg}"
    );

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_rejects_ntext_column_from_server_metadata() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "CREATE TABLE #BulkRejNtext (id INT NOT NULL, body NTEXT NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    let builder = BulkInsertBuilder::new("#BulkRejNtext").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("body", "NVARCHAR(MAX)", 1).unwrap(),
    ]);

    let msg = match client.bulk_insert(&builder).await {
        Ok(_writer) => panic!("bulk_insert should reject NTEXT column reported by the server"),
        Err(e) => e.to_string(),
    };
    assert!(
        msg.contains("NTEXT"),
        "error should mention NTEXT, got: {msg}"
    );
    assert!(
        msg.contains("NVARCHAR(MAX)"),
        "error should redirect to NVARCHAR(MAX), got: {msg}"
    );

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_bulk_insert_rejects_image_column_from_server_metadata() {
    let config = get_test_config().expect("SQL Server config required");
    let mut client = Client::connect(config).await.expect("Failed to connect");

    client
        .execute(
            "CREATE TABLE #BulkRejImage (id INT NOT NULL, blob IMAGE NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    let builder = BulkInsertBuilder::new("#BulkRejImage").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0)
            .unwrap()
            .with_nullable(false),
        BulkColumn::new("blob", "VARBINARY(MAX)", 1).unwrap(),
    ]);

    let msg = match client.bulk_insert(&builder).await {
        Ok(_writer) => panic!("bulk_insert should reject IMAGE column reported by the server"),
        Err(e) => e.to_string(),
    };
    assert!(
        msg.contains("IMAGE"),
        "error should mention IMAGE, got: {msg}"
    );
    assert!(
        msg.contains("VARBINARY(MAX)"),
        "error should redirect to VARBINARY(MAX), got: {msg}"
    );
    assert!(
        msg.contains("deprecated"),
        "error should mention deprecation, got: {msg}"
    );

    client.close().await.expect("Failed to close");
}
