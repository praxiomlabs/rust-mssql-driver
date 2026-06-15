//! Incremental, sans-IO token decoder for the streaming read path.
//!
//! Today the driver reads an *entire* response into one `Bytes` before parsing
//! a single token (`Connection::read_message` reassembles every packet; see
//! `client/response.rs`). That makes peak client memory proportional to the
//! whole result set — the buffering proven by the `streaming_memory` test.
//!
//! [`RowSource`] is the engine that replaces that. It holds a **rolling buffer**
//! of packet payloads and yields one token at a time. A token — a row, or a
//! column value — may straddle a TDS packet boundary, so the decoder must be
//! able to stop mid-token, accept more bytes, and resume. Rather than rewrite
//! the token parser as a resumable state machine, it reuses the existing
//! [`TokenParser`] and the fact that the per-type decoders signal a short buffer
//! with [`ProtocolError::UnexpectedEof`] / [`ProtocolError::IncompletePacket`]:
//! attempt to parse the next complete token from the buffered bytes; if the
//! buffer is short, report [`Pull::NeedMore`] so the caller feeds another
//! packet and retries. Peak buffer is therefore one packet plus one partial
//! token, not the whole response.
//!
//! This module is **sans-IO**: it never touches a socket. The caller drives it
//! by reading packets (`Connection::read_packet`) and handing the payloads to
//! [`RowSource::push_packet`]. That keeps the resumption logic — the part that
//! is easy to get wrong across packet splits — unit-testable synchronously
//! against synthesized byte streams, independent of any async transport. Wiring
//! it drives [`RowStream`](crate::RowStream), the incremental streaming read
//! path; the buffered eager path ([`QueryStream`](crate::QueryStream)) is
//! unchanged.

use bytes::{Bytes, BytesMut};
use tds_protocol::ProtocolError;
use tds_protocol::token::{ColMetaData, Token, TokenParser};

/// Result of asking a [`RowSource`] for the next token.
#[derive(Debug)]
pub(crate) enum Pull {
    /// A complete token was decoded from the buffered bytes.
    Token(Token),
    /// The buffer holds the start of a token but not all of it. The caller must
    /// push another packet and call [`RowSource::pull`] again.
    NeedMore,
    /// All buffered bytes are consumed and the final (end-of-message) packet has
    /// been pushed — the response is fully drained.
    End,
}

/// Incremental token decoder over a rolling buffer of packet payloads.
///
/// See the [module docs](self) for the design.
pub(crate) struct RowSource {
    /// Unconsumed bytes accumulated from pushed packets. Sliced forward as
    /// tokens are consumed so the head allocation is released once spent.
    buf: Bytes,
    /// Most recent column metadata, needed to decode `Row` / `NbcRow` tokens.
    metadata: Option<ColMetaData>,
    /// Whether Always Encrypted was negotiated (selects the encrypted
    /// `ColMetaData` layout, mirroring the eager parser).
    encryption_enabled: bool,
    /// Set once the end-of-message packet has been pushed: no more bytes will
    /// arrive, so a short buffer is a truncated stream rather than `NeedMore`.
    eom: bool,
}

impl RowSource {
    /// Create an empty row source.
    pub(crate) fn new(encryption_enabled: bool) -> Self {
        Self {
            buf: Bytes::new(),
            metadata: None,
            encryption_enabled,
            eom: false,
        }
    }

    /// Append one packet's payload to the rolling buffer.
    ///
    /// `is_eom` is the packet's END_OF_MESSAGE status: once a packet with it set
    /// is pushed, the buffer can no longer grow, so a partial trailing token is
    /// an error rather than [`Pull::NeedMore`].
    pub(crate) fn push_packet(&mut self, payload: Bytes, is_eom: bool) {
        if self.buf.is_empty() {
            // Common case: the previous packet's bytes were fully consumed.
            // Adopt the new payload directly — no copy, and the old (now spent)
            // allocation is dropped.
            self.buf = payload;
        } else {
            // A token straddles the boundary: concatenate the unconsumed tail
            // with the new payload. Bounded by one partial token + one packet.
            let mut joined = BytesMut::with_capacity(self.buf.len() + payload.len());
            joined.extend_from_slice(&self.buf);
            joined.extend_from_slice(&payload);
            self.buf = joined.freeze();
        }
        self.eom |= is_eom;
    }

    /// Whether the end-of-message packet has been observed.
    #[cfg(test)]
    pub(crate) fn is_eom(&self) -> bool {
        self.eom
    }

    /// Consume the source, returning its unconsumed buffer and end-of-message
    /// flag. Used to hand the post-metadata buffer to the BLOB streaming path,
    /// which decodes rows column-by-column rather than whole-row.
    pub(crate) fn into_parts(self) -> (Bytes, bool) {
        (self.buf, self.eom)
    }

    /// Attempt to decode the next token from the buffered bytes.
    ///
    /// Returns [`Pull::Token`] on success (advancing past the consumed bytes),
    /// [`Pull::NeedMore`] if more packets are needed to complete the token, or
    /// [`Pull::End`] when the buffer is drained and the stream has ended. A
    /// short buffer *after* the end-of-message packet is a truncated-response
    /// error.
    pub(crate) fn pull(&mut self) -> Result<Pull, ProtocolError> {
        if self.buf.is_empty() {
            return Ok(if self.eom { Pull::End } else { Pull::NeedMore });
        }

        // `Bytes::clone` is a refcount bump and the parser starts at offset 0 of
        // the slice; we track consumption ourselves via `remaining()`. A partial
        // token leaves `self.buf` untouched so the retry re-parses from the same
        // position once more bytes are appended.
        let mut parser =
            TokenParser::new(self.buf.clone()).with_encryption(self.encryption_enabled);

        match parser.next_token_with_metadata(self.metadata.as_ref()) {
            Ok(Some(token)) => {
                let consumed = self.buf.len() - parser.remaining();
                self.buf = self.buf.slice(consumed..);
                if let Token::ColMetaData(meta) = &token {
                    self.metadata = Some(meta.clone());
                }
                Ok(Pull::Token(token))
            }
            // The buffer ended exactly on a token boundary: either done (eom) or
            // awaiting the next packet.
            Ok(None) => Ok(if self.eom { Pull::End } else { Pull::NeedMore }),
            // A short buffer is recoverable only while more packets can arrive.
            Err(ProtocolError::UnexpectedEof | ProtocolError::IncompletePacket { .. })
                if !self.eom =>
            {
                Ok(Pull::NeedMore)
            }
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    /// COLMETADATA token (0x81) for two columns: `INT4 id`, `NVARCHAR(50) name`.
    fn colmetadata() -> Vec<u8> {
        let mut v = vec![0x81]; // ColMetaData token
        v.extend_from_slice(&[0x02, 0x00]); // 2 columns
        // Column 1: INT4 "id"
        v.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // user_type
        v.extend_from_slice(&[0x01, 0x00]); // flags (nullable)
        v.push(0x38); // TypeId::Int4
        v.push(0x02); // name length (chars)
        v.extend_from_slice(&[b'i', 0x00, b'd', 0x00]);
        // Column 2: NVARCHAR(50) "name"
        v.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // user_type
        v.extend_from_slice(&[0x01, 0x00]); // flags (nullable)
        v.push(0xE7); // TypeId::NVarChar
        v.extend_from_slice(&[0x64, 0x00]); // max_length = 100
        v.extend_from_slice(&[0x09, 0x04, 0xD0, 0x00, 0x34]); // collation
        v.push(0x04); // name length (chars)
        v.extend_from_slice(&[b'n', 0x00, b'a', 0x00, b'm', 0x00, b'e', 0x00]);
        v
    }

    /// ROW token (0xD1) for `(id: i32, name: &str | NULL)`.
    fn row(id: i32, name: Option<&str>) -> Vec<u8> {
        let mut v = vec![0xD1];
        v.extend_from_slice(&id.to_le_bytes());
        match name {
            None => v.extend_from_slice(&[0xFF, 0xFF]), // NVARCHAR NULL
            Some(s) => {
                let utf16: Vec<u8> = s.encode_utf16().flat_map(u16::to_le_bytes).collect();
                v.extend_from_slice(&(utf16.len() as u16).to_le_bytes());
                v.extend_from_slice(&utf16);
            }
        }
        v
    }

    /// DONE token (0xFD): status + curcmd + 8-byte row count.
    fn done(row_count: u64) -> Vec<u8> {
        let mut v = vec![0xFD];
        v.extend_from_slice(&[0x10, 0x00]); // DONE_COUNT
        v.extend_from_slice(&[0xC1, 0x00]); // curcmd
        v.extend_from_slice(&row_count.to_le_bytes());
        v
    }

    /// A complete three-row response.
    fn sample_response() -> Vec<u8> {
        let mut v = colmetadata();
        v.extend_from_slice(&row(1, Some("alpha")));
        v.extend_from_slice(&row(2, None));
        v.extend_from_slice(&row(3, Some("gamma")));
        v.extend_from_slice(&done(3));
        v
    }

    /// Row payloads collected from a source, as `(is_nbc, data)` for comparison.
    type CollectedRow = (bool, Bytes);

    /// Drain whatever tokens are currently decodable, collecting row payloads.
    /// Returns `true` once the stream has ended.
    fn drain(src: &mut RowSource, rows: &mut Vec<CollectedRow>) -> bool {
        loop {
            match src.pull().expect("pull must not error on valid input") {
                Pull::Token(Token::Row(r)) => rows.push((false, r.data)),
                Pull::Token(Token::NbcRow(r)) => rows.push((true, r.data)),
                Pull::Token(_) => {} // metadata / done / etc.
                Pull::NeedMore => return false,
                Pull::End => return true,
            }
        }
    }

    /// Reference: the eager parser's row payloads for the same bytes.
    fn eager_rows(bytes: &[u8]) -> Vec<CollectedRow> {
        let mut parser = TokenParser::new(Bytes::copy_from_slice(bytes));
        let mut meta: Option<ColMetaData> = None;
        let mut rows = Vec::new();
        while let Some(token) = parser
            .next_token_with_metadata(meta.as_ref())
            .expect("eager parse")
        {
            match token {
                Token::ColMetaData(m) => meta = Some(m),
                Token::Row(r) => rows.push((false, r.data)),
                Token::NbcRow(r) => rows.push((true, r.data)),
                _ => {}
            }
        }
        rows
    }

    #[test]
    fn whole_response_in_one_packet() {
        let full = sample_response();
        let mut src = RowSource::new(false);
        src.push_packet(Bytes::copy_from_slice(&full), true);

        let mut rows = Vec::new();
        assert!(drain(&mut src, &mut rows));
        assert_eq!(rows, eager_rows(&full));
        assert_eq!(rows.len(), 3);
    }

    /// The core falsification: splitting the response into two packets at *every*
    /// byte boundary must yield exactly the eager parser's rows. A parser that
    /// can't resume mid-token (mid-row, mid-column, mid-metadata, mid-header)
    /// fails at the boundaries that land inside a token.
    #[test]
    fn every_two_packet_split_matches_eager() {
        let full = sample_response();
        let reference = eager_rows(&full);

        for split in 0..=full.len() {
            let mut src = RowSource::new(false);
            let mut rows = Vec::new();

            src.push_packet(Bytes::copy_from_slice(&full[..split]), false);
            let ended_early = drain(&mut src, &mut rows);
            assert!(!ended_early, "must not end before eom (split {split})");

            src.push_packet(Bytes::copy_from_slice(&full[split..]), true);
            let ended = drain(&mut src, &mut rows);

            assert!(ended, "stream must end after eom (split {split})");
            assert_eq!(rows, reference, "rows differ at split {split}");
        }
    }

    /// The pathological feed: every single byte as its own non-terminal packet.
    /// This exercises resumption at all boundaries simultaneously.
    #[test]
    fn byte_by_byte_feed_matches_eager() {
        let full = sample_response();
        let reference = eager_rows(&full);

        let mut src = RowSource::new(false);
        let mut rows = Vec::new();

        for (i, b) in full.iter().enumerate() {
            let is_last = i == full.len() - 1;
            src.push_packet(Bytes::copy_from_slice(&[*b]), is_last);
            drain(&mut src, &mut rows);
        }
        assert_eq!(rows, reference);
        assert_eq!(rows.len(), 3);
    }

    /// An empty buffer before eom is `NeedMore`; after eom it is `End`.
    #[test]
    fn empty_buffer_reports_need_more_then_end() {
        let mut src = RowSource::new(false);
        assert!(matches!(src.pull().unwrap(), Pull::NeedMore));
        assert!(!src.is_eom());

        src.push_packet(Bytes::new(), true);
        assert!(src.is_eom());
        assert!(matches!(src.pull().unwrap(), Pull::End));
    }

    /// A trailing partial token *after* the end-of-message packet is a truncated
    /// response, surfaced as an error rather than an infinite `NeedMore` loop.
    #[test]
    fn truncated_token_after_eom_errors() {
        // ROW token header for the two-column schema, but only 2 of the 4 int
        // bytes present, with eom set: the stream is malformed/truncated.
        let mut src = RowSource::new(false);
        src.push_packet(Bytes::copy_from_slice(&colmetadata()), false);
        // consume the metadata
        let mut rows = Vec::new();
        drain(&mut src, &mut rows);

        src.push_packet(Bytes::copy_from_slice(&[0xD1, 0x2A, 0x00]), true);
        let err = src.pull();
        assert!(
            err.is_err(),
            "truncated token after eom must error, got {err:?}"
        );
    }

    /// Metadata buffered separately from its rows is retained: a `Row` pulled
    /// only after a later packet still decodes against the earlier metadata.
    #[test]
    fn metadata_persists_across_packets() {
        let mut src = RowSource::new(false);
        src.push_packet(Bytes::copy_from_slice(&colmetadata()), false);

        let mut rows = Vec::new();
        assert!(!drain(&mut src, &mut rows)); // metadata consumed, no rows yet
        assert!(rows.is_empty());

        let mut tail = row(7, Some("z"));
        tail.extend_from_slice(&done(1));
        src.push_packet(Bytes::copy_from_slice(&tail), true);
        assert!(drain(&mut src, &mut rows));

        assert_eq!(rows.len(), 1);
        // id=7 little-endian followed by the NVARCHAR length + "z" UTF-16LE.
        assert_eq!(&rows[0].1[..4], &7i32.to_le_bytes());
    }
}
