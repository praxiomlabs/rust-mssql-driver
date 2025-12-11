//! SQL Server container support via testcontainers.

use testcontainers::Image;
use testcontainers::core::{ContainerPort, WaitFor};

/// SQL Server container image.
///
/// Uses the official Microsoft SQL Server container image.
#[derive(Debug, Clone)]
pub struct SqlServerContainer {
    /// SQL Server SA password.
    pub password: String,
    /// Container tag (version).
    pub tag: String,
}

impl Default for SqlServerContainer {
    fn default() -> Self {
        Self {
            password: "Password123!".to_string(),
            tag: "2022-latest".to_string(),
        }
    }
}

impl SqlServerContainer {
    /// Create a new SQL Server container configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the SA password.
    #[must_use]
    pub fn with_password(mut self, password: impl Into<String>) -> Self {
        self.password = password.into();
        self
    }

    /// Set the container tag (SQL Server version).
    #[must_use]
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = tag.into();
        self
    }
}

impl Image for SqlServerContainer {
    fn name(&self) -> &str {
        "mcr.microsoft.com/mssql/server"
    }

    fn tag(&self) -> &str {
        &self.tag
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![
            WaitFor::message_on_stdout("SQL Server is now ready for client connections"),
            WaitFor::seconds(5),
        ]
    }

    fn env_vars(
        &self,
    ) -> impl IntoIterator<
        Item = (
            impl Into<std::borrow::Cow<'_, str>>,
            impl Into<std::borrow::Cow<'_, str>>,
        ),
    > {
        vec![
            ("ACCEPT_EULA", "Y"),
            ("MSSQL_SA_PASSWORD", self.password.as_str()),
            ("MSSQL_PID", "Developer"),
        ]
    }

    fn expose_ports(&self) -> &[ContainerPort] {
        &[ContainerPort::Tcp(1433)]
    }
}
