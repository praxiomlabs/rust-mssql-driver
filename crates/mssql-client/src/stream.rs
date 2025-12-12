//! Streaming query result support.
//!
//! This module provides streaming result sets for memory-efficient
//! processing of large query results.

use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::Stream;

use crate::error::Error;
use crate::row::{Column, Row};

/// A streaming result set from a query.
///
/// This stream yields rows one at a time, allowing processing of
/// large result sets without loading everything into memory.
///
/// # Example
///
/// ```rust,ignore
/// use futures::StreamExt;
///
/// let mut stream = client.query_stream("SELECT * FROM large_table").await?;
///
/// while let Some(row) = stream.next().await {
///     let row = row?;
///     process_row(&row);
/// }
/// ```
pub struct QueryStream<'a> {
    /// Column metadata for the result set.
    columns: Vec<Column>,
    /// Whether the stream has completed.
    finished: bool,
    /// Lifetime tied to the connection.
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a> QueryStream<'a> {
    /// Create a new query stream.
    #[allow(dead_code)] // Used when query execution is implemented
    pub(crate) fn new(columns: Vec<Column>) -> Self {
        Self {
            columns,
            finished: false,
            _marker: std::marker::PhantomData,
        }
    }

    /// Get the column metadata for this result set.
    #[must_use]
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// Check if the stream has finished.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Collect all remaining rows into a vector.
    ///
    /// This consumes the stream and loads all rows into memory.
    /// For large result sets, consider iterating with the stream instead.
    pub async fn collect_all(self) -> Result<Vec<Row>, Error> {
        // Placeholder: actual implementation would collect from stream
        Ok(Vec::new())
    }
}

impl Stream for QueryStream<'_> {
    type Item = Result<Row, Error>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if this.finished {
            return Poll::Ready(None);
        }

        // Placeholder: actual implementation would poll the connection
        // for the next row in the result set
        this.finished = true;
        Poll::Ready(None)
    }
}

/// Result of a non-query execution.
///
/// Contains the number of affected rows and any output parameters.
#[derive(Debug, Clone)]
pub struct ExecuteResult {
    /// Number of rows affected by the statement.
    pub rows_affected: u64,
    /// Output parameters from stored procedures.
    pub output_params: Vec<OutputParam>,
}

/// An output parameter from a stored procedure call.
#[derive(Debug, Clone)]
pub struct OutputParam {
    /// Parameter name.
    pub name: String,
    /// Parameter value.
    pub value: mssql_types::SqlValue,
}

impl ExecuteResult {
    /// Create a new execute result.
    pub fn new(rows_affected: u64) -> Self {
        Self {
            rows_affected,
            output_params: Vec::new(),
        }
    }

    /// Create a result with output parameters.
    pub fn with_outputs(rows_affected: u64, output_params: Vec<OutputParam>) -> Self {
        Self {
            rows_affected,
            output_params,
        }
    }

    /// Get an output parameter by name.
    #[must_use]
    pub fn get_output(&self, name: &str) -> Option<&OutputParam> {
        self.output_params
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
    }
}

/// Multiple result sets from a batch or stored procedure.
///
/// Some queries return multiple result sets (e.g., stored procedures
/// with multiple SELECT statements).
pub struct MultiResultStream<'a> {
    /// Current result set index.
    current_result: usize,
    /// Total number of result sets (if known).
    #[allow(dead_code)] // Will be used when multi-result handling is implemented
    total_results: Option<usize>,
    /// Lifetime tied to the connection.
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a> MultiResultStream<'a> {
    /// Create a new multi-result stream.
    #[allow(dead_code)] // Used when multi-result queries are implemented
    pub(crate) fn new() -> Self {
        Self {
            current_result: 0,
            total_results: None,
            _marker: std::marker::PhantomData,
        }
    }

    /// Get the current result set index (0-based).
    #[must_use]
    pub fn current_result_index(&self) -> usize {
        self.current_result
    }

    /// Move to the next result set.
    ///
    /// Returns `true` if there is another result set, `false` if no more.
    pub async fn next_result(&mut self) -> Result<bool, Error> {
        // Placeholder: actual implementation would advance to next result set
        self.current_result += 1;
        Ok(false)
    }

    /// Get the next row from the current result set.
    pub async fn next_row(&mut self) -> Result<Option<Row>, Error> {
        // Placeholder: actual implementation would get the next row
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_result() {
        let result = ExecuteResult::new(42);
        assert_eq!(result.rows_affected, 42);
        assert!(result.output_params.is_empty());
    }

    #[test]
    fn test_execute_result_with_outputs() {
        let outputs = vec![OutputParam {
            name: "ReturnValue".to_string(),
            value: mssql_types::SqlValue::Int(100),
        }];

        let result = ExecuteResult::with_outputs(10, outputs);
        assert_eq!(result.rows_affected, 10);
        assert!(result.get_output("ReturnValue").is_some());
        assert!(result.get_output("returnvalue").is_some()); // case-insensitive
        assert!(result.get_output("NotFound").is_none());
    }

    #[test]
    fn test_query_stream_columns() {
        let columns = vec![Column {
            name: "id".to_string(),
            index: 0,
            type_name: "INT".to_string(),
            nullable: false,
            max_length: Some(4),
            precision: Some(0),
            scale: Some(0),
        }];

        let stream = QueryStream::new(columns);
        assert_eq!(stream.columns().len(), 1);
        assert_eq!(stream.columns()[0].name, "id");
        assert!(!stream.is_finished());
    }
}
