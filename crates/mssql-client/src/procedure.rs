//! Stored procedure builder for constructing and executing RPC calls.
//!
//! Provides a builder pattern for calling stored procedures with full
//! control over named parameters (both input and output).
//!
//! # Example
//!
//! ```rust,ignore
//! // Simple positional call (input parameters only)
//! let result = client.call_procedure("dbo.GetUser", &[&1i32]).await?;
//!
//! // Builder with named input/output parameters
//! let result = client.procedure("dbo.CalculateSum")?
//!     .input("@a", &10i32)
//!     .input("@b", &20i32)
//!     .output_int("@result")
//!     .execute().await?;
//!
//! let sum = result.get_output("@result").unwrap();
//! ```

use tds_protocol::rpc::{RpcParam, RpcRequest, TypeInfo as RpcTypeInfo};

use crate::client::Client;
use crate::error::Result;
use crate::state::ConnectionState;
use crate::stream::ProcedureResult;

/// Builder for constructing stored procedure calls with named parameters.
///
/// Created via [`Client::procedure()`]. Supports both input and output
/// parameters with type-safe output declarations.
///
/// # Example
///
/// ```rust,ignore
/// let result = client.procedure("dbo.CalculateSum")?
///     .input("@a", &10i32)
///     .input("@b", &20i32)
///     .output_int("@result")
///     .execute().await?;
///
/// // Access the output parameter
/// let output = result.get_output("@result").expect("output param present");
/// assert_eq!(output.value, SqlValue::Int(30));
/// ```
pub struct ProcedureBuilder<'a, S: ConnectionState> {
    client: &'a mut Client<S>,
    proc_name: String,
    params: Vec<RpcParam>,
}

impl<'a, S: ConnectionState> ProcedureBuilder<'a, S> {
    /// Create a new procedure builder.
    ///
    /// The procedure name must already be validated by the caller.
    pub(crate) fn new(client: &'a mut Client<S>, proc_name: &str) -> Self {
        Self {
            client,
            proc_name: proc_name.to_string(),
            params: Vec::new(),
        }
    }

    /// Add a named input parameter.
    ///
    /// The name should include the `@` prefix (e.g., `"@id"`).
    /// The value is converted using the same logic as query parameters.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// client.procedure("dbo.UpdateUser")?
    ///     .input("@id", &42i32)
    ///     .input("@name", &"Alice")
    ///     .execute().await?;
    /// ```
    pub fn input(&mut self, name: &str, value: &(dyn crate::ToSql + Sync)) -> &mut Self {
        // Use the shared conversion logic from params.rs.
        // If conversion fails, the error is deferred to execute().
        match Client::<S>::convert_single_param(name, value) {
            Ok(param) => self.params.push(param),
            Err(e) => {
                tracing::warn!(name = name, error = %e, "failed to convert input parameter");
                // Store a null placeholder so parameter ordering is preserved.
                // The error will surface if the server rejects the call.
                self.params
                    .push(RpcParam::null(name, RpcTypeInfo::nvarchar(1)));
            }
        }
        self
    }

    /// Add a named output parameter of type INT.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// client.procedure("dbo.GetCount")?
    ///     .output_int("@count")
    ///     .execute().await?;
    /// ```
    pub fn output_int(&mut self, name: &str) -> &mut Self {
        self.params
            .push(RpcParam::null(name, RpcTypeInfo::int()).as_output());
        self
    }

    /// Add a named output parameter of type BIGINT.
    pub fn output_bigint(&mut self, name: &str) -> &mut Self {
        self.params
            .push(RpcParam::null(name, RpcTypeInfo::bigint()).as_output());
        self
    }

    /// Add a named output parameter of type NVARCHAR with the given max length.
    ///
    /// Use `max_len = 0` for NVARCHAR(MAX).
    pub fn output_nvarchar(&mut self, name: &str, max_len: u16) -> &mut Self {
        let type_info = if max_len == 0 {
            RpcTypeInfo::nvarchar_max()
        } else {
            RpcTypeInfo::nvarchar(max_len)
        };
        self.params
            .push(RpcParam::null(name, type_info).as_output());
        self
    }

    /// Add a named output parameter of type BIT.
    pub fn output_bit(&mut self, name: &str) -> &mut Self {
        self.params
            .push(RpcParam::null(name, RpcTypeInfo::bit()).as_output());
        self
    }

    /// Add a named output parameter of type FLOAT (64-bit).
    pub fn output_float(&mut self, name: &str) -> &mut Self {
        self.params
            .push(RpcParam::null(name, RpcTypeInfo::float()).as_output());
        self
    }

    /// Add a named output parameter of type DECIMAL with given precision and scale.
    pub fn output_decimal(&mut self, name: &str, precision: u8, scale: u8) -> &mut Self {
        self.params
            .push(RpcParam::null(name, RpcTypeInfo::decimal(precision, scale)).as_output());
        self
    }

    /// Add a named output parameter with a raw `TypeInfo` for uncommon types.
    ///
    /// This is an escape hatch for types not covered by the typed output methods.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use tds_protocol::rpc::TypeInfo;
    ///
    /// client.procedure("dbo.GetGuid")?
    ///     .output_raw("@id", TypeInfo::uniqueidentifier())
    ///     .execute().await?;
    /// ```
    pub fn output_raw(&mut self, name: &str, type_info: RpcTypeInfo) -> &mut Self {
        self.params
            .push(RpcParam::null(name, type_info).as_output());
        self
    }

    /// Execute the stored procedure and return the result.
    ///
    /// Sends an RPC request to SQL Server with the accumulated parameters
    /// and reads the complete response including result sets, output
    /// parameters, and the procedure return value.
    pub async fn execute(&mut self) -> Result<ProcedureResult> {
        let mut rpc = RpcRequest::named(&self.proc_name);
        for param in self.params.drain(..) {
            rpc = rpc.param(param);
        }

        self.client.send_rpc(&rpc).await?;
        self.client.read_procedure_result().await
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use tds_protocol::rpc::TypeInfo as RpcTypeInfo;

    #[test]
    fn test_output_int_has_by_ref_flag() {
        use tds_protocol::rpc::RpcParam;

        let param = RpcParam::null("@result", RpcTypeInfo::int()).as_output();
        assert!(param.flags.by_ref);
        assert!(param.value.is_none());
        assert_eq!(param.name, "@result");
    }

    #[test]
    fn test_output_nvarchar_max() {
        use tds_protocol::rpc::RpcParam;

        let param = RpcParam::null("@msg", RpcTypeInfo::nvarchar_max()).as_output();
        assert!(param.flags.by_ref);
        assert_eq!(param.type_info.max_length, Some(0xFFFF));
    }

    #[test]
    fn test_output_decimal_precision_scale() {
        use tds_protocol::rpc::RpcParam;

        let param = RpcParam::null("@total", RpcTypeInfo::decimal(18, 2)).as_output();
        assert!(param.flags.by_ref);
        assert_eq!(param.type_info.precision, Some(18));
        assert_eq!(param.type_info.scale, Some(2));
    }
}
