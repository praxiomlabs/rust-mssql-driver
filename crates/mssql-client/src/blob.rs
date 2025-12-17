//! Streaming large object (LOB) reader for VARBINARY(MAX) and TEXT types.
//!
//! This module provides `BlobReader` for streaming large binary objects without
//! requiring the entire object in memory at once (from the user's perspective).
//!
//! ## Supported Types
//!
//! - `VARBINARY(MAX)` - Binary data up to 2GB
//! - `VARCHAR(MAX)` / `NVARCHAR(MAX)` - Text data up to 2GB
//! - `TEXT` / `NTEXT` / `IMAGE` - Legacy types (prefer MAX variants)
//! - `XML` - XML documents
//!
//! ## Usage
//!
//! ```rust,ignore
//! use mssql_client::blob::BlobReader;
//! use tokio::io::AsyncReadExt;
//!
//! // Get binary column as a blob reader
//! let data: Bytes = row.get(0)?;
//! let mut reader = BlobReader::from_bytes(data);
//!
//! // Read in chunks
//! let mut buffer = vec![0u8; 8192];
//! loop {
//!     let n = reader.read(&mut buffer).await?;
//!     if n == 0 {
//!         break;
//!     }
//!     process_chunk(&buffer[..n]);
//! }
//!
//! // Or copy directly to a file
//! let mut file = tokio::fs::File::create("output.bin").await?;
//! tokio::io::copy(&mut reader, &mut file).await?;
//! ```
//!
//! ## Memory Model
//!
//! The current implementation buffers the complete LOB data internally (received
//! from SQL Server as a single `Bytes` allocation). The `BlobReader` API enables:
//!
//! - Chunked processing without additional allocations
//! - Streaming to files or other destinations
//! - Compatible API for future true-streaming implementation
//!
//! For LOBs under 100MB, this buffering approach is acceptable. For larger objects,
//! consider application-level chunking via SQL `SUBSTRING` queries.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use tokio::io::{AsyncRead, ReadBuf};

/// Streaming reader for large binary objects (LOBs).
///
/// `BlobReader` implements [`AsyncRead`] to provide a streaming interface for
/// reading large objects. Data can be read in chunks without allocating
/// additional buffers for each read operation.
///
/// # Example
///
/// ```rust,ignore
/// use mssql_client::blob::BlobReader;
/// use tokio::io::AsyncReadExt;
///
/// let data: Bytes = row.get("binary_column")?;
/// let mut reader = BlobReader::from_bytes(data);
///
/// // Read up to 4KB at a time
/// let mut buffer = vec![0u8; 4096];
/// let bytes_read = reader.read(&mut buffer).await?;
/// ```
pub struct BlobReader {
    /// The underlying buffer containing LOB data.
    buffer: Bytes,
    /// Current read position within the buffer.
    position: usize,
}

impl BlobReader {
    /// Create a new `BlobReader` from a `Bytes` buffer.
    ///
    /// This is the primary constructor for creating a blob reader from
    /// column data retrieved from a query result.
    ///
    /// # Example
    ///
    /// ```rust
    /// use bytes::Bytes;
    /// use mssql_client::blob::BlobReader;
    ///
    /// let data = Bytes::from_static(b"Hello, World!");
    /// let reader = BlobReader::from_bytes(data);
    /// assert_eq!(reader.len(), Some(13));
    /// ```
    #[must_use]
    pub fn from_bytes(buffer: Bytes) -> Self {
        Self {
            buffer,
            position: 0,
        }
    }

    /// Create an empty `BlobReader`.
    ///
    /// Returns a reader with no data that will immediately return EOF.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            buffer: Bytes::new(),
            position: 0,
        }
    }

    /// Create a `BlobReader` from a byte slice.
    ///
    /// This copies the data into an owned `Bytes` buffer.
    #[must_use]
    pub fn from_slice(data: &[u8]) -> Self {
        Self {
            buffer: Bytes::copy_from_slice(data),
            position: 0,
        }
    }

    /// Get the total length of the BLOB in bytes.
    ///
    /// Returns `Some(length)` with the total size of the data.
    /// Returns `None` only for truly streaming implementations where
    /// the total length is unknown in advance.
    ///
    /// # Example
    ///
    /// ```rust
    /// use bytes::Bytes;
    /// use mssql_client::blob::BlobReader;
    ///
    /// let reader = BlobReader::from_bytes(Bytes::from_static(b"test"));
    /// assert_eq!(reader.len(), Some(4));
    /// ```
    #[must_use]
    pub fn len(&self) -> Option<u64> {
        Some(self.buffer.len() as u64)
    }

    /// Check if the BLOB is empty.
    ///
    /// Returns `true` if the BLOB contains no data.
    ///
    /// # Example
    ///
    /// ```rust
    /// use bytes::Bytes;
    /// use mssql_client::blob::BlobReader;
    ///
    /// let empty = BlobReader::empty();
    /// assert!(empty.is_empty());
    ///
    /// let non_empty = BlobReader::from_bytes(Bytes::from_static(b"data"));
    /// assert!(!non_empty.is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Get the number of bytes read so far.
    ///
    /// This tracks progress through the BLOB and can be used for
    /// progress reporting.
    ///
    /// # Example
    ///
    /// ```rust
    /// use bytes::Bytes;
    /// use mssql_client::blob::BlobReader;
    ///
    /// let reader = BlobReader::from_bytes(Bytes::from_static(b"Hello"));
    /// assert_eq!(reader.bytes_read(), 0);
    /// ```
    #[must_use]
    pub fn bytes_read(&self) -> u64 {
        self.position as u64
    }

    /// Get the number of bytes remaining to be read.
    ///
    /// Returns `Some(remaining)` if the total length is known.
    #[must_use]
    pub fn remaining(&self) -> Option<u64> {
        self.len()
            .map(|total| total.saturating_sub(self.position as u64))
    }

    /// Reset the reader position to the beginning.
    ///
    /// After calling this, `bytes_read()` will return 0 and subsequent
    /// reads will start from the beginning of the data.
    pub fn rewind(&mut self) {
        self.position = 0;
    }

    /// Consume the reader and return the underlying `Bytes` buffer.
    ///
    /// This returns the complete data, including any bytes already read.
    /// The buffer is not consumed by reads; it remains complete.
    ///
    /// # Example
    ///
    /// ```rust
    /// use bytes::Bytes;
    /// use mssql_client::blob::BlobReader;
    ///
    /// let data = Bytes::from_static(b"Hello");
    /// let reader = BlobReader::from_bytes(data.clone());
    /// let recovered = reader.into_bytes();
    /// assert_eq!(recovered, data);
    /// ```
    #[must_use]
    pub fn into_bytes(self) -> Bytes {
        self.buffer
    }

    /// Get a reference to the underlying bytes.
    ///
    /// Returns the complete buffer, not just unread bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &Bytes {
        &self.buffer
    }

    /// Get the unread portion as a slice.
    ///
    /// Returns a slice of the data that hasn't been read yet.
    #[must_use]
    pub fn unread_slice(&self) -> &[u8] {
        &self.buffer[self.position..]
    }

    /// Check if all data has been read.
    ///
    /// Returns `true` if `bytes_read() == len()`.
    #[must_use]
    pub fn is_exhausted(&self) -> bool {
        self.position >= self.buffer.len()
    }
}

impl Default for BlobReader {
    fn default() -> Self {
        Self::empty()
    }
}

impl AsyncRead for BlobReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // Calculate how many bytes we can read
        let remaining = self.buffer.len().saturating_sub(self.position);
        if remaining == 0 {
            // EOF - no more data to read
            return Poll::Ready(Ok(()));
        }

        // Read up to the buffer capacity or remaining data, whichever is smaller
        let to_read = remaining.min(buf.remaining());
        let end = self.position + to_read;

        // Copy data to the read buffer
        buf.put_slice(&self.buffer[self.position..end]);

        // Update position
        self.position = end;

        Poll::Ready(Ok(()))
    }
}

impl std::fmt::Debug for BlobReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlobReader")
            .field("total_len", &self.buffer.len())
            .field("position", &self.position)
            .field("remaining", &self.remaining())
            .finish()
    }
}

impl Clone for BlobReader {
    /// Clone the reader.
    ///
    /// The cloned reader shares the underlying `Bytes` buffer (cheap clone)
    /// but has its own position, starting from the beginning.
    fn clone(&self) -> Self {
        Self {
            buffer: self.buffer.clone(),
            position: 0, // Clone starts from beginning
        }
    }
}

impl From<Bytes> for BlobReader {
    fn from(bytes: Bytes) -> Self {
        Self::from_bytes(bytes)
    }
}

impl From<Vec<u8>> for BlobReader {
    fn from(vec: Vec<u8>) -> Self {
        Self::from_bytes(Bytes::from(vec))
    }
}

impl From<&[u8]> for BlobReader {
    fn from(slice: &[u8]) -> Self {
        Self::from_slice(slice)
    }
}

impl From<&str> for BlobReader {
    fn from(s: &str) -> Self {
        Self::from_slice(s.as_bytes())
    }
}

impl From<String> for BlobReader {
    fn from(s: String) -> Self {
        Self::from_bytes(Bytes::from(s))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    #[test]
    fn test_blob_reader_creation() {
        let data = Bytes::from_static(b"Hello, World!");
        let reader = BlobReader::from_bytes(data);

        assert_eq!(reader.len(), Some(13));
        assert!(!reader.is_empty());
        assert_eq!(reader.bytes_read(), 0);
        assert_eq!(reader.remaining(), Some(13));
    }

    #[test]
    fn test_blob_reader_empty() {
        let reader = BlobReader::empty();

        assert_eq!(reader.len(), Some(0));
        assert!(reader.is_empty());
        assert!(reader.is_exhausted());
    }

    #[test]
    fn test_blob_reader_from_slice() {
        let reader = BlobReader::from_slice(b"test data");

        assert_eq!(reader.len(), Some(9));
        assert_eq!(reader.as_bytes().as_ref(), b"test data");
    }

    #[tokio::test]
    async fn test_blob_reader_read_all() {
        let data = Bytes::from_static(b"Hello, World!");
        let mut reader = BlobReader::from_bytes(data);

        let mut output = Vec::new();
        reader.read_to_end(&mut output).await.unwrap();

        assert_eq!(output, b"Hello, World!");
        assert_eq!(reader.bytes_read(), 13);
        assert!(reader.is_exhausted());
    }

    #[tokio::test]
    async fn test_blob_reader_read_chunked() {
        let data = Bytes::from_static(b"0123456789ABCDEF");
        let mut reader = BlobReader::from_bytes(data);

        let mut buffer = [0u8; 4];

        // Read first chunk
        let n = reader.read(&mut buffer).await.unwrap();
        assert_eq!(n, 4);
        assert_eq!(&buffer, b"0123");
        assert_eq!(reader.bytes_read(), 4);

        // Read second chunk
        let n = reader.read(&mut buffer).await.unwrap();
        assert_eq!(n, 4);
        assert_eq!(&buffer, b"4567");
        assert_eq!(reader.bytes_read(), 8);

        // Read remaining
        let mut remaining = Vec::new();
        reader.read_to_end(&mut remaining).await.unwrap();
        assert_eq!(remaining, b"89ABCDEF");
        assert!(reader.is_exhausted());
    }

    #[tokio::test]
    async fn test_blob_reader_empty_read() {
        let mut reader = BlobReader::empty();

        let mut buffer = [0u8; 10];
        let n = reader.read(&mut buffer).await.unwrap();

        assert_eq!(n, 0); // EOF immediately
    }

    #[test]
    fn test_blob_reader_rewind() {
        let data = Bytes::from_static(b"test");
        let mut reader = BlobReader::from_bytes(data);

        // Simulate reading
        reader.position = 4;
        assert!(reader.is_exhausted());

        // Rewind
        reader.rewind();
        assert_eq!(reader.bytes_read(), 0);
        assert!(!reader.is_exhausted());
    }

    #[test]
    fn test_blob_reader_into_bytes() {
        let data = Bytes::from_static(b"Hello");
        let reader = BlobReader::from_bytes(data.clone());

        // Consume and get bytes back
        let recovered = reader.into_bytes();
        assert_eq!(recovered, data);
    }

    #[test]
    fn test_blob_reader_unread_slice() {
        let data = Bytes::from_static(b"Hello");
        let mut reader = BlobReader::from_bytes(data);

        assert_eq!(reader.unread_slice(), b"Hello");

        reader.position = 2;
        assert_eq!(reader.unread_slice(), b"llo");
    }

    #[test]
    fn test_blob_reader_clone() {
        let data = Bytes::from_static(b"test");
        let mut original = BlobReader::from_bytes(data);
        original.position = 2;

        let cloned = original.clone();

        // Cloned reader starts from beginning
        assert_eq!(cloned.bytes_read(), 0);
        assert_eq!(original.bytes_read(), 2);

        // Both share the same underlying data
        assert_eq!(cloned.as_bytes(), original.as_bytes());
    }

    #[test]
    fn test_blob_reader_from_conversions() {
        let from_vec: BlobReader = vec![1u8, 2, 3].into();
        assert_eq!(from_vec.len(), Some(3));

        let from_slice: BlobReader = b"hello".as_slice().into();
        assert_eq!(from_slice.len(), Some(5));

        let from_str: BlobReader = "world".into();
        assert_eq!(from_str.len(), Some(5));

        let from_string: BlobReader = String::from("test").into();
        assert_eq!(from_string.len(), Some(4));
    }

    #[test]
    fn test_blob_reader_debug() {
        let reader = BlobReader::from_bytes(Bytes::from_static(b"test"));
        let debug = format!("{:?}", reader);

        assert!(debug.contains("BlobReader"));
        assert!(debug.contains("total_len"));
        assert!(debug.contains("position"));
    }

    #[tokio::test]
    async fn test_blob_reader_large_data() {
        // Test with larger data to ensure no issues with buffer sizes
        let data: Vec<u8> = (0..10000).map(|i| (i % 256) as u8).collect();
        let mut reader = BlobReader::from_bytes(Bytes::from(data.clone()));

        let mut output = Vec::new();
        reader.read_to_end(&mut output).await.unwrap();

        assert_eq!(output, data);
    }
}
