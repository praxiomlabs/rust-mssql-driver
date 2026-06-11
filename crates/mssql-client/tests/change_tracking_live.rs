//! Live end-to-end test for the Change Tracking helpers.
//!
//! The `change_tracking` module ships SQL builders (`ChangeTrackingQuery`,
//! `ChangeTracking`) that were previously validated only as strings. This
//! exercises the generated SQL against a real CT-enabled database: enable
//! CT, perform DML, and read the changes back through the builder output —
//! proving the generated CHANGETABLE queries, the FORCESEEK variant, the
//! enable/disable DDL, and the version helpers all execute and report the
//! right operations.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use mssql_client::change_tracking::{
    ChangeOperation, ChangeTracking, ChangeTrackingQuery, SyncVersionStatus,
};
use mssql_client::{Client, Config, Error};

fn get_test_config(database: &str) -> Option<Config> {
    let host = std::env::var("MSSQL_HOST").ok()?;
    let port = std::env::var("MSSQL_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(1433);
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "MyStrongPassw0rd".into());
    let encrypt = std::env::var("MSSQL_ENCRYPT").unwrap_or_else(|_| "false".into());

    let conn_str = format!(
        "Server={host},{port};Database={database};User Id={user};Password={password};\
         TrustServerCertificate=true;Encrypt={encrypt}"
    );
    Config::from_connection_string(&conn_str).ok()
}

async fn scalar_i64(client: &mut Client<mssql_client::Ready>, sql: &str) -> i64 {
    let rows = client.query(sql, &[]).await.expect("scalar query");
    rows.into_iter()
        .next()
        .expect("scalar row")
        .expect("scalar row ok")
        .get(0)
        .expect("scalar value")
}

#[tokio::test]
#[ignore = "Requires SQL Server"]
async fn test_change_tracking_end_to_end() -> Result<(), Error> {
    let db_name = format!(
        "mssql_driver_test_ct_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );

    let setup_config = get_test_config("master").expect("SQL Server config required");

    {
        let mut setup = Client::connect(setup_config.clone()).await?;
        setup
            .execute(&format!("CREATE DATABASE {db_name}"), &[])
            .await?;
        // Builder-generated DDL: enable CT on the database.
        setup
            .execute(&ChangeTracking::enable_database_sql(&db_name, 2, true), &[])
            .await?;
        setup.close().await?;
    }

    let run = async {
        let mut client = Client::connect(get_test_config(&db_name).expect("config")).await?;

        client
            .execute(
                "CREATE TABLE dbo.Items (id INT NOT NULL PRIMARY KEY, name NVARCHAR(50) NOT NULL)",
                &[],
            )
            .await?;
        // Builder-generated DDL: enable CT on the table.
        client
            .execute(&ChangeTracking::enable_table_sql("dbo.Items", true), &[])
            .await?;

        // Seed two rows, then capture the sync baseline.
        client
            .execute(
                "INSERT INTO dbo.Items (id, name) VALUES (1, N'one'), (2, N'two')",
                &[],
            )
            .await?;
        let baseline = scalar_i64(&mut client, ChangeTracking::current_version_sql()).await;

        // Tracked window: one update, one delete.
        client
            .execute("UPDATE dbo.Items SET name = N'uno' WHERE id = 1", &[])
            .await?;
        client
            .execute("DELETE FROM dbo.Items WHERE id = 2", &[])
            .await?;

        // The baseline must still be valid for synchronization.
        let min_valid = scalar_i64(
            &mut client,
            &ChangeTracking::min_valid_version_sql("dbo.Items"),
        )
        .await;
        assert_eq!(
            SyncVersionStatus::check(baseline, Some(min_valid)),
            SyncVersionStatus::Valid,
            "fresh baseline must be a valid sync version"
        );

        // Builder-generated CHANGETABLE query. Column order per the builder:
        // SYS_CHANGE_VERSION, SYS_CHANGE_CREATION_VERSION,
        // SYS_CHANGE_OPERATION, SYS_CHANGE_COLUMNS, SYS_CHANGE_CONTEXT,
        // then the primary keys.
        let sql = ChangeTrackingQuery::changes("dbo.Items", baseline)
            .with_primary_keys(&["id"])
            .to_sql();
        let rows = client.query(&sql, &[]).await?;
        let mut changes: Vec<(ChangeOperation, i32)> = Vec::new();
        for row in rows {
            let row = row?;
            let op: String = row.get(2)?;
            let id: i32 = row.get(5)?;
            changes.push((
                ChangeOperation::from_sql(&op).expect("known operation code"),
                id,
            ));
        }
        changes.sort_by_key(|(_, id)| *id);
        assert_eq!(
            changes,
            vec![(ChangeOperation::Update, 1), (ChangeOperation::Delete, 2)],
            "the tracked window contains exactly one update and one delete"
        );

        // The data-join variant (with FORCESEEK) must also execute and join
        // live column values for surviving rows.
        let sql = ChangeTrackingQuery::changes("dbo.Items", baseline)
            .with_primary_keys(&["id"])
            .with_force_seek()
            .to_sql_with_data(&["name"]);
        let rows = client.query(&sql, &[]).await?;
        let mut joined: Vec<(String, i32, Option<String>)> = Vec::new();
        for row in rows {
            let row = row?;
            // ct columns (5), then CT.id (pk), then T.name (per to_sql_with_data).
            let op: String = row.get(2)?;
            let id: i32 = row.get(5)?;
            let name: Option<String> = row.get(6)?;
            joined.push((op, id, name));
        }
        joined.sort_by_key(|(_, id, _)| *id);
        assert_eq!(joined.len(), 2);
        assert_eq!(joined[0].0, "U");
        assert_eq!(
            joined[0].2.as_deref(),
            Some("uno"),
            "updated row must join its live column value"
        );
        assert_eq!(joined[1].0, "D");
        assert_eq!(
            joined[1].2, None,
            "deleted row has no live column value to join"
        );

        // Builder-generated disable DDL must execute cleanly.
        client
            .execute(&ChangeTracking::disable_table_sql("dbo.Items"), &[])
            .await?;

        client.close().await?;
        Ok::<_, Error>(())
    }
    .await;

    {
        let mut cleanup = Client::connect(setup_config).await?;
        let _ = cleanup
            .execute(
                &format!(
                    "IF DB_ID('{db_name}') IS NOT NULL BEGIN \
                        ALTER DATABASE {db_name} SET SINGLE_USER WITH ROLLBACK IMMEDIATE; \
                        DROP DATABASE {db_name}; \
                     END"
                ),
                &[],
            )
            .await;
        cleanup.close().await?;
    }

    run
}
