//! Sans-IO resumable decoder for PLP (Partially Length-Prefixed) values.
//!
//! MAX columns (`VARBINARY(MAX)`, `NVARCHAR(MAX)`, `VARCHAR(MAX)`, `XML`, large
//! UDT) are sent on the wire as a PLP value:
//!
//! ```text
//! [u64 total_len]                          ; 0xFFFF…FF = NULL, 0xFFFF…FE = UNKNOWN
//! ( [u32 chunk_len] [chunk_len bytes] )*   ; one or more data chunks
//! [u32 0]                                  ; terminator
//! ```
//!
//! Today the whole value is buffered into the row. To stream a multi-GB cell
//! without materializing it, [`PlpDecoder`] extracts chunk data **incrementally**
//! from a rolling buffer: the length prefix, a chunk header, or a chunk body can
//! each straddle a TDS packet boundary, so the decoder stops on a short buffer
//! ([`PlpEvent::NeedMore`]), the caller appends another packet, and it resumes.
//! Peak memory is one chunk slice, not the whole value.
//!
//! This module is sans-IO: it never touches a socket. The async BLOB reader
//! ([`BlobStream`](crate::BlobStream)) drives it by reading packets and
//! refilling the buffer, exactly as [`RowSource`](crate::row_source::RowSource)
//! drives token decoding.

use bytes::{Buf, Bytes};

use tds_protocol::ProtocolError;

/// `total_len` sentinel: the value is NULL.
const PLP_NULL: u64 = 0xFFFF_FFFF_FFFF_FFFF;
/// `total_len` sentinel: the total length is not known up front (chunks are
/// still terminated by a zero-length chunk).
const PLP_UNKNOWN_LEN: u64 = 0xFFFF_FFFF_FFFF_FFFE;

/// Resumable PLP value decoder over a caller-supplied rolling buffer.
#[derive(Debug)]
pub(crate) struct PlpDecoder {
    state: State,
    /// Declared total length, once the prefix is read. `None` for NULL or
    /// UNKNOWN-length values.
    total_len: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
enum State {
    /// Awaiting the 8-byte total-length prefix.
    NeedTotalLen,
    /// Awaiting a 4-byte chunk-length header.
    NeedChunkHeader,
    /// Mid-chunk, with this many data bytes still to emit.
    InChunk(u32),
    /// Terminator reached, or the value was NULL.
    Done,
}

/// Outcome of pulling from a [`PlpDecoder`].
#[derive(Debug)]
pub(crate) enum PlpEvent {
    /// A slice of value bytes (a whole chunk or the part of one currently
    /// buffered). Zero-copy into the rolling buffer.
    Data(Bytes),
    /// The buffer holds less than the next length/chunk needs. Append another
    /// packet and pull again.
    NeedMore,
    /// The value is fully decoded (terminator consumed, or it was NULL).
    End,
}

impl PlpDecoder {
    pub(crate) fn new() -> Self {
        Self {
            state: State::NeedTotalLen,
            total_len: None,
        }
    }

    /// The declared total length in bytes, once known. `None` before the prefix
    /// is read, for a NULL value, or for an UNKNOWN-length value.
    pub(crate) fn total_len(&self) -> Option<u64> {
        self.total_len
    }

    /// Whether the value was NULL (known only after the prefix is consumed).
    pub(crate) fn is_done(&self) -> bool {
        matches!(self.state, State::Done)
    }

    /// Pull the next available value bytes from `buf`, advancing it past what is
    /// consumed. Returns [`PlpEvent::NeedMore`] without consuming a partial
    /// length/header so the caller can append more bytes and retry.
    pub(crate) fn pull(&mut self, buf: &mut Bytes) -> Result<PlpEvent, ProtocolError> {
        loop {
            match self.state {
                State::Done => return Ok(PlpEvent::End),
                State::NeedTotalLen => {
                    if buf.remaining() < 8 {
                        return Ok(PlpEvent::NeedMore);
                    }
                    let total = buf.get_u64_le();
                    if total == PLP_NULL {
                        self.state = State::Done;
                        return Ok(PlpEvent::End);
                    }
                    if total != PLP_UNKNOWN_LEN {
                        self.total_len = Some(total);
                    }
                    self.state = State::NeedChunkHeader;
                }
                State::NeedChunkHeader => {
                    if buf.remaining() < 4 {
                        return Ok(PlpEvent::NeedMore);
                    }
                    let chunk_len = buf.get_u32_le();
                    if chunk_len == 0 {
                        self.state = State::Done;
                        return Ok(PlpEvent::End);
                    }
                    self.state = State::InChunk(chunk_len);
                }
                State::InChunk(remaining) => {
                    if buf.is_empty() {
                        return Ok(PlpEvent::NeedMore);
                    }
                    let take = (remaining as usize).min(buf.len());
                    let data = buf.split_to(take);
                    let left = remaining - take as u32;
                    self.state = if left == 0 {
                        State::NeedChunkHeader
                    } else {
                        State::InChunk(left)
                    };
                    return Ok(PlpEvent::Data(data));
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use bytes::{BufMut, BytesMut};

    /// Encode `value` (or NULL) as a PLP value with the given chunk size.
    fn encode_plp(value: Option<&[u8]>, chunk_size: usize) -> Vec<u8> {
        let mut v = Vec::new();
        match value {
            None => {
                v.extend_from_slice(&PLP_NULL.to_le_bytes());
            }
            Some(data) => {
                v.extend_from_slice(&(data.len() as u64).to_le_bytes());
                for chunk in data.chunks(chunk_size.max(1)) {
                    v.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
                    v.extend_from_slice(chunk);
                }
                v.extend_from_slice(&0u32.to_le_bytes()); // terminator
            }
        }
        v
    }

    /// Drive a decoder over `wire` fed in `feed`-sized packets; return the
    /// reassembled value bytes and whether it ended cleanly.
    fn decode_in_packets(wire: &[u8], feed: usize) -> (Vec<u8>, bool) {
        let mut dec = PlpDecoder::new();
        let mut buf = Bytes::new();
        let mut out = Vec::new();
        let mut pos = 0;
        loop {
            match dec.pull(&mut buf).expect("pull") {
                PlpEvent::Data(d) => out.extend_from_slice(&d),
                PlpEvent::End => return (out, true),
                PlpEvent::NeedMore => {
                    if pos >= wire.len() {
                        return (out, false); // truncated
                    }
                    let end = (pos + feed.max(1)).min(wire.len());
                    let mut next = BytesMut::from(&buf[..]);
                    next.put_slice(&wire[pos..end]);
                    buf = next.freeze();
                    pos = end;
                }
            }
        }
    }

    #[test]
    fn single_chunk_roundtrip() {
        let data = b"hello world";
        let wire = encode_plp(Some(data), 100);
        let (out, ended) = decode_in_packets(&wire, wire.len());
        assert!(ended);
        assert_eq!(out, data);
    }

    #[test]
    fn null_value() {
        let wire = encode_plp(None, 1);
        let mut dec = PlpDecoder::new();
        let mut buf = Bytes::copy_from_slice(&wire);
        assert!(matches!(dec.pull(&mut buf).unwrap(), PlpEvent::End));
        assert!(dec.is_done());
        assert_eq!(dec.total_len(), None);
    }

    #[test]
    fn total_len_is_reported() {
        let data = vec![7u8; 5000];
        let wire = encode_plp(Some(&data), 1024);
        let mut dec = PlpDecoder::new();
        let mut buf = Bytes::copy_from_slice(&wire);
        // First pull reads the prefix + first chunk slice.
        let _ = dec.pull(&mut buf).unwrap();
        assert_eq!(dec.total_len(), Some(5000));
    }

    /// The core falsification: a multi-chunk value fed one byte per packet (so
    /// the length prefix, every chunk header, and every chunk body straddle a
    /// boundary) must reassemble exactly. A non-resumable decoder fails here.
    #[test]
    fn byte_by_byte_feed_reassembles() {
        let data: Vec<u8> = (0..3000u32).map(|i| (i % 251) as u8).collect();
        let wire = encode_plp(Some(&data), 256); // many chunks
        let (out, ended) = decode_in_packets(&wire, 1);
        assert!(ended);
        assert_eq!(out, data);
    }

    /// Every packet-split boundary must reassemble identically.
    #[test]
    fn every_split_reassembles() {
        let data: Vec<u8> = (0..777u32).map(|i| (i % 97) as u8).collect();
        let wire = encode_plp(Some(&data), 64);
        for feed in 1..=wire.len() {
            let (out, ended) = decode_in_packets(&wire, feed);
            assert!(ended, "feed {feed} did not end");
            assert_eq!(out, data, "feed {feed} mismatch");
        }
    }

    /// An empty (zero-length) MAX value: prefix 0, immediate terminator.
    #[test]
    fn empty_value() {
        let wire = encode_plp(Some(&[]), 1);
        let (out, ended) = decode_in_packets(&wire, 1);
        assert!(ended);
        assert!(out.is_empty());
    }
}
