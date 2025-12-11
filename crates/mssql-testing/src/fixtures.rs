//! Test fixture utilities.

/// Test database fixture for setting up and tearing down test data.
pub struct TestFixture {
    /// Database name.
    pub database: String,
    /// Tables created by this fixture.
    pub tables: Vec<String>,
}

impl TestFixture {
    /// Create a new test fixture.
    #[must_use]
    pub fn new(database: impl Into<String>) -> Self {
        Self {
            database: database.into(),
            tables: Vec::new(),
        }
    }

    /// Add a table to the fixture.
    #[must_use]
    pub fn with_table(mut self, table: impl Into<String>) -> Self {
        self.tables.push(table.into());
        self
    }

    /// Generate SQL to create the test database.
    #[must_use]
    pub fn create_database_sql(&self) -> String {
        format!(
            "IF NOT EXISTS (SELECT * FROM sys.databases WHERE name = '{db}')
             CREATE DATABASE [{db}]",
            db = self.database
        )
    }

    /// Generate SQL to drop the test database.
    #[must_use]
    pub fn drop_database_sql(&self) -> String {
        format!(
            "IF EXISTS (SELECT * FROM sys.databases WHERE name = '{db}')
             DROP DATABASE [{db}]",
            db = self.database
        )
    }
}
