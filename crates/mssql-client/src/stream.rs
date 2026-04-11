//! Streaming query result support.
//!
//! This module provides streaming result sets for memory-efficient
//! processing of large query results.
//!
//! ## Buffered vs True Streaming
//!
//! The current implementation uses a buffered approach where all rows from
//! the TDS response are parsed upfront. This works well because:
//!
//! 1. TDS responses arrive as complete messages (reassembled by mssql-codec)
//! 2. Memory is shared via `Arc<Bytes>` pattern per ADR-004
//! 3. No complex lifetime/borrow issues with the connection
//!
//! For truly large result sets, consider using OFFSET/FETCH pagination.

use std::collections::VecDeque;
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
/// let mut stream = client.query("SELECT * FROM large_table", &[]).await?;
///
/// while let Some(row) = stream.next().await {
///     let row = row?;
///     process_row(&row);
/// }
/// ```
#[must_use = "streams must be consumed; dropping a stream discards remaining rows"]
#[derive(Debug)]
pub struct QueryStream<'a> {
    /// Column metadata for the result set.
    columns: Vec<Column>,
    /// Buffered rows from the response.
    rows: VecDeque<Row>,
    /// Whether the stream has completed.
    finished: bool,
    /// Lifetime tied to the connection.
    _marker: std::marker::PhantomData<&'a ()>,
}

impl QueryStream<'_> {
    /// Create a new query stream with columns and buffered rows.
    pub(crate) fn new(columns: Vec<Column>, rows: Vec<Row>) -> Self {
        Self {
            columns,
            rows: rows.into(),
            finished: false,
            _marker: std::marker::PhantomData,
        }
    }

    /// Create an empty query stream (no results).
    #[allow(dead_code)]
    pub(crate) fn empty() -> Self {
        Self {
            columns: Vec::new(),
            rows: VecDeque::new(),
            finished: true,
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

    /// Get the number of rows remaining in the buffer.
    #[must_use]
    pub fn rows_remaining(&self) -> usize {
        self.rows.len()
    }

    /// Collect all remaining rows into a vector.
    ///
    /// This consumes the stream and loads all rows into memory.
    /// For large result sets, consider iterating with the stream instead.
    pub async fn collect_all(mut self) -> Result<Vec<Row>, Error> {
        // Drain all remaining rows from the buffer
        let rows: Vec<Row> = self.rows.drain(..).collect();
        self.finished = true;
        Ok(rows)
    }

    /// Try to get the next row synchronously (without async).
    ///
    /// Returns `None` when no more rows are available.
    pub fn try_next(&mut self) -> Option<Row> {
        if self.finished {
            return None;
        }

        match self.rows.pop_front() {
            Some(row) => Some(row),
            None => {
                self.finished = true;
                None
            }
        }
    }
}

impl Stream for QueryStream<'_> {
    type Item = Result<Row, Error>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if this.finished {
            return Poll::Ready(None);
        }

        // Pop the next row from the buffer
        match this.rows.pop_front() {
            Some(row) => Poll::Ready(Some(Ok(row))),
            None => {
                this.finished = true;
                Poll::Ready(None)
            }
        }
    }
}

impl ExactSizeIterator for QueryStream<'_> {}

impl Iterator for QueryStream<'_> {
    type Item = Result<Row, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        match self.rows.pop_front() {
            Some(row) => Some(Ok(row)),
            None => {
                self.finished = true;
                None
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.rows.len();
        (remaining, Some(remaining))
    }
}

/// Result of a stored procedure execution.
///
/// Contains output parameters, affected rows, and optional result set.
///
/// # Structure
///
/// The `output_params` vector always contains:
/// 1. **RETURN value** (index 0) - Always present per SQL Server spec
/// 2. **OUTPUT parameters** - Following RETURN value, in declaration order
///
/// # Example
///
/// ```rust,ignore
/// let result = client.execute_procedure("dbo.MyProc", &[&1i32]).await?;
///
/// // Get RETURN value
/// if let Some(rv) = result.get_return_value() {
///     let status: i32 = rv.value.as_i32()?;
///     println!("Status: {}", status);
/// }
///
/// // Get output parameter
/// if let Some(out) = result.get_output("result") {
///     let value: i32 = out.value.as_i32()?;
///     println!("Result: {}", value);
/// }
///
/// // Process result set if available
/// if let Some(stream) = result.result_set {
///     while let Some(row) = stream.next() {
///         // Handle rows
///     }
/// }
/// ```
#[derive(Debug)]
#[must_use]
pub struct ExecuteResult<'a> {
    /// Output parameters from the stored procedure.
    ///
    /// Structure:
    /// - Index 0: RETURN value (name = "return_value", type = i32)
    /// - Index 1+: OUTPUT parameters in declaration order
    ///
    /// Per SQL Server specification, every stored procedure has an integer
    /// return value (default: 0) automatically included here.
    pub output_params: Vec<OutputParam>,

    /// Number of rows affected by the statement
    pub rows_affected: u64,

    /// Result set from SELECT statements (if any)
    pub result_set: Option<QueryStream<'a>>,
}

/// An output parameter from a stored procedure call.
#[derive(Debug, Clone)]
pub struct OutputParam {
    /// Parameter name:
    /// - "return_value" indicates the RETURN value (always present)
    /// - Other strings indicate OUTPUT parameter names (e.g., "sum", "result")
    pub name: String,

    /// Parameter value
    pub value: mssql_types::SqlValue,
}

impl<'a> ExecuteResult<'a> {
    /// Create a new execute result with all components.
    pub(crate) fn new(
        output_params: Vec<OutputParam>,
        rows_affected: u64,
        result_set: Option<QueryStream<'a>>,
    ) -> Self {
        Self {
            output_params,
            rows_affected,
            result_set,
        }
    }

    /// Create a result with only output parameters (no result set).
    pub fn with_outputs(output_params: Vec<OutputParam>, rows_affected: u64) -> Self {
        Self {
            output_params,
            rows_affected,
            result_set: None,
        }
    }

    /// Check if result set is available.
    #[must_use]
    pub fn has_result_set(&self) -> bool {
        self.result_set.is_some()
    }

    /// Get result set reference.
    #[must_use]
    pub fn get_result_set(&self) -> Option<&QueryStream<'a>> {
        self.result_set.as_ref()
    }

    /// Take result set, leaving None in its place.
    pub fn take_result_set(&mut self) -> Option<QueryStream<'a>> {
        self.result_set.take()
    }

    /// Get an output parameter by name (case-insensitive).
    ///
    /// The name can be with or without @ prefix. For example, both "@result" and "result"
    /// will match a parameter named "result".
    #[must_use]
    pub fn get_output(&self, name: &str) -> Option<&OutputParam> {
        // Strip @ prefix if present for matching
        let search_name = name.strip_prefix('@').unwrap_or(name);

        self.output_params
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(search_name))
    }

    /// Get the RETURN value from the stored procedure.
    ///
    /// Per SQL Server specification, every stored procedure has an integer return value
    /// (default: 0) that is always included as the first output parameter with the name
    /// "return_value".
    ///
    /// # Returns
    ///
    /// * `Some(&OutputParam)` - The RETURN value (always present)
    /// * `None` - Only if the protocol implementation has a bug
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let result = client.execute_procedure("dbo.MyProc", &[&1i32]).await?;
    ///
    /// // Get RETURN value
    /// if let Some(return_value) = result.get_return_value() {
    ///     let status: i32 = return_value.value.as_i32().unwrap();
    ///     println!("Stored procedure return status: {}", status);
    /// }
    /// ```
    #[must_use]
    pub fn get_return_value(&self) -> Option<&OutputParam> {
        self.output_params
            .first()
            .filter(|p| p.name == "return_value")
    }
}

/// A single result set within a multi-result batch.
#[derive(Debug)]
#[must_use]
pub struct ResultSet {
    /// Column metadata for this result set.
    columns: Vec<Column>,
    /// Rows in this result set.
    rows: VecDeque<Row>,
}

impl ResultSet {
    /// Create a new result set.
    pub fn new(columns: Vec<Column>, rows: Vec<Row>) -> Self {
        Self {
            columns,
            rows: rows.into(),
        }
    }

    /// Get the column metadata.
    #[must_use]
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// Get the number of rows remaining.
    #[must_use]
    pub fn rows_remaining(&self) -> usize {
        self.rows.len()
    }

    /// Get the next row from this result set.
    pub fn next_row(&mut self) -> Option<Row> {
        self.rows.pop_front()
    }

    /// Check if this result set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Collect all remaining rows into a vector.
    pub fn collect_all(&mut self) -> Vec<Row> {
        self.rows.drain(..).collect()
    }
}

/// Multiple result sets from a batch or stored procedure.
///
/// Some queries return multiple result sets (e.g., stored procedures
/// with multiple SELECT statements, or batches with multiple queries).
///
/// # Example
///
/// ```rust,ignore
/// // Execute a batch with multiple SELECTs
/// let mut results = client.query_multiple("SELECT 1 AS a; SELECT 2 AS b, 3 AS c;", &[]).await?;
///
/// // Process first result set
/// while let Some(row) = results.next_row().await? {
///     println!("Result 1: {:?}", row);
/// }
///
/// // Move to second result set
/// if results.next_result().await? {
///     while let Some(row) = results.next_row().await? {
///         println!("Result 2: {:?}", row);
///     }
/// }
/// ```
#[must_use = "streams must be consumed; dropping a stream discards remaining results"]
pub struct MultiResultStream<'a> {
    /// All result sets from the batch.
    result_sets: Vec<ResultSet>,
    /// Current result set index (0-based).
    current_result: usize,
    /// Lifetime tied to the connection.
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a> MultiResultStream<'a> {
    /// Create a new multi-result stream from parsed result sets.
    pub(crate) fn new(result_sets: Vec<ResultSet>) -> Self {
        Self {
            result_sets,
            current_result: 0,
            _marker: std::marker::PhantomData,
        }
    }

    /// Create an empty multi-result stream.
    #[allow(dead_code)]
    pub(crate) fn empty() -> Self {
        Self {
            result_sets: Vec::new(),
            current_result: 0,
            _marker: std::marker::PhantomData,
        }
    }

    /// Get the current result set index (0-based).
    #[must_use]
    pub fn current_result_index(&self) -> usize {
        self.current_result
    }

    /// Get the total number of result sets.
    #[must_use]
    pub fn result_count(&self) -> usize {
        self.result_sets.len()
    }

    /// Check if there are more result sets after the current one.
    #[must_use]
    pub fn has_more_results(&self) -> bool {
        self.current_result + 1 < self.result_sets.len()
    }

    /// Get the column metadata for the current result set.
    ///
    /// Returns `None` if there are no result sets or we've moved past all of them.
    #[must_use]
    pub fn columns(&self) -> Option<&[Column]> {
        self.result_sets
            .get(self.current_result)
            .map(|rs| rs.columns())
    }

    /// Move to the next result set.
    ///
    /// Returns `true` if there is another result set, `false` if no more.
    pub async fn next_result(&mut self) -> Result<bool, Error> {
        if self.current_result + 1 < self.result_sets.len() {
            self.current_result += 1;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get the next row from the current result set.
    ///
    /// Returns `None` when no more rows in the current result set.
    /// Call `next_result()` to move to the next result set.
    pub async fn next_row(&mut self) -> Result<Option<Row>, Error> {
        if let Some(result_set) = self.result_sets.get_mut(self.current_result) {
            Ok(result_set.next_row())
        } else {
            Ok(None)
        }
    }

    /// Get a mutable reference to the current result set.
    #[must_use]
    pub fn current_result_set(&mut self) -> Option<&mut ResultSet> {
        self.result_sets.get_mut(self.current_result)
    }

    /// Collect all rows from the current result set.
    pub fn collect_current(&mut self) -> Vec<Row> {
        self.result_sets
            .get_mut(self.current_result)
            .map(|rs| rs.collect_all())
            .unwrap_or_default()
    }

    /// Consume the stream and return all result sets as `QueryStream`s.
    pub fn into_query_streams(self) -> Vec<QueryStream<'a>> {
        self.result_sets
            .into_iter()
            .map(|rs| QueryStream::new(rs.columns, rs.rows.into()))
            .collect()
    }
}

/// Result of a stored procedure execution that may return multiple result sets.
///
/// This combines the OUTPUT parameters and RETURN value from a stored procedure
/// with multiple result sets (e.g., from multiple SELECT statements).
///
/// # Example
///
/// ```rust,ignore
/// // SQL: CREATE PROCEDURE dbo.sp_GetMultipleData
/// //      @min_score INT,
/// //      @row_count INT OUTPUT
/// // AS
/// // BEGIN
/// //     -- First result set
/// //     SELECT Id, Name FROM Users WHERE Score >= @min_score;
/// //
/// //     -- Second result set
/// //     SELECT Id, Score FROM UserScores WHERE Score >= @min_score;
/// //
/// //     SET @row_count = @@ROWCOUNT;
/// // END
///
/// let mut result = client
///     .execute_procedure_multiple("dbo.sp_GetMultipleData", &[&90i32])
///     .await?;
///
/// // Access OUTPUT parameters
/// let row_count = result.get_output("@row_count").unwrap();
/// println!("Total rows: {}", row_count.value.as_i32()?);
///
/// // Process first result set
/// while let Some(row) = result.next_row().await? {
///     let id: i32 = row.get(0)?;
///     let name: String = row.get(1)?;
///     println!("User: {} - {}", id, name);
/// }
///
/// // Move to second result set
/// if result.next_result().await? {
///     while let Some(row) = result.next_row().await? {
///         let id: i32 = row.get(0)?;
///         let score: i32 = row.get(1)?;
///         println!("Score: {} - {}", id, score);
///     }
/// }
/// ```
#[must_use = "results must be consumed; dropping discards remaining data"]
pub struct MultiExecuteResult<'a> {
    /// OUTPUT parameters (index 0 = RETURN value, index 1+ = OUTPUT parameters)
    pub output_params: Vec<OutputParam>,

    /// Number of rows affected
    pub rows_affected: u64,

    /// Multiple result sets from the stored procedure
    result_sets: MultiResultStream<'a>,
}

impl<'a> MultiExecuteResult<'a> {
    /// Create a new multi-execute result.
    pub(crate) fn new(
        output_params: Vec<OutputParam>,
        rows_affected: u64,
        result_sets: MultiResultStream<'a>,
    ) -> Self {
        Self {
            output_params,
            rows_affected,
            result_sets,
        }
    }

    /// Get the current result set index (0-based).
    #[must_use]
    pub fn current_result_index(&self) -> usize {
        self.result_sets.current_result_index()
    }

    /// Get the total number of result sets.
    #[must_use]
    pub fn result_count(&self) -> usize {
        self.result_sets.result_count()
    }

    /// Check if there are more result sets after the current one.
    #[must_use]
    pub fn has_more_results(&self) -> bool {
        self.result_sets.has_more_results()
    }

    /// Get the column metadata for the current result set.
    ///
    /// Returns `None` if there are no result sets or we've moved past all of them.
    #[must_use]
    pub fn columns(&self) -> Option<&[Column]> {
        self.result_sets.columns()
    }

    /// Move to the next result set.
    ///
    /// Returns `true` if there is another result set, `false` if no more.
    pub async fn next_result(&mut self) -> Result<bool, Error> {
        self.result_sets.next_result().await
    }

    /// Get the next row from the current result set.
    ///
    /// Returns `None` when no more rows in the current result set.
    /// Call `next_result()` to move to the next result set.
    pub async fn next_row(&mut self) -> Result<Option<Row>, Error> {
        self.result_sets.next_row().await
    }

    /// Get a mutable reference to the current result set.
    #[must_use]
    pub fn current_result_set(&mut self) -> Option<&mut ResultSet> {
        self.result_sets.current_result_set()
    }

    /// Collect all rows from the current result set.
    pub fn collect_current(&mut self) -> Vec<Row> {
        self.result_sets.collect_current()
    }

    /// Get an output parameter by name (case-insensitive).
    ///
    /// The name can be with or without @ prefix. For example, both "@result" and "result"
    /// will match a parameter named "result".
    #[must_use]
    pub fn get_output(&self, name: &str) -> Option<&OutputParam> {
        // Strip @ prefix if present for matching
        let search_name = name.strip_prefix('@').unwrap_or(name);

        self.output_params
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(search_name))
    }

    /// Get the RETURN value from the stored procedure.
    ///
    /// Per SQL Server specification, every stored procedure has an integer return value
    /// (default: 0) that is always included as the first output parameter with the name
    /// "return_value".
    ///
    /// # Returns
    ///
    /// * `Some(&OutputParam)` - The RETURN value (always present)
    /// * `None` - Only if the protocol implementation has a bug
    #[must_use]
    pub fn get_return_value(&self) -> Option<&OutputParam> {
        self.output_params
            .first()
            .filter(|p| p.name == "return_value")
    }

    /// Check if result set is available.
    #[must_use]
    pub fn has_result_set(&self) -> bool {
        self.result_sets.result_count() > 0
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_result() {
        let result = ExecuteResult::new(Vec::new(), 42, None);
        assert_eq!(result.rows_affected, 42);
        assert!(result.output_params.is_empty());
    }

    #[test]
    fn test_execute_result_with_outputs() {
        let outputs = vec![OutputParam {
            name: "ReturnValue".to_string(),
            value: mssql_types::SqlValue::Int(100),
        }];

        let result = ExecuteResult::with_outputs(outputs, 10);
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
            collation: None,
        }];

        let stream = QueryStream::new(columns, Vec::new());
        assert_eq!(stream.columns().len(), 1);
        assert_eq!(stream.columns()[0].name, "id");
        assert!(!stream.is_finished());
    }

    #[test]
    fn test_query_stream_with_rows() {
        use mssql_types::SqlValue;

        let columns = vec![
            Column {
                name: "id".to_string(),
                index: 0,
                type_name: "INT".to_string(),
                nullable: false,
                max_length: Some(4),
                precision: None,
                scale: None,
                collation: None,
            },
            Column {
                name: "name".to_string(),
                index: 1,
                type_name: "NVARCHAR".to_string(),
                nullable: true,
                max_length: Some(100),
                precision: None,
                scale: None,
                collation: None,
            },
        ];

        let rows = vec![
            Row::from_values(
                columns.clone(),
                vec![SqlValue::Int(1), SqlValue::String("Alice".to_string())],
            ),
            Row::from_values(
                columns.clone(),
                vec![SqlValue::Int(2), SqlValue::String("Bob".to_string())],
            ),
        ];

        let mut stream = QueryStream::new(columns, rows);
        assert_eq!(stream.columns().len(), 2);
        assert_eq!(stream.rows_remaining(), 2);
        assert!(!stream.is_finished());

        // First row
        let row1 = stream.try_next().unwrap();
        assert_eq!(row1.get::<i32>(0).unwrap(), 1);
        assert_eq!(row1.get_by_name::<String>("name").unwrap(), "Alice");

        // Second row
        let row2 = stream.try_next().unwrap();
        assert_eq!(row2.get::<i32>(0).unwrap(), 2);
        assert_eq!(row2.get_by_name::<String>("name").unwrap(), "Bob");

        // No more rows
        assert!(stream.try_next().is_none());
        assert!(stream.is_finished());
    }

    #[test]
    fn test_query_stream_iterator() {
        use mssql_types::SqlValue;

        let columns = vec![Column {
            name: "val".to_string(),
            index: 0,
            type_name: "INT".to_string(),
            nullable: false,
            max_length: None,
            precision: None,
            scale: None,
            collation: None,
        }];

        let rows = vec![
            Row::from_values(columns.clone(), vec![SqlValue::Int(10)]),
            Row::from_values(columns.clone(), vec![SqlValue::Int(20)]),
            Row::from_values(columns.clone(), vec![SqlValue::Int(30)]),
        ];

        let mut stream = QueryStream::new(columns, rows);

        // Use iterator
        let values: Vec<i32> = stream
            .by_ref()
            .filter_map(|r| r.ok())
            .map(|r| r.get::<i32>(0).unwrap())
            .collect();

        assert_eq!(values, vec![10, 20, 30]);
        assert!(stream.is_finished());
    }

    #[test]
    fn test_query_stream_empty() {
        let stream = QueryStream::empty();
        assert!(stream.columns().is_empty());
        assert_eq!(stream.rows_remaining(), 0);
        assert!(stream.is_finished());
    }
}
