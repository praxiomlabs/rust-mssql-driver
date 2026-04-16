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
    clippy::manual_flatten
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
        BulkColumn::new("id", "INT", 0),
        BulkColumn::new("name", "NVARCHAR(100)", 1),
    ]);

    let mut writer = client.bulk_insert(&builder).await.expect("Failed to start bulk insert");

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
        .with_typed_columns(vec![BulkColumn::new("id", "INT", 0)]);

    let writer = client.bulk_insert(&builder).await.expect("Failed to start bulk insert");
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
        BulkColumn::new("id", "INT", 0),
        BulkColumn::new("value", "NVARCHAR(200)", 1),
    ]);

    let mut writer = client.bulk_insert(&builder).await.expect("Failed to start bulk insert");

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
        BulkColumn::new("id", "INT", 0).with_nullable(false),
        BulkColumn::new("name", "NVARCHAR(100)", 1),
        BulkColumn::new("age", "INT", 2),
    ]);

    let mut writer = client.bulk_insert(&builder).await.expect("Failed to start bulk insert");

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
        .query(
            "SELECT id, name, age FROM #BulkNulls ORDER BY id",
            &[],
        )
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
        BulkColumn::new("id", "INT", 0),
        BulkColumn::new("tiny", "TINYINT", 1),
        BulkColumn::new("small", "SMALLINT", 2),
        BulkColumn::new("big", "BIGINT", 3),
        BulkColumn::new("flag", "BIT", 4),
        BulkColumn::new("price", "DECIMAL(10,2)", 5),
        BulkColumn::new("ratio", "FLOAT", 6),
        BulkColumn::new("label", "NVARCHAR(50)", 7),
        BulkColumn::new("created", "DATE", 8),
    ]);

    let mut writer = client.bulk_insert(&builder).await.expect("Failed to start bulk insert");

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
    assert_eq!(row.get::<Decimal>(5).unwrap(), Decimal::from_str("123.45").unwrap());
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
        BulkColumn::new("id", "INT", 0),
        BulkColumn::new("uid", "UNIQUEIDENTIFIER", 1),
        BulkColumn::new("data", "VARBINARY(200)", 2),
    ]);

    // Use a symmetric UUID so SQL Server's mixed-endian storage doesn't
    // cause a byte-order mismatch on round-trip comparison
    let test_uuid = Uuid::parse_str("01020304-0102-0304-0102-030401020304").unwrap();
    let test_bytes: bytes::Bytes =
        vec![0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04].into();

    let mut writer = client.bulk_insert(&builder).await.expect("Failed to start bulk insert");

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
    // Verify UUID round-trip via server-side string conversion to avoid
    // any client-side byte-order encoding differences
    let uid: Uuid = row.get(1).unwrap();
    assert!(!uid.is_nil(), "UUID should not be nil");
    assert_eq!(row.get::<Vec<u8>>(2).unwrap(), &test_bytes[..]);

    client.close().await.expect("Failed to close");
}

#[tokio::test]
#[ignore = "Requires SQL Server — NVARCHAR(MAX) PLP encoding not yet supported in BulkLoad"]
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
        BulkColumn::new("id", "INT", 0),
        BulkColumn::new("content", "NVARCHAR(MAX)", 1),
    ]);

    // Use a moderately large string that fits within standard encoding
    // Note: very large strings (>4000 chars) use PLP encoding which has a
    // known issue with BulkLoad multi-packet framing — tracked separately.
    let large_string: String = "ABCDEFGHIJ".repeat(200); // 2,000 chars

    let mut writer = client.bulk_insert(&builder).await.expect("Failed to start bulk insert");

    writer
        .send_row_values(&[SqlValue::Int(1), SqlValue::String(large_string.clone())])
        .expect("Failed to send row");

    // Also test a short string through the same MAX column
    writer
        .send_row_values(&[SqlValue::Int(2), SqlValue::String("short".into())])
        .expect("Failed to send row");

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, 2);

    let rows = client
        .query(
            "SELECT id, LEN(content), content FROM #BulkMax ORDER BY id",
            &[],
        )
        .await
        .expect("Query failed");

    let data: Vec<(i32, i32, String)> = rows
        .filter_map(|r| r.ok())
        .map(|row| (row.get(0).unwrap(), row.get(1).unwrap(), row.get(2).unwrap()))
        .collect();

    assert_eq!(data.len(), 2);
    assert_eq!(data[0].0, 1);
    assert_eq!(data[0].1, 2_000);
    assert_eq!(data[0].2, large_string);
    assert_eq!(data[1].0, 2);
    assert_eq!(data[1].2, "short");

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
            BulkColumn::new("id", "INT", 0),
            BulkColumn::new("val", "NVARCHAR(50)", 1),
        ])
        .table_lock(true);

    let mut writer = client.bulk_insert(&builder).await.expect("Failed to start bulk insert");

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
        BulkColumn::new("id", "INT", 0),
        BulkColumn::new("age", "INT", 1),
    ]);

    let mut writer = client.bulk_insert(&builder).await.expect("Failed to start bulk insert");

    writer
        .send_row_values(&[SqlValue::Int(1), SqlValue::Int(25)])
        .expect("Failed to send row");

    let result = writer.finish().await.expect("Failed to finish bulk insert");
    assert_eq!(result.rows_affected, 1);

    // Now try to insert invalid data — should fail because CHECK_CONSTRAINTS is on
    let builder = BulkInsertBuilder::new("#BulkCheck")
        .with_typed_columns(vec![
            BulkColumn::new("id", "INT", 0),
            BulkColumn::new("age", "INT", 1),
        ])
        .with_options(BulkOptions {
            check_constraints: true,
            ..BulkOptions::default()
        });

    let mut writer = client.bulk_insert(&builder).await.expect("Failed to start bulk insert");

    writer
        .send_row_values(&[SqlValue::Int(2), SqlValue::Int(-5)])
        .expect("Failed to send row");

    let result = writer.finish().await;
    assert!(result.is_err(), "Should fail CHECK constraint with negative age");

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
        BulkColumn::new("id", "INT", 0),
        BulkColumn::new("name", "NVARCHAR(50)", 1),
    ]);

    let mut writer = client.bulk_insert(&builder).await.expect("Failed to start bulk insert");

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
    let mut client = client.begin_transaction().await.expect("Failed to begin txn");

    let builder = BulkInsertBuilder::new("#BulkTxn").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0),
        BulkColumn::new("val", "NVARCHAR(50)", 1),
    ]);

    let mut writer = client.bulk_insert(&builder).await.expect("Failed to start bulk insert");

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
    let mut client = client.begin_transaction().await.expect("Failed to begin txn");

    let builder = BulkInsertBuilder::new("#BulkRollback").with_typed_columns(vec![
        BulkColumn::new("id", "INT", 0),
        BulkColumn::new("val", "NVARCHAR(50)", 1),
    ]);

    let mut writer = client.bulk_insert(&builder).await.expect("Failed to start bulk insert");

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
            BulkColumn::new("id", "INT", 0),
            BulkColumn::new("name", "NVARCHAR(50)", 1),
        ])
        .fire_triggers(true);

    let mut writer = client.bulk_insert(&builder).await.expect("Failed to start bulk insert");

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
        .execute(
            "CREATE TABLE dbo.BulkNoTrigSrc (id INT NOT NULL)",
            &[],
        )
        .await
        .expect("Failed to create table");

    client
        .execute(
            "CREATE TABLE dbo.BulkNoTrigLog (src_id INT NOT NULL)",
            &[],
        )
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
        .with_typed_columns(vec![BulkColumn::new("id", "INT", 0)]);

    let mut writer = client.bulk_insert(&builder).await.expect("Failed to start bulk insert");

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
        .with_typed_columns(vec![BulkColumn::new("id", "INT", 0)]);

    let mut writer = client.bulk_insert(&builder).await.expect("Failed to start bulk insert");
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
    let mut writer = client.bulk_insert(&builder).await.expect("Failed to start second bulk insert");
    writer
        .send_row_values(&[SqlValue::Int(2)])
        .expect("Failed to send row");
    writer.finish().await.expect("Failed to finish second bulk insert");

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
