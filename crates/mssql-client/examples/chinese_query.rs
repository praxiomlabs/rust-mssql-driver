//!
//! An example of querying a SQL Server database and retrieving Chinese characters.
//! 
//! # Running
//!
//! ```bash
//! cargo run --example chinese_query
//! ``` 
//! 
use mssql_client::{Client, Config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
     // Build configuration using connection string
    let host = std::env::var("MSSQL_HOST").unwrap_or_else(|_| "192.168.100.5".into());
    let database = std::env::var("MSSQL_DATABASE").unwrap_or_else(|_| "master".into());
    let user = std::env::var("MSSQL_USER").unwrap_or_else(|_| "sa".into());
    let password = std::env::var("MSSQL_PASSWORD").unwrap_or_else(|_| "@cwc3002#".into());
    // Set MSSQL_ENCRYPT=false for development servers without TLS configured
    let encrypt = std::env::var("MSSQL_ENCRYPT").unwrap_or_else(|_| "false".into());

    let conn_str = format!(
        "Server={};Database={};User Id={};Password={};TrustServerCertificate=true;Encrypt={}",
        host, database, user, password, encrypt
    );
    let config = Config::from_connection_string(&conn_str)?;

    let mut client = Client::connect(config).await?;

    // Execute a query with parameters
    let rows = client
        .query("SELECT CONVERT(VARCHAR(40),'中文') COLLATE Chinese_PRC_CI_AI AS info,CONVERT(NVARCHAR(40),'汉字') AS lang", &[])
        .await?;

    println!("Number of rows: {}", rows.len());

    for row in rows {
        let row = row?;
        let info: String = row.get(0)?;
        let lang: String = row.get(1)?;
        println!("{:?} {:?}", info, lang);
    }

    Ok(())
}
