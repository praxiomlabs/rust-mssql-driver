//! True incremental row streaming: rows pulled from the socket on demand.
//!
//! Unlike [`QueryStream`](crate::QueryStream) — which buffers the entire
//! response in memory and then decodes rows lazily — [`RowStream`] holds the
//! connection and reads TDS packets only as rows are pulled. Peak memory is
//! roughly one packet plus one partial row, independent of result-set size, so
//! a multi-million-row `SELECT` does not have to fit in client memory.
//!
//! Returned by [`Client::query_stream`](crate::Client::query_stream). The
//! stream borrows the client mutably for its lifetime, so no other operation
//! can run on the connection until the stream is consumed or dropped — natural
//! backpressure, and the type system enforces the exclusivity.
//!
//! ```no_run
//! # use mssql_client::{Client, Ready, Row};
//! # async fn ex(client: &mut Client<Ready>) -> Result<(), mssql_client::Error> {
//! let mut stream = client.query_stream("SELECT id, name FROM big_table", &[]).await?;
//! while let Some(row) = stream.try_next().await? {
//!     let id: i32 = row.get_by_name("id")?;
//!     let _ = id;
//! }
//! # Ok(())
//! # }
//! ```

use tds_protocol::token::{ColMetaData, Token};

use crate::Client;
use crate::error::{Error, Result};
use crate::row::{Column, Row};
use crate::row_source::{Pull, RowSource};
use crate::state::Ready;

/// An incrementally streamed result set: rows are read from the network as they
/// are pulled, not buffered up front.
///
/// See the [module docs](self) for how this differs from
/// [`QueryStream`](crate::QueryStream). Obtain one from
/// [`Client::query_stream`](crate::Client::query_stream).
///
/// Pulling is async (each row may require reading another packet), so this type
/// is consumed via [`try_next`](Self::try_next) / [`collect_all`](Self::collect_all)
/// rather than the synchronous [`Iterator`] that `QueryStream` offers.
#[must_use = "streams must be consumed; dropping a stream discards remaining rows"]
pub struct RowStream<'a> {
    /// The client whose connection supplies packets. Borrowed for the stream's
    /// lifetime so no other request can run concurrently.
    client: &'a mut Client<Ready>,
    /// The incremental token decoder over the rolling packet buffer.
    source: RowSource,
    /// Columns for the current result set (rebuilt on each ColMetaData).
    columns: Vec<Column>,
    /// Protocol metadata for decoding raw rows of the current result set.
    meta: ColMetaData,
    /// Pre-resolved column decryptor for the current Always Encrypted result set.
    #[cfg(feature = "always-encrypted")]
    decryptor: Option<std::sync::Arc<crate::column_decryptor::ColumnDecryptor>>,
    /// Whether the stream has reached the end of the response.
    finished: bool,
}

impl<'a> RowStream<'a> {
    /// Construct a stream positioned just after the first ColMetaData, ready to
    /// yield the result set's rows. Called by `Client::query_stream`.
    pub(crate) fn new(
        client: &'a mut Client<Ready>,
        source: RowSource,
        columns: Vec<Column>,
        meta: ColMetaData,
        #[cfg(feature = "always-encrypted")] decryptor: Option<
            std::sync::Arc<crate::column_decryptor::ColumnDecryptor>,
        >,
    ) -> Self {
        Self {
            client,
            source,
            columns,
            meta,
            #[cfg(feature = "always-encrypted")]
            decryptor,
            finished: false,
        }
    }

    /// Construct an already-finished stream (the query produced no result set,
    /// e.g. an `INSERT`). The caller has already cleared the in-flight flag.
    pub(crate) fn empty(client: &'a mut Client<Ready>) -> Self {
        Self {
            client,
            source: RowSource::new(false),
            columns: Vec::new(),
            meta: ColMetaData::default(),
            #[cfg(feature = "always-encrypted")]
            decryptor: None,
            finished: true,
        }
    }

    /// The columns of the current result set.
    #[must_use]
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// Whether the stream has been fully consumed.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Pull the next row, reading more packets from the connection as needed.
    ///
    /// Returns `Ok(None)` once the response is fully drained — at which point
    /// the connection is clean for the next request. A server error token in
    /// the stream is surfaced here as [`Error::Server`].
    pub async fn try_next(&mut self) -> Result<Option<Row>> {
        if self.finished {
            return Ok(None);
        }

        loop {
            match self.source.pull()? {
                Pull::Token(Token::Row(raw)) => return Ok(Some(self.decode_raw(&raw)?)),
                Pull::Token(Token::NbcRow(nbc)) => return Ok(Some(self.decode_nbc(&nbc)?)),
                Pull::Token(Token::ColMetaData(meta)) => {
                    // A new result set within the same response (multi-statement
                    // batch). Stream its rows flatly, continuing from here.
                    self.switch_result_set(meta).await?;
                }
                Pull::Token(Token::Error(err)) => {
                    self.finish();
                    return Err(crate::client::response::server_token_to_error(&err));
                }
                Pull::Token(Token::Done(done)) => {
                    if done.status.error {
                        self.finish();
                        return Err(Error::Query(
                            "query failed (server set error flag in DONE token)".to_string(),
                        ));
                    }
                    // Otherwise keep going: rows of another result set, or the
                    // final DONE followed by Pull::End, may still come.
                }
                Pull::Token(_) => {
                    // Info / EnvChange / Order / DoneProc / DoneInProc, etc.
                    // Not row data; keep pulling.
                }
                Pull::NeedMore => match self.client.read_response_packet().await? {
                    Some((payload, is_eom)) => self.source.push_packet(payload, is_eom),
                    None => {
                        self.finish();
                        return Err(Error::ConnectionClosed);
                    }
                },
                Pull::End => {
                    self.finish();
                    return Ok(None);
                }
            }
        }
    }

    /// Drain the remaining rows into a vector.
    ///
    /// For large result sets prefer [`try_next`](Self::try_next) — this loads
    /// every remaining row into memory at once.
    pub async fn collect_all(mut self) -> Result<Vec<Row>> {
        let mut out = Vec::new();
        while let Some(row) = self.try_next().await? {
            out.push(row);
        }
        Ok(out)
    }

    /// Stop the stream early and leave the connection reusable.
    ///
    /// Sends an Attention to the server and drains to its acknowledgement so the
    /// connection is clean for the next request — the correct way to abandon a
    /// large result set you no longer need.
    ///
    /// Calling this is optional: simply **dropping** a partially-read stream is
    /// safe but leaves the connection marked in-flight, so a pooled connection
    /// is discarded on return and a directly reused client recovers it (with an
    /// Attention/drain) on its next request. `cancel` avoids that discard and
    /// reports any error from the cancellation.
    pub async fn cancel(mut self) -> Result<()> {
        if self.finished {
            return Ok(());
        }
        self.finished = true;
        self.client.cancel_in_flight_response().await
    }

    /// Mark the stream finished and the connection clean for the next request.
    fn finish(&mut self) {
        self.finished = true;
        self.client.note_response_drained();
    }

    /// Adopt a new result set's metadata mid-stream (multi-statement batch).
    async fn switch_result_set(&mut self, meta: ColMetaData) -> Result<()> {
        self.columns = Client::<Ready>::build_columns(&meta);
        #[cfg(feature = "always-encrypted")]
        {
            self.decryptor = self
                .client
                .resolve_decryptor(&meta)
                .await?
                .map(std::sync::Arc::new);
        }
        self.meta = meta;
        Ok(())
    }

    /// Decode a raw row against the current result set's metadata.
    fn decode_raw(&self, raw: &tds_protocol::token::RawRow) -> Result<Row> {
        #[cfg(feature = "always-encrypted")]
        if let Some(ref dec) = self.decryptor {
            return crate::column_parser::convert_raw_row_decrypted(
                raw,
                &self.meta,
                &self.columns,
                dec,
            );
        }
        crate::column_parser::convert_raw_row(raw, &self.meta, &self.columns)
    }

    /// Decode a null-bitmap-compressed row against the current metadata.
    fn decode_nbc(&self, nbc: &tds_protocol::token::NbcRow) -> Result<Row> {
        #[cfg(feature = "always-encrypted")]
        if let Some(ref dec) = self.decryptor {
            return crate::column_parser::convert_nbc_row_decrypted(
                nbc,
                &self.meta,
                &self.columns,
                dec,
            );
        }
        crate::column_parser::convert_nbc_row(nbc, &self.meta, &self.columns)
    }
}
