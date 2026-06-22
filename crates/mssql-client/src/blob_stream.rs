//! Streaming a row's trailing MAX column directly from the socket.
//!
//! [`Client::query_stream_blob`](crate::Client::query_stream_blob) targets the
//! one OOM case the row streaming of [`RowStream`](crate::RowStream) does not
//! cover: a single MAX cell (`VARBINARY(MAX)`, `NVARCHAR(MAX)`, `VARCHAR(MAX)`,
//! `XML`) larger than memory. Row streaming bounds peak to ~one row, but a row
//! holding a 4 GB BLOB is still ~4 GB because the whole PLP value is decoded
//! into the row.
//!
//! [`BlobStream`] decodes each row's **leading scalar columns** into a [`Row`]
//! (they are small), then exposes the **trailing MAX column(s)** as chunk
//! streams pulled from the connection on demand via the `PlpDecoder`. Peak
//! memory is one packet plus one PLP chunk.
//!
//! Because TDS sends columns inline and sequentially, the MAX column(s) must be
//! **trailing** — every column must precede them (a scalar column after a BLOB
//! could not be decoded until that BLOB was consumed).
//! [`Client::query_stream_blob`](crate::Client::query_stream_blob) targets the
//! single-trailing-MAX case; [`Client::query_stream_rows`](crate::Client::query_stream_rows)
//! generalizes it to a run of trailing MAX columns, iterated with
//! [`BlobStream::next_blob`]. The blob(s) of one row must be consumed before the
//! next row; calling [`BlobStream::next`] auto-drains any unconsumed blob so the
//! wire stays aligned.
//!
//! ```no_run
//! # async fn ex(client: &mut mssql_client::Client<mssql_client::Ready>) -> Result<(), mssql_client::Error> {
//! # let mut sink: Vec<u8> = Vec::new();
//! let mut stream = client
//!     .query_stream_blob("SELECT id, document FROM files", &[])
//!     .await?;
//! while let Some(row) = stream.next().await? {
//!     let id: i32 = row.get_by_name("id")?;
//!     let _ = id;
//!     stream.copy_blob_to(&mut sink).await?; // streamed, never fully buffered
//! }
//! # Ok(())
//! # }
//! ```

use bytes::{Buf, Bytes, BytesMut};
use tds_protocol::ProtocolError;
use tds_protocol::token::{ColMetaData, ColumnData, NbcRow, RawRow, Token, TokenParser};
use tds_protocol::types::TypeId;

use crate::Client;
use crate::client::response::server_token_to_error;
use crate::error::{Error, Result};
use crate::plp::{PlpDecoder, PlpEvent};
use crate::row::{Column, Row};
use crate::state::{ConnectionState, Ready};

/// Whether a column is a PLP-encoded MAX type that this path can sub-stream.
pub(crate) fn is_plp_max(col: &ColumnData) -> bool {
    match col.type_id {
        TypeId::BigVarChar | TypeId::BigVarBinary | TypeId::NVarChar => {
            col.type_info.max_length == Some(0xFFFF)
        }
        TypeId::Xml => true,
        _ => false,
    }
}

/// A stream of rows whose trailing MAX column is read incrementally from the
/// socket. See the [module docs](self). Obtain one from
/// [`Client::query_stream_blob`](crate::Client::query_stream_blob).
#[must_use = "streams must be consumed; dropping a stream discards remaining rows"]
pub struct BlobStream<'a, S: ConnectionState = Ready> {
    client: &'a mut Client<S>,
    /// Unconsumed wire bytes (post-metadata), shared by the column decode and
    /// the PLP chunk streaming.
    buf: Bytes,
    /// END_OF_MESSAGE seen — no more packets will arrive.
    eom: bool,
    encryption_enabled: bool,
    /// Full result-set metadata (all columns, including the trailing MAX ones).
    meta: ColMetaData,
    /// Metadata for just the leading scalar columns (for row decoding).
    prefix_meta: ColMetaData,
    /// `Column`s for the leading scalar columns.
    scalar_row_meta: std::sync::Arc<crate::row::ColMetaData>,
    /// `Column`s for the trailing MAX columns, in wire order.
    blob_row_meta: std::sync::Arc<crate::row::ColMetaData>,
    /// Index of the first trailing MAX column; the scalar prefix is `0..first_blob`.
    first_blob: usize,
    /// Number of trailing MAX columns (1 for `query_stream_blob`).
    blob_count: usize,
    /// Index (within the trailing run, `0..blob_count`) of the next blob to
    /// position; reset to 0 at the start of each row.
    next_blob_idx: usize,
    /// The blob currently positioned for reading (within the trailing run), or
    /// `None` before the first is positioned / after the row is exhausted.
    current_blob_idx: Option<usize>,
    /// When the current row arrived as an NBCROW, its null bitmap (so each
    /// trailing blob's nullness can be looked up); `None` for a plain ROW.
    current_nbc: Option<NbcRow>,
    /// Whether `next` should auto-position the first trailing blob. True for the
    /// single-blob `query_stream_blob` path (preserves its API: the blob is
    /// ready to read after `next`); false for the multi-blob `query_stream_rows`
    /// path (the caller drives blobs explicitly via [`next_blob`](Self::next_blob)).
    auto_position_first: bool,
    /// PLP decoder for the current blob; `Some` while a non-NULL blob is being read.
    plp: Option<PlpDecoder>,
    /// The current blob is NULL (an NBCROW omitted its value).
    blob_null: bool,
    finished: bool,
}

impl<'a, S: ConnectionState> BlobStream<'a, S> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        client: &'a mut Client<S>,
        buf: Bytes,
        eom: bool,
        encryption_enabled: bool,
        meta: ColMetaData,
        first_blob: usize,
        blob_count: usize,
        auto_position_first: bool,
    ) -> Self {
        let prefix_meta = ColMetaData {
            columns: meta.columns.iter().take(first_blob).cloned().collect(),
            cek_table: meta.cek_table.clone(),
        };
        let blob_meta = ColMetaData {
            columns: meta
                .columns
                .iter()
                .skip(first_blob)
                .take(blob_count)
                .cloned()
                .collect(),
            cek_table: meta.cek_table.clone(),
        };
        let scalar_columns = Client::<S>::build_columns(&prefix_meta);
        let blob_columns = Client::<S>::build_columns(&blob_meta);
        Self {
            client,
            buf,
            eom,
            encryption_enabled,
            meta,
            prefix_meta,
            scalar_row_meta: std::sync::Arc::new(crate::row::ColMetaData::new(scalar_columns)),
            blob_row_meta: std::sync::Arc::new(crate::row::ColMetaData::new(blob_columns)),
            first_blob,
            blob_count,
            // Start as if the previous row were fully consumed, so the first
            // `next` does not try to drain a nonexistent prior row.
            next_blob_idx: blob_count,
            current_blob_idx: None,
            current_nbc: None,
            auto_position_first,
            plp: None,
            blob_null: false,
            finished: true, // set false once construction succeeds below
        }
        .started()
    }

    fn started(mut self) -> Self {
        self.finished = false;
        self
    }

    /// The leading (scalar) columns of the result set — everything before the
    /// trailing MAX column(s).
    #[must_use]
    pub fn columns(&self) -> &[Column] {
        &self.scalar_row_meta.columns
    }

    /// The trailing MAX (blob) columns of the result set, in wire order.
    ///
    /// For a [`query_stream_blob`](crate::Client::query_stream_blob) stream this
    /// is a single column; for
    /// [`query_stream_rows`](crate::Client::query_stream_rows) it is the run of
    /// trailing MAX columns, in the order [`next_blob`](Self::next_blob) visits
    /// them.
    #[must_use]
    pub fn blob_columns(&self) -> &[Column] {
        &self.blob_row_meta.columns
    }

    /// The column metadata of the currently positioned blob, or `None` before
    /// the first blob of a row is positioned (or after the row is exhausted).
    #[must_use]
    pub fn current_blob_column(&self) -> Option<&Column> {
        self.current_blob_idx
            .and_then(|j| self.blob_row_meta.columns.get(j))
    }

    /// Advance to the next row, returning its scalar columns.
    ///
    /// Auto-drains any unread trailing blob(s) of the previous row, so the wire
    /// stays aligned. Returns `Ok(None)` at end of stream (connection clean).
    pub async fn next(&mut self) -> Result<Option<Row>> {
        if self.finished {
            return Ok(None);
        }
        self.drain_to_row_end().await?;

        loop {
            if self.buf.is_empty() {
                if !self.pull_packet().await? {
                    self.finish();
                    return Ok(None);
                }
                continue;
            }
            match self.buf[0] {
                0xD1 => return Ok(Some(self.decode_row().await?)),
                0xD2 => return Ok(Some(self.decode_nbc_row().await?)),
                _ => match self.parse_control_token().await? {
                    Control::Finished => {
                        self.finish();
                        return Ok(None);
                    }
                    Control::Continue => continue,
                },
            }
        }
    }

    /// Advance to the next trailing MAX column of the current row, returning
    /// `true` while one is positioned and `false` once the row's blobs are
    /// exhausted.
    ///
    /// Auto-drains the previously positioned blob (if not fully read) before
    /// advancing, so the wire stays aligned. After this returns `true`, read the
    /// blob with [`read_chunk`](Self::read_chunk) / [`copy_blob_to`](Self::copy_blob_to)
    /// and inspect it with [`current_blob_column`](Self::current_blob_column) /
    /// [`blob_is_null`](Self::blob_is_null) / [`blob_len`](Self::blob_len).
    ///
    /// This is the iteration primitive for
    /// [`query_stream_rows`](crate::Client::query_stream_rows):
    ///
    /// ```no_run
    /// # async fn ex(client: &mut mssql_client::Client<mssql_client::Ready>) -> Result<(), mssql_client::Error> {
    /// # let mut sink: Vec<u8> = Vec::new();
    /// let mut stream = client
    ///     .query_stream_rows("SELECT id, doc1, doc2 FROM files", &[])
    ///     .await?;
    /// while let Some(row) = stream.next().await? {
    ///     let id: i32 = row.get_by_name("id")?;
    ///     let _ = id;
    ///     while stream.next_blob().await? {
    ///         stream.copy_blob_to(&mut sink).await?; // each trailing MAX column
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn next_blob(&mut self) -> Result<bool> {
        if self.finished {
            return Ok(false);
        }
        self.drain_current_blob().await?;
        if self.next_blob_idx >= self.blob_count {
            self.current_blob_idx = None;
            return Ok(false);
        }
        self.position_next_blob();
        Ok(true)
    }

    /// Read the next chunk of the current blob, or `None` when it is fully
    /// read (or NULL). Reads more packets from the socket as needed.
    ///
    /// Chunks are **raw bytes**, not decoded text. For an `NVARCHAR(MAX)` /
    /// `XML` column the bytes are little-endian UCS-2, and a chunk boundary can
    /// fall in the middle of a two-byte code unit (or a surrogate pair) — so do
    /// not decode each chunk to `str` independently. Concatenate the chunks
    /// first (or stream to a byte sink), then decode the whole value.
    pub async fn read_chunk(&mut self) -> Result<Option<Bytes>> {
        loop {
            let event = match self.plp.as_mut() {
                Some(plp) if !plp.is_done() => plp.pull(&mut self.buf)?,
                _ => return Ok(None),
            };
            match event {
                PlpEvent::Data(d) => return Ok(Some(d)),
                PlpEvent::End => return Ok(None),
                PlpEvent::NeedMore => {
                    if !self.pull_packet().await? {
                        return Err(Error::ConnectionClosed);
                    }
                }
            }
        }
    }

    /// Stream the current row's blob to an async writer, returning bytes written.
    pub async fn copy_blob_to<W>(&mut self, w: &mut W) -> Result<u64>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        use tokio::io::AsyncWriteExt;
        let mut total = 0u64;
        while let Some(chunk) = self.read_chunk().await? {
            w.write_all(&chunk).await.map_err(Error::from)?;
            total += chunk.len() as u64;
        }
        Ok(total)
    }

    /// The current row's blob length in bytes, once known (after the first
    /// chunk is read). `None` before that, for a NULL blob, or for an
    /// unknown-length value.
    #[must_use]
    pub fn blob_len(&self) -> Option<u64> {
        if self.blob_null {
            return None;
        }
        self.plp.as_ref().and_then(PlpDecoder::total_len)
    }

    /// Whether the current row's blob is NULL.
    #[must_use]
    pub fn blob_is_null(&self) -> bool {
        self.blob_null
    }

    fn finish(&mut self) {
        self.finished = true;
        self.client.note_response_drained();
    }

    async fn decode_row(&mut self) -> Result<Row> {
        loop {
            let mut view: &[u8] = &self.buf[..];
            let before = view.len();
            view.advance(1); // ROW token byte
            match RawRow::decode_prefix(&mut view, &self.meta, self.first_blob) {
                Ok(raw) => {
                    let consumed = before - view.len();
                    self.buf.advance(consumed);
                    let row = crate::column_parser::convert_raw_row(
                        &raw,
                        &self.prefix_meta,
                        &self.scalar_row_meta,
                    )?;
                    self.begin_row(None);
                    return Ok(row);
                }
                Err(ProtocolError::UnexpectedEof) if !self.eom => {
                    self.pull_packet().await?;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    async fn decode_nbc_row(&mut self) -> Result<Row> {
        loop {
            let mut view: &[u8] = &self.buf[..];
            let before = view.len();
            view.advance(1); // NBCROW token byte
            match NbcRow::decode_prefix(&mut view, &self.meta, self.first_blob) {
                Ok(nbc) => {
                    let consumed = before - view.len();
                    self.buf.advance(consumed);
                    let row = crate::column_parser::convert_nbc_row(
                        &nbc,
                        &self.prefix_meta,
                        &self.scalar_row_meta,
                    )?;
                    self.begin_row(Some(nbc));
                    return Ok(row);
                }
                Err(ProtocolError::UnexpectedEof) if !self.eom => {
                    self.pull_packet().await?;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    /// Parse a single non-row token (Done / Error / Info / …) and decide whether
    /// the stream continues or has finished.
    async fn parse_control_token(&mut self) -> Result<Control> {
        loop {
            let mut parser =
                TokenParser::new(self.buf.clone()).with_encryption(self.encryption_enabled);
            match parser.next_token_with_metadata(Some(&self.meta)) {
                Ok(Some(token)) => {
                    let consumed = self.buf.len() - parser.remaining();
                    self.buf.advance(consumed);
                    return self.classify(token);
                }
                Ok(None) => {
                    if self.eom {
                        return Ok(Control::Finished);
                    }
                    self.pull_packet().await?;
                }
                Err(ProtocolError::UnexpectedEof | ProtocolError::IncompletePacket { .. })
                    if !self.eom =>
                {
                    self.pull_packet().await?;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    fn classify(&mut self, token: Token) -> Result<Control> {
        match token {
            Token::Done(d) => {
                if d.status.error {
                    return Err(Error::Query(
                        "query failed (server set error flag in DONE token)".to_string(),
                    ));
                }
                Ok(if d.status.more {
                    Control::Continue
                } else {
                    Control::Finished
                })
            }
            Token::Error(e) => Err(server_token_to_error(&e)),
            Token::ColMetaData(_) => Err(Error::Protocol(
                "blob streaming does not support multiple result sets".to_string(),
            )),
            Token::EnvChange(ref e) => {
                // Keep the transaction descriptor in sync with raw
                // BEGIN/COMMIT/ROLLBACK seen mid-stream, as the buffered
                // readers do.
                self.client.apply_transaction_env_change(e);
                Ok(Control::Continue)
            }
            // DoneProc / DoneInProc / Info / Order / etc.
            _ => Ok(Control::Continue),
        }
    }

    /// Reset per-row blob state after decoding a row's scalar prefix, optionally
    /// auto-positioning the first trailing blob (for the single-blob path).
    fn begin_row(&mut self, nbc: Option<NbcRow>) {
        self.current_nbc = nbc;
        self.next_blob_idx = 0;
        self.current_blob_idx = None;
        self.plp = None;
        self.blob_null = false;
        if self.auto_position_first {
            self.position_next_blob();
        }
    }

    /// Position the next trailing blob (sync; no socket IO). Sets up its PLP
    /// decoder, or marks it NULL when an NBCROW omitted its value.
    fn position_next_blob(&mut self) {
        let j = self.next_blob_idx;
        self.next_blob_idx += 1;
        self.current_blob_idx = Some(j);
        let col_idx = self.first_blob + j;
        let is_null = self
            .current_nbc
            .as_ref()
            .is_some_and(|n| n.is_null(col_idx));
        self.blob_null = is_null;
        self.plp = if is_null {
            None
        } else {
            Some(PlpDecoder::new())
        };
    }

    /// Drain the currently positioned blob off the wire (if not fully read).
    async fn drain_current_blob(&mut self) -> Result<()> {
        if self.plp.is_some() && !self.blob_null {
            while self.read_chunk().await?.is_some() {}
        }
        self.plp = None;
        Ok(())
    }

    /// Consume every remaining trailing blob of the current row off the wire, so
    /// the next ROW/NBCROW token is reachable. Drains the positioned blob, then
    /// positions and drains each not-yet-visited trailing blob in turn.
    async fn drain_to_row_end(&mut self) -> Result<()> {
        loop {
            self.drain_current_blob().await?;
            if self.next_blob_idx >= self.blob_count {
                self.current_blob_idx = None;
                return Ok(());
            }
            self.position_next_blob();
        }
    }

    /// Pull one packet onto the rolling buffer. Returns `false` at EOF.
    async fn pull_packet(&mut self) -> Result<bool> {
        match self.client.read_response_packet().await? {
            Some((payload, is_eom)) => {
                if self.buf.is_empty() {
                    self.buf = payload;
                } else {
                    let mut joined = BytesMut::with_capacity(self.buf.len() + payload.len());
                    joined.extend_from_slice(&self.buf);
                    joined.extend_from_slice(&payload);
                    self.buf = joined.freeze();
                }
                self.eom |= is_eom;
                Ok(true)
            }
            None => {
                self.eom = true;
                Ok(false)
            }
        }
    }
}

/// Outcome of parsing a non-row control token.
enum Control {
    Continue,
    Finished,
}
