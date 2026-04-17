//! Streaming query result support.
//!
//! This module provides streaming result sets for memory-efficient
//! processing of large query results.
//!
//! ## Buffered vs True Streaming
//!
//! The underlying TDS response is reassembled into a single [`bytes::Bytes`]
//! payload by [`mssql-codec`](mssql_codec). That payload is handed to the
//! token parser which walks it once and enqueues each row's raw byte slice
//! (a cheap, refcounted slice into the original [`bytes::Bytes`] per ADR-004)
//! into the stream. Individual [`Row`]s are then decoded lazily when callers
//! pull them — either via the [`Stream`]/[`Iterator`] impls or
//! [`QueryStream::collect_all`].
//!
//! This "lazy decode" pattern keeps peak memory at roughly the size of the
//! raw payload instead of payload + fully-typed `Vec<Row>`. Users who iterate
//! and drop each [`Row`] see memory proportional to a single row at a time
//! plus the shared raw payload. Users who `collect_all()` pay for the full
//! `Vec<Row>` just like before.
//!
//! The same lazy-decode pattern applies to [`MultiResultStream`],
//! [`ResultSet`], and [`ProcedureResult::result_sets`]: raw row bytes are
//! stashed during response read and each [`Row`] is decoded when the caller
//! pulls it. Because decoding can fail per row, [`ResultSet::next_row`]
//! returns `Option<Result<Row, Error>>` rather than `Option<Row>` — callers
//! observe decode errors at iteration time instead of at
//! `call_procedure().await?` / `query_multiple().await?`.
//!
//! For truly large result sets, consider using OFFSET/FETCH pagination.

use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::Stream;
use tds_protocol::token::{ColMetaData, NbcRow, RawRow};

use crate::error::Error;
use crate::row::{Column, Row};

/// A row that may be already decoded or still held as raw TDS bytes.
///
/// The lazy-parse query path enqueues raw rows (cheap `Bytes` slices into
/// the original response payload) and decodes them on demand. The eager
/// path used by tests and [`MultiResultStream::into_query_streams`] wraps
/// already-decoded rows.
#[derive(Debug, Clone)]
pub(crate) enum PendingRow {
    /// Already-decoded row (eager path — tests + `MultiResultStream` compat).
    Parsed(Row),
    /// Raw TDS row bytes, to be decoded on pull.
    Raw(RawRow),
    /// Null-bitmap-compressed row bytes, to be decoded on pull.
    Nbc(NbcRow),
}

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
pub struct QueryStream<'a> {
    /// Column metadata for the result set.
    columns: Vec<Column>,
    /// Buffered rows (typed or raw) from the response.
    rows: VecDeque<PendingRow>,
    /// Protocol metadata needed to decode raw rows. `None` if every pending
    /// row is already [`PendingRow::Parsed`].
    meta: Option<ColMetaData>,
    /// Pre-resolved column decryptor for Always Encrypted result sets.
    ///
    /// Wrapped in `Arc` so lazy result-set containers (`ResultSet`) can share
    /// this state without duplicating the derived-key material.
    #[cfg(feature = "always-encrypted")]
    decryptor: Option<std::sync::Arc<crate::column_decryptor::ColumnDecryptor>>,
    /// Whether the stream has completed.
    finished: bool,
    /// Lifetime tied to the connection.
    _marker: std::marker::PhantomData<&'a ()>,
}

impl QueryStream<'_> {
    /// Create a new query stream from already-decoded rows.
    ///
    /// This is the eager constructor used by unit tests. Production query
    /// paths use [`QueryStream::from_raw`] to defer row decoding.
    /// [`MultiResultStream::into_query_streams`] uses
    /// [`ResultSet::into_query_stream`] to preserve pending-row state.
    #[cfg(test)]
    pub(crate) fn new(columns: Vec<Column>, rows: Vec<Row>) -> Self {
        Self {
            columns,
            rows: rows.into_iter().map(PendingRow::Parsed).collect(),
            meta: None,
            #[cfg(feature = "always-encrypted")]
            decryptor: None,
            finished: false,
            _marker: std::marker::PhantomData,
        }
    }

    /// Create a query stream from raw row bytes and protocol metadata.
    ///
    /// Rows are decoded on demand as the stream is pulled. The `meta` must
    /// describe every row in `pending`. If decryption is configured,
    /// `decryptor` must cover the same column set.
    pub(crate) fn from_raw(
        columns: Vec<Column>,
        pending: Vec<PendingRow>,
        meta: ColMetaData,
        #[cfg(feature = "always-encrypted")] decryptor: Option<
            std::sync::Arc<crate::column_decryptor::ColumnDecryptor>,
        >,
    ) -> Self {
        Self {
            columns,
            rows: pending.into(),
            meta: Some(meta),
            #[cfg(feature = "always-encrypted")]
            decryptor,
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
            meta: None,
            #[cfg(feature = "always-encrypted")]
            decryptor: None,
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
    /// This consumes the stream and loads all rows into memory. Each row is
    /// decoded lazily here, so large raw payloads are freed as rows are
    /// produced rather than held alongside the typed `Vec<Row>` throughout
    /// the caller's query call.
    ///
    /// For very large result sets, consider iterating with the stream
    /// instead.
    pub async fn collect_all(mut self) -> Result<Vec<Row>, Error> {
        let mut out = Vec::with_capacity(self.rows.len());
        while let Some(pending) = self.rows.pop_front() {
            out.push(self.decode(pending)?);
        }
        self.finished = true;
        Ok(out)
    }

    /// Try to get the next row synchronously (without async).
    ///
    /// Returns `None` when no more rows are available or the next pending
    /// row fails to decode. Use [`Iterator::next`] instead if you need to
    /// observe decode errors.
    pub fn try_next(&mut self) -> Option<Row> {
        self.next().and_then(|r| r.ok())
    }

    /// Decode a pending row into a typed [`Row`].
    fn decode(&self, pending: PendingRow) -> Result<Row, Error> {
        match pending {
            PendingRow::Parsed(row) => Ok(row),
            PendingRow::Raw(raw) => {
                let meta = self
                    .meta
                    .as_ref()
                    .ok_or_else(|| Error::Protocol("row metadata missing for raw row".into()))?;
                #[cfg(feature = "always-encrypted")]
                if let Some(ref dec) = self.decryptor {
                    return crate::column_parser::convert_raw_row_decrypted(
                        &raw,
                        meta,
                        &self.columns,
                        dec,
                    );
                }
                crate::column_parser::convert_raw_row(&raw, meta, &self.columns)
            }
            PendingRow::Nbc(nbc) => {
                let meta = self
                    .meta
                    .as_ref()
                    .ok_or_else(|| Error::Protocol("row metadata missing for NBC row".into()))?;
                #[cfg(feature = "always-encrypted")]
                if let Some(ref dec) = self.decryptor {
                    return crate::column_parser::convert_nbc_row_decrypted(
                        &nbc,
                        meta,
                        &self.columns,
                        dec,
                    );
                }
                crate::column_parser::convert_nbc_row(&nbc, meta, &self.columns)
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

        match this.rows.pop_front() {
            Some(pending) => Poll::Ready(Some(this.decode(pending))),
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
            Some(pending) => Some(self.decode(pending)),
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

/// Result of a non-query execution.
///
/// Contains the number of affected rows and any output parameters.
#[derive(Debug, Clone)]
#[non_exhaustive]
#[must_use]
pub struct ExecuteResult {
    /// Number of rows affected by the statement.
    pub rows_affected: u64,
    /// Output parameters from stored procedures.
    pub output_params: Vec<OutputParam>,
}

/// An output parameter from a stored procedure call.
#[derive(Debug, Clone)]
#[non_exhaustive]
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

/// Result of a stored procedure execution.
///
/// Contains the return value, affected row count, output parameters,
/// and any result sets produced by the procedure.
///
/// # Example
///
/// ```rust,ignore
/// let result = client.call_procedure("dbo.GetUser", &[&1i32]).await?;
///
/// // Check the return value (RETURN statement in the proc)
/// assert_eq!(result.return_value, 0);
///
/// // Process result sets
/// for mut rs in result.result_sets {
///     while let Some(row) = rs.next_row() {
///         let row = row?;
///         println!("{:?}", row);
///     }
/// }
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
#[must_use]
pub struct ProcedureResult {
    /// Return value from the stored procedure's RETURN statement.
    ///
    /// Defaults to 0 if the procedure does not explicitly return a value,
    /// which matches SQL Server's default behavior.
    pub return_value: i32,
    /// Total number of rows affected by statements within the procedure.
    pub rows_affected: u64,
    /// Output parameters returned by the procedure.
    pub output_params: Vec<OutputParam>,
    /// Result sets produced by SELECT statements within the procedure.
    pub result_sets: Vec<ResultSet>,
}

impl ProcedureResult {
    /// Create a new empty procedure result.
    pub(crate) fn new() -> Self {
        Self {
            return_value: 0,
            rows_affected: 0,
            output_params: Vec::new(),
            result_sets: Vec::new(),
        }
    }

    /// Get the return value from the stored procedure.
    ///
    /// This is the value from the procedure's `RETURN` statement.
    /// Defaults to 0 if not explicitly set by the procedure.
    #[must_use]
    pub fn get_return_value(&self) -> i32 {
        self.return_value
    }

    /// Get an output parameter by name (case-insensitive).
    ///
    /// Strips the `@` prefix from both the search name and stored names
    /// before comparing, so `get_output("result")` and `get_output("@result")`
    /// are equivalent.
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
    /// let output = result.get_output("@result").expect("output param exists");
    /// assert_eq!(output.value, SqlValue::Int(30));
    /// ```
    #[must_use]
    pub fn get_output(&self, name: &str) -> Option<&OutputParam> {
        let search = name.strip_prefix('@').unwrap_or(name);
        self.output_params.iter().find(|p| {
            let stored = p.name.strip_prefix('@').unwrap_or(&p.name);
            stored.eq_ignore_ascii_case(search)
        })
    }

    /// Get the first result set, if any.
    ///
    /// Convenience method for procedures that return a single result set.
    #[must_use]
    pub fn first_result_set(&self) -> Option<&ResultSet> {
        self.result_sets.first()
    }

    /// Check if the procedure produced any result sets.
    #[must_use]
    pub fn has_result_sets(&self) -> bool {
        !self.result_sets.is_empty()
    }
}

/// A single result set within a multi-result batch.
///
/// Rows are stored as `PendingRow` values that may be either already-decoded
/// [`Row`]s (eager path, used by tests and direct construction) or raw TDS
/// bytes (lazy path, used by [`crate::Client::call_procedure`] and
/// [`crate::Client::query_multiple`]). Decoding happens on pull, so per-row
/// decode errors surface through [`ResultSet::next_row`] and
/// [`ResultSet::collect_all`].
#[derive(Debug, Clone)]
#[must_use]
pub struct ResultSet {
    /// Column metadata for this result set.
    columns: Vec<Column>,
    /// Pending rows — either pre-parsed or raw TDS bytes awaiting decode.
    pending_rows: VecDeque<PendingRow>,
    /// Protocol metadata required to decode raw rows. `None` when every
    /// pending row is already [`PendingRow::Parsed`] (eager path).
    meta: Option<ColMetaData>,
    /// Pre-resolved column decryptor for Always Encrypted result sets.
    ///
    /// Wrapped in `Arc` so cloning a [`ResultSet`] stays cheap (clones share
    /// the underlying decryptor state instead of duplicating derived keys).
    #[cfg(feature = "always-encrypted")]
    decryptor: Option<std::sync::Arc<crate::column_decryptor::ColumnDecryptor>>,
}

impl ResultSet {
    /// Create a new result set from already-decoded rows.
    ///
    /// This is the eager constructor used by tests and callers that already
    /// hold typed [`Row`]s. Production query paths use `ResultSet::from_raw`
    /// (private) to defer row decoding.
    pub fn new(columns: Vec<Column>, rows: Vec<Row>) -> Self {
        Self {
            columns,
            pending_rows: rows.into_iter().map(PendingRow::Parsed).collect(),
            meta: None,
            #[cfg(feature = "always-encrypted")]
            decryptor: None,
        }
    }

    /// Create a result set from raw row bytes and protocol metadata.
    ///
    /// Rows are decoded on demand as the caller pulls them via
    /// [`ResultSet::next_row`] or [`ResultSet::collect_all`]. The `meta` must
    /// describe every row in `pending`. If decryption is configured,
    /// `decryptor` must cover the same column set.
    pub(crate) fn from_raw(
        columns: Vec<Column>,
        pending: Vec<PendingRow>,
        meta: ColMetaData,
        #[cfg(feature = "always-encrypted")] decryptor: Option<
            std::sync::Arc<crate::column_decryptor::ColumnDecryptor>,
        >,
    ) -> Self {
        Self {
            columns,
            pending_rows: pending.into(),
            meta: Some(meta),
            #[cfg(feature = "always-encrypted")]
            decryptor,
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
        self.pending_rows.len()
    }

    /// Get the next row from this result set.
    ///
    /// Returns `None` when no more rows remain, or `Some(Err(_))` when the
    /// next pending row fails to decode. The stream is not short-circuited
    /// on decode error — the caller may continue to pull subsequent rows.
    pub fn next_row(&mut self) -> Option<Result<Row, Error>> {
        self.pending_rows.pop_front().map(|p| self.decode(p))
    }

    /// Check if this result set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pending_rows.is_empty()
    }

    /// Collect all remaining rows into a vector.
    ///
    /// Stops at the first decode error and returns it.
    pub fn collect_all(&mut self) -> Result<Vec<Row>, Error> {
        let mut out = Vec::with_capacity(self.pending_rows.len());
        while let Some(pending) = self.pending_rows.pop_front() {
            out.push(self.decode(pending)?);
        }
        Ok(out)
    }

    /// Decode a pending row into a typed [`Row`].
    fn decode(&self, pending: PendingRow) -> Result<Row, Error> {
        match pending {
            PendingRow::Parsed(row) => Ok(row),
            PendingRow::Raw(raw) => {
                let meta = self
                    .meta
                    .as_ref()
                    .ok_or_else(|| Error::Protocol("row metadata missing for raw row".into()))?;
                #[cfg(feature = "always-encrypted")]
                if let Some(ref dec) = self.decryptor {
                    return crate::column_parser::convert_raw_row_decrypted(
                        &raw,
                        meta,
                        &self.columns,
                        dec,
                    );
                }
                crate::column_parser::convert_raw_row(&raw, meta, &self.columns)
            }
            PendingRow::Nbc(nbc) => {
                let meta = self
                    .meta
                    .as_ref()
                    .ok_or_else(|| Error::Protocol("row metadata missing for NBC row".into()))?;
                #[cfg(feature = "always-encrypted")]
                if let Some(ref dec) = self.decryptor {
                    return crate::column_parser::convert_nbc_row_decrypted(
                        &nbc,
                        meta,
                        &self.columns,
                        dec,
                    );
                }
                crate::column_parser::convert_nbc_row(&nbc, meta, &self.columns)
            }
        }
    }

    /// Consume this result set and produce a [`QueryStream`] that carries the
    /// same pending rows and decode state.
    ///
    /// Used by [`MultiResultStream::into_query_streams`] — avoids eagerly
    /// materializing rows when the caller wants stream-level ergonomics.
    fn into_query_stream<'a>(self) -> QueryStream<'a> {
        QueryStream {
            columns: self.columns,
            rows: self.pending_rows,
            meta: self.meta,
            #[cfg(feature = "always-encrypted")]
            decryptor: self.decryptor,
            finished: false,
            _marker: std::marker::PhantomData,
        }
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
    ///
    /// Per-row decode errors (from lazy row decoding) surface here as
    /// `Err(_)`. Pre-2.9 this reader decoded rows eagerly and decode errors
    /// surfaced at `query_multiple().await?` instead.
    pub async fn next_row(&mut self) -> Result<Option<Row>, Error> {
        if let Some(result_set) = self.result_sets.get_mut(self.current_result) {
            result_set.next_row().transpose()
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
    ///
    /// Returns `Ok(vec![])` if the current result index is out of range
    /// (e.g., all result sets have been consumed). Propagates decode errors
    /// from the underlying lazy row parser.
    pub fn collect_current(&mut self) -> Result<Vec<Row>, Error> {
        match self.result_sets.get_mut(self.current_result) {
            Some(rs) => rs.collect_all(),
            None => Ok(Vec::new()),
        }
    }

    /// Consume the stream and return all result sets as `QueryStream`s.
    pub fn into_query_streams(self) -> Vec<QueryStream<'a>> {
        self.result_sets
            .into_iter()
            .map(ResultSet::into_query_stream)
            .collect()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_result() {
        let result = ExecuteResult::new(42);
        assert_eq!(result.rows_affected, 42);
        assert!(result.output_params.is_empty());
    }

    #[test]
    fn test_procedure_result_defaults() {
        let result = ProcedureResult::new();
        assert_eq!(result.return_value, 0);
        assert_eq!(result.rows_affected, 0);
        assert!(result.output_params.is_empty());
        assert!(result.result_sets.is_empty());
        assert!(!result.has_result_sets());
        assert!(result.first_result_set().is_none());
    }

    #[test]
    fn test_procedure_result_get_output() {
        let mut result = ProcedureResult::new();
        result.output_params.push(OutputParam {
            name: "@Total".to_string(),
            value: mssql_types::SqlValue::Int(42),
        });
        result.output_params.push(OutputParam {
            name: "@Message".to_string(),
            value: mssql_types::SqlValue::String("ok".to_string()),
        });

        // Exact match (case-insensitive)
        assert!(result.get_output("@Total").is_some());
        assert!(result.get_output("@total").is_some());
        assert!(result.get_output("@TOTAL").is_some());

        // @ prefix stripping
        assert!(result.get_output("Total").is_some());
        assert!(result.get_output("total").is_some());

        // Non-existent
        assert!(result.get_output("@NotHere").is_none());
        assert!(result.get_output("NotHere").is_none());
    }

    #[test]
    fn test_procedure_result_with_result_sets() {
        use mssql_types::SqlValue;

        let columns = vec![Column {
            name: "id".to_string(),
            index: 0,
            type_name: "INT".to_string(),
            nullable: false,
            max_length: Some(4),
            precision: None,
            scale: None,
            collation: None,
        }];
        let rows = vec![Row::from_values(columns.clone(), vec![SqlValue::Int(1)])];
        let rs = ResultSet::new(columns, rows);

        let mut result = ProcedureResult::new();
        result.result_sets.push(rs);
        result.return_value = 7;
        result.rows_affected = 5;

        assert!(result.has_result_sets());
        assert_eq!(result.get_return_value(), 7);
        assert_eq!(result.first_result_set().unwrap().columns().len(), 1);
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

        // Use iterator — unwrap each Result so test failures are visible
        // (QueryStream's Iterator impl always yields Ok, but we should
        // not silently swallow errors if that ever changes)
        let values: Vec<i32> = stream
            .by_ref()
            .map(|r| r.unwrap().get::<i32>(0).unwrap())
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

    /// Exercises the lazy-decode path: rows are stored as raw TDS bytes and
    /// decoded only when the caller pulls them. Mirrors what
    /// `read_query_response` now produces and pins the contract between
    /// `PendingRow::Raw` and the per-row decode in `poll_next`/`next`.
    #[test]
    fn test_query_stream_lazy_raw_row_decoding() {
        use bytes::Bytes;
        use tds_protocol::token::{ColMetaData, ColumnData, RawRow, TypeInfo};
        use tds_protocol::types::TypeId;

        // Build raw row bytes for two columns: IntN(42) + IntN(NULL).
        let mut data = Vec::new();
        data.push(4); // IntN length prefix — 4 bytes
        data.extend_from_slice(&42i32.to_le_bytes());
        data.push(0); // IntN NULL (zero-length)

        let meta = ColMetaData {
            columns: vec![
                ColumnData {
                    name: "a".to_string(),
                    type_id: TypeId::IntN,
                    col_type: 0x26,
                    flags: 0x00,
                    user_type: 0,
                    type_info: TypeInfo {
                        max_length: Some(4),
                        precision: None,
                        scale: None,
                        collation: None,
                    },
                    crypto_metadata: None,
                },
                ColumnData {
                    name: "b".to_string(),
                    type_id: TypeId::IntN,
                    col_type: 0x26,
                    flags: 0x01,
                    user_type: 0,
                    type_info: TypeInfo {
                        max_length: Some(4),
                        precision: None,
                        scale: None,
                        collation: None,
                    },
                    crypto_metadata: None,
                },
            ],
            cek_table: None,
        };

        let columns = vec![
            Column {
                name: "a".to_string(),
                index: 0,
                type_name: "INT".to_string(),
                nullable: false,
                max_length: Some(4),
                precision: None,
                scale: None,
                collation: None,
            },
            Column {
                name: "b".to_string(),
                index: 1,
                type_name: "INT".to_string(),
                nullable: true,
                max_length: Some(4),
                precision: None,
                scale: None,
                collation: None,
            },
        ];

        let pending = vec![PendingRow::Raw(RawRow {
            data: Bytes::from(data),
        })];

        #[cfg(feature = "always-encrypted")]
        let mut stream = QueryStream::from_raw(columns, pending, meta, None);
        #[cfg(not(feature = "always-encrypted"))]
        let mut stream = QueryStream::from_raw(columns, pending, meta);

        assert_eq!(stream.rows_remaining(), 1);
        let row = stream
            .next()
            .expect("one row pending")
            .expect("row decoded successfully");
        assert_eq!(row.get::<i32>(0).unwrap(), 42);
        assert!(row.is_null(1));
        assert!(stream.next().is_none());
        assert!(stream.is_finished());
    }

    /// Decoder errors must surface per-row via `Stream`/`Iterator` without
    /// derailing the stream state. Truncated raw bytes trigger a decode
    /// error that the caller observes as `Some(Err(_))`.
    #[test]
    fn test_query_stream_lazy_decode_error_propagates() {
        use bytes::Bytes;
        use tds_protocol::token::{ColMetaData, ColumnData, RawRow, TypeInfo};
        use tds_protocol::types::TypeId;

        // Declare an Int4 column but provide only 2 bytes — decode must fail.
        let data = vec![0x01u8, 0x02];

        let meta = ColMetaData {
            columns: vec![ColumnData {
                name: "a".to_string(),
                type_id: TypeId::Int4,
                col_type: 0x38,
                flags: 0x00,
                user_type: 0,
                type_info: TypeInfo {
                    max_length: Some(4),
                    precision: None,
                    scale: None,
                    collation: None,
                },
                crypto_metadata: None,
            }],
            cek_table: None,
        };

        let columns = vec![Column {
            name: "a".to_string(),
            index: 0,
            type_name: "INT".to_string(),
            nullable: false,
            max_length: Some(4),
            precision: None,
            scale: None,
            collation: None,
        }];

        let pending = vec![PendingRow::Raw(RawRow {
            data: Bytes::from(data),
        })];

        #[cfg(feature = "always-encrypted")]
        let mut stream = QueryStream::from_raw(columns, pending, meta, None);
        #[cfg(not(feature = "always-encrypted"))]
        let mut stream = QueryStream::from_raw(columns, pending, meta);

        let item = stream.next().expect("pending row present");
        assert!(item.is_err(), "truncated bytes must surface a decode error");
        assert!(stream.next().is_none());
    }

    /// Helper to build a single-column IntN metadata block for the lazy
    /// `ResultSet` / `MultiResultStream` tests below.
    #[cfg(test)]
    fn intn_meta_and_columns(
        col_name: &str,
        nullable: bool,
    ) -> (tds_protocol::token::ColMetaData, Vec<Column>) {
        use tds_protocol::token::{ColMetaData, ColumnData, TypeInfo};
        use tds_protocol::types::TypeId;
        (
            ColMetaData {
                columns: vec![ColumnData {
                    name: col_name.to_string(),
                    type_id: TypeId::IntN,
                    col_type: 0x26,
                    flags: if nullable { 0x01 } else { 0x00 },
                    user_type: 0,
                    type_info: TypeInfo {
                        max_length: Some(4),
                        precision: None,
                        scale: None,
                        collation: None,
                    },
                    crypto_metadata: None,
                }],
                cek_table: None,
            },
            vec![Column {
                name: col_name.to_string(),
                index: 0,
                type_name: "INT".to_string(),
                nullable,
                max_length: Some(4),
                precision: None,
                scale: None,
                collation: None,
            }],
        )
    }

    /// Exercises the `ResultSet` lazy-decode path introduced in 2.9.
    /// Mirrors `test_query_stream_lazy_raw_row_decoding` but via the
    /// result-set API that `call_procedure` / `query_multiple` expose.
    #[test]
    fn test_result_set_lazy_raw_row_decoding() {
        use bytes::Bytes;
        use tds_protocol::token::RawRow;

        let (meta, columns) = intn_meta_and_columns("a", false);

        // Two rows: 7 and 11 encoded as IntN(4).
        let pending = vec![
            PendingRow::Raw(RawRow {
                data: {
                    let mut b = Vec::with_capacity(5);
                    b.push(4);
                    b.extend_from_slice(&7i32.to_le_bytes());
                    Bytes::from(b)
                },
            }),
            PendingRow::Raw(RawRow {
                data: {
                    let mut b = Vec::with_capacity(5);
                    b.push(4);
                    b.extend_from_slice(&11i32.to_le_bytes());
                    Bytes::from(b)
                },
            }),
        ];

        #[cfg(feature = "always-encrypted")]
        let mut rs = ResultSet::from_raw(columns, pending, meta, None);
        #[cfg(not(feature = "always-encrypted"))]
        let mut rs = ResultSet::from_raw(columns, pending, meta);

        assert_eq!(rs.rows_remaining(), 2);
        assert!(!rs.is_empty());

        let row1 = rs.next_row().expect("row present").expect("decodes");
        assert_eq!(row1.get::<i32>(0).unwrap(), 7);

        let row2 = rs.next_row().expect("row present").expect("decodes");
        assert_eq!(row2.get::<i32>(0).unwrap(), 11);

        assert!(rs.next_row().is_none());
        assert!(rs.is_empty());
    }

    /// Decoder errors in `ResultSet::next_row` must surface per-row without
    /// derailing further calls. Same contract as
    /// `test_query_stream_lazy_decode_error_propagates`.
    #[test]
    fn test_result_set_lazy_decode_error_propagates() {
        use bytes::Bytes;
        use tds_protocol::token::{ColMetaData, ColumnData, RawRow, TypeInfo};
        use tds_protocol::types::TypeId;

        // Int4 (not IntN) with only 2 bytes → decode must fail.
        let meta = ColMetaData {
            columns: vec![ColumnData {
                name: "a".to_string(),
                type_id: TypeId::Int4,
                col_type: 0x38,
                flags: 0x00,
                user_type: 0,
                type_info: TypeInfo {
                    max_length: Some(4),
                    precision: None,
                    scale: None,
                    collation: None,
                },
                crypto_metadata: None,
            }],
            cek_table: None,
        };
        let columns = vec![Column {
            name: "a".to_string(),
            index: 0,
            type_name: "INT".to_string(),
            nullable: false,
            max_length: Some(4),
            precision: None,
            scale: None,
            collation: None,
        }];

        let pending = vec![PendingRow::Raw(RawRow {
            data: Bytes::from(vec![0x01u8, 0x02]),
        })];

        #[cfg(feature = "always-encrypted")]
        let mut rs = ResultSet::from_raw(columns, pending, meta, None);
        #[cfg(not(feature = "always-encrypted"))]
        let mut rs = ResultSet::from_raw(columns, pending, meta);

        let first = rs.next_row().expect("pending row present");
        assert!(
            first.is_err(),
            "truncated bytes must surface a decode error"
        );
        assert!(rs.next_row().is_none());
    }

    /// `collect_all` on a lazy `ResultSet` decodes every pending row and
    /// propagates the first decode error. Ensures 2.9's signature change is
    /// exercised end-to-end.
    #[test]
    fn test_result_set_lazy_collect_all_success_and_error() {
        use bytes::Bytes;
        use tds_protocol::token::RawRow;

        // Success: two rows decode cleanly.
        let (meta_ok, cols_ok) = intn_meta_and_columns("a", false);
        let pending_ok = vec![
            PendingRow::Raw(RawRow {
                data: {
                    let mut b = Vec::with_capacity(5);
                    b.push(4);
                    b.extend_from_slice(&10i32.to_le_bytes());
                    Bytes::from(b)
                },
            }),
            PendingRow::Raw(RawRow {
                data: {
                    let mut b = Vec::with_capacity(5);
                    b.push(4);
                    b.extend_from_slice(&20i32.to_le_bytes());
                    Bytes::from(b)
                },
            }),
        ];

        #[cfg(feature = "always-encrypted")]
        let mut rs_ok = ResultSet::from_raw(cols_ok, pending_ok, meta_ok, None);
        #[cfg(not(feature = "always-encrypted"))]
        let mut rs_ok = ResultSet::from_raw(cols_ok, pending_ok, meta_ok);
        let rows = rs_ok.collect_all().expect("all rows decode");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get::<i32>(0).unwrap(), 10);
        assert_eq!(rows[1].get::<i32>(0).unwrap(), 20);
        assert!(rs_ok.is_empty());

        // Error: a truncated row (declared Int4 with only 2 bytes) makes
        // collect_all fail. collect_all short-circuits on the first Err.
        use tds_protocol::token::{ColMetaData, ColumnData, TypeInfo};
        use tds_protocol::types::TypeId;
        let meta_err = ColMetaData {
            columns: vec![ColumnData {
                name: "a".to_string(),
                type_id: TypeId::Int4,
                col_type: 0x38,
                flags: 0x00,
                user_type: 0,
                type_info: TypeInfo {
                    max_length: Some(4),
                    precision: None,
                    scale: None,
                    collation: None,
                },
                crypto_metadata: None,
            }],
            cek_table: None,
        };
        let cols_err = vec![Column {
            name: "a".to_string(),
            index: 0,
            type_name: "INT".to_string(),
            nullable: false,
            max_length: Some(4),
            precision: None,
            scale: None,
            collation: None,
        }];
        let pending_err = vec![PendingRow::Raw(RawRow {
            data: Bytes::from(vec![0x01u8, 0x02]),
        })];

        #[cfg(feature = "always-encrypted")]
        let mut rs_err = ResultSet::from_raw(cols_err, pending_err, meta_err, None);
        #[cfg(not(feature = "always-encrypted"))]
        let mut rs_err = ResultSet::from_raw(cols_err, pending_err, meta_err);
        let err = rs_err.collect_all();
        assert!(err.is_err(), "collect_all must propagate decode error");
    }

    /// `MultiResultStream` end-to-end lazy-decode path: two lazy `ResultSet`s
    /// decoded on demand as the caller walks through via `next_row` /
    /// `next_result`. Pins the 2.9 refactor of `read_multi_result_response`.
    #[tokio::test]
    async fn test_multi_result_stream_lazy_decode_across_result_sets() {
        use bytes::Bytes;
        use tds_protocol::token::RawRow;

        let (meta1, cols1) = intn_meta_and_columns("a", false);
        let pending1 = vec![PendingRow::Raw(RawRow {
            data: {
                let mut b = Vec::with_capacity(5);
                b.push(4);
                b.extend_from_slice(&101i32.to_le_bytes());
                Bytes::from(b)
            },
        })];
        #[cfg(feature = "always-encrypted")]
        let rs1 = ResultSet::from_raw(cols1, pending1, meta1, None);
        #[cfg(not(feature = "always-encrypted"))]
        let rs1 = ResultSet::from_raw(cols1, pending1, meta1);

        let (meta2, cols2) = intn_meta_and_columns("b", false);
        let pending2 = vec![PendingRow::Raw(RawRow {
            data: {
                let mut b = Vec::with_capacity(5);
                b.push(4);
                b.extend_from_slice(&202i32.to_le_bytes());
                Bytes::from(b)
            },
        })];
        #[cfg(feature = "always-encrypted")]
        let rs2 = ResultSet::from_raw(cols2, pending2, meta2, None);
        #[cfg(not(feature = "always-encrypted"))]
        let rs2 = ResultSet::from_raw(cols2, pending2, meta2);

        let mut stream = MultiResultStream::new(vec![rs1, rs2]);
        assert_eq!(stream.result_count(), 2);
        assert_eq!(stream.current_result_index(), 0);

        let row = stream
            .next_row()
            .await
            .expect("first row success")
            .expect("row present");
        assert_eq!(row.get::<i32>(0).unwrap(), 101);
        assert!(stream.next_row().await.expect("no more rows").is_none());

        assert!(stream.has_more_results());
        assert!(stream.next_result().await.expect("advance ok"));
        assert_eq!(stream.current_result_index(), 1);

        let row = stream
            .next_row()
            .await
            .expect("second row success")
            .expect("row present");
        assert_eq!(row.get::<i32>(0).unwrap(), 202);
    }
}
