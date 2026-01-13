//! Query cancellation support.
//!
//! This module provides a type-erased [`CancelHandle`] that allows cancelling
//! queries from a separate task or context while the main task is blocked
//! reading results.
//!
//! ## How Cancellation Works
//!
//! SQL Server uses out-of-band "Attention" packets to signal query cancellation.
//! The driver splits the TCP connection into read and write halves, enabling
//! the `CancelHandle` to send an Attention packet even while the main task is
//! blocked waiting for query results.
//!
//! ## Example
//!
//! ```rust,ignore
//! use mssql_client::Client;
//! use std::time::Duration;
//!
//! // Get a cancel handle before starting the query
//! let cancel_handle = client.cancel_handle();
//!
//! // Spawn a task to cancel after 5 seconds
//! tokio::spawn(async move {
//!     tokio::time::sleep(Duration::from_secs(5)).await;
//!     if let Err(e) = cancel_handle.cancel().await {
//!         eprintln!("Failed to cancel: {}", e);
//!     }
//! });
//!
//! // This query will be cancelled if it runs longer than 5 seconds
//! let result = client.query("SELECT * FROM very_large_table", &[]).await;
//! ```
//!
//! ## Important Notes
//!
//! - The `CancelHandle` is cloneable and can be shared across tasks
//! - Calling `cancel()` is idempotent; multiple calls have no additional effect
//! - After cancellation, the current query will return an error
//! - The connection remains usable for subsequent queries

use std::sync::Arc;

use mssql_codec::connection::CancelHandle as CodecCancelHandle;
#[cfg(feature = "tls")]
use mssql_tls::TlsStream;
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use crate::error::{Error, Result};

/// Type alias for the TLS cancel handle.
#[cfg(feature = "tls")]
type TlsCancelHandle = CodecCancelHandle<TlsStream<TcpStream>>;

/// Type alias for the PreLogin wrapper cancel handle.
#[cfg(feature = "tls")]
type TlsPreloginCancelHandle =
    CodecCancelHandle<TlsStream<mssql_tls::TlsPreloginWrapper<TcpStream>>>;

/// Type alias for the plain TCP cancel handle.
type PlainCancelHandle = CodecCancelHandle<TcpStream>;

/// Handle for cancelling the current query on a connection.
///
/// This handle can be cloned and sent to other tasks, enabling cancellation
/// from a separate async context while the main task is blocked reading results.
///
/// # Thread Safety
///
/// The `CancelHandle` is `Send + Sync` and can be safely shared between tasks.
#[derive(Clone)]
pub struct CancelHandle {
    inner: Arc<Mutex<CancelHandleInner>>,
}

/// Inner cancel handle that holds the actual codec handle.
enum CancelHandleInner {
    /// TLS connection (TDS 8.0 strict mode)
    #[cfg(feature = "tls")]
    Tls(TlsCancelHandle),
    /// TLS connection with PreLogin wrapping (TDS 7.x style)
    #[cfg(feature = "tls")]
    TlsPrelogin(TlsPreloginCancelHandle),
    /// Plain TCP connection
    Plain(PlainCancelHandle),
}

impl CancelHandle {
    /// Create a new cancel handle for a TLS connection (TDS 8.0 strict mode).
    #[cfg(feature = "tls")]
    pub(crate) fn from_tls(handle: TlsCancelHandle) -> Self {
        Self {
            inner: Arc::new(Mutex::new(CancelHandleInner::Tls(handle))),
        }
    }

    /// Create a new cancel handle for a TLS PreLogin connection (TDS 7.x style).
    #[cfg(feature = "tls")]
    pub(crate) fn from_tls_prelogin(handle: TlsPreloginCancelHandle) -> Self {
        Self {
            inner: Arc::new(Mutex::new(CancelHandleInner::TlsPrelogin(handle))),
        }
    }

    /// Create a new cancel handle for a plain TCP connection.
    pub(crate) fn from_plain(handle: PlainCancelHandle) -> Self {
        Self {
            inner: Arc::new(Mutex::new(CancelHandleInner::Plain(handle))),
        }
    }

    /// Send a cancellation request to the server.
    ///
    /// This sends an Attention packet to SQL Server, signaling that the
    /// current query should be cancelled. The server will acknowledge the
    /// cancellation with a DONE token containing the ATTENTION flag.
    ///
    /// # Errors
    ///
    /// Returns an error if the Attention packet cannot be sent, typically
    /// due to a network error or closed connection.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let cancel_handle = client.cancel_handle();
    ///
    /// // From another task:
    /// cancel_handle.cancel().await?;
    /// ```
    pub async fn cancel(&self) -> Result<()> {
        let inner = self.inner.lock().await;
        match &*inner {
            #[cfg(feature = "tls")]
            CancelHandleInner::Tls(h) => h.cancel().await.map_err(|e| Error::Cancel(e.to_string())),
            #[cfg(feature = "tls")]
            CancelHandleInner::TlsPrelogin(h) => {
                h.cancel().await.map_err(|e| Error::Cancel(e.to_string()))
            }
            CancelHandleInner::Plain(h) => {
                h.cancel().await.map_err(|e| Error::Cancel(e.to_string()))
            }
        }
    }

    /// Wait for the cancellation to complete.
    ///
    /// This waits until the server has acknowledged the cancellation by
    /// sending a DONE token with the ATTENTION flag set.
    ///
    /// Note: This is typically not needed as the main query will return
    /// with an error after cancellation is acknowledged.
    pub async fn wait_cancelled(&self) {
        let inner = self.inner.lock().await;
        match &*inner {
            #[cfg(feature = "tls")]
            CancelHandleInner::Tls(h) => h.wait_cancelled().await,
            #[cfg(feature = "tls")]
            CancelHandleInner::TlsPrelogin(h) => h.wait_cancelled().await,
            CancelHandleInner::Plain(h) => h.wait_cancelled().await,
        }
    }

    /// Check if a cancellation is currently in progress.
    ///
    /// Returns `true` if `cancel()` has been called but the server has not
    /// yet acknowledged the cancellation.
    #[must_use]
    pub fn is_cancelling(&self) -> bool {
        // Use try_lock to avoid blocking; if locked, someone is actively cancelling
        self.inner
            .try_lock()
            .map(|inner| match &*inner {
                #[cfg(feature = "tls")]
                CancelHandleInner::Tls(h) => h.is_cancelling(),
                #[cfg(feature = "tls")]
                CancelHandleInner::TlsPrelogin(h) => h.is_cancelling(),
                CancelHandleInner::Plain(h) => h.is_cancelling(),
            })
            .unwrap_or(true)
    }
}

impl std::fmt::Debug for CancelHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CancelHandle")
            .field("is_cancelling", &self.is_cancelling())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cancel_handle_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<CancelHandle>();
    }

    #[test]
    fn test_cancel_handle_is_clone() {
        fn assert_clone<T: Clone>() {}
        assert_clone::<CancelHandle>();
    }
}
