//! SQL Server FILESTREAM BLOB access.
//!
//! This module provides async read/write access to SQL Server FILESTREAM BLOBs
//! via the `OpenSqlFilestream` Win32 API. FILESTREAM allows SQL Server to store
//! `VARBINARY(MAX)` data directly on the NTFS filesystem while maintaining
//! transactional consistency with the database.
//!
//! ## Requirements
//!
//! - **Windows only** — FILESTREAM uses Win32 file handles via the OLE DB Driver
//! - **SQL Server with FILESTREAM enabled** — `sp_configure 'filestream access level', 2`
//! - **OLE DB Driver for SQL Server** — `msoledbsql19.dll` or `msoledbsql.dll`
//!   (alternatively, the deprecated SQL Server Native Client `sqlncli11.dll`)
//! - **Active transaction** — FILESTREAM handles are bound to a SQL transaction
//!
//! ## Usage
//!
//! ```rust,ignore
//! use mssql_client::{Client, Config, FileStream, FileStreamAccess};
//!
//! // Connect and begin a transaction (FILESTREAM requires a transaction)
//! let mut client = Client::connect(config).await?;
//! let mut tx = client.begin_transaction().await?;
//!
//! // Step 1: Get the FILESTREAM path for the BLOB column
//! let rows = tx.query(
//!     "SELECT Content.PathName() FROM dbo.Documents WHERE Id = @p1",
//!     &[&doc_id],
//! ).await?;
//! let path: String = rows.into_iter().next().unwrap()?.get(0)?;
//!
//! // Step 2: Open the FILESTREAM BLOB
//! let mut stream = tx.open_filestream(&path, FileStreamAccess::Read).await?;
//!
//! // Step 3: Read data using tokio async I/O
//! use tokio::io::AsyncReadExt;
//! let mut buf = Vec::new();
//! stream.read_to_end(&mut buf).await?;
//!
//! // Step 4: Drop the stream before committing
//! drop(stream);
//! tx.commit().await?;
//! ```
//!
//! ## Architecture
//!
//! The FILESTREAM access pattern is a three-step dance:
//!
//! 1. **Get the file path** — T-SQL `column.PathName()` returns a UNC path
//! 2. **Get the transaction token** — T-SQL `GET_FILESTREAM_TRANSACTION_CONTEXT()`
//!    returns a `varbinary` token binding file access to the current transaction
//! 3. **Open the file** — `OpenSqlFilestream()` from the OLE DB driver DLL returns
//!    a Win32 `HANDLE` compatible with `ReadFile`/`WriteFile`
//!
//! Steps 1 and 2 are regular SQL queries. Step 3 is the FFI call this module provides.
//! The returned handle is wrapped in a [`tokio::fs::File`] for async I/O.
//!
//! ## Async I/O Strategy
//!
//! The Win32 `HANDLE` from `OpenSqlFilestream` is wrapped in [`tokio::fs::File`], which
//! dispatches read/write operations to tokio's blocking thread pool via `spawn_blocking`.
//! This is the standard tokio approach for file handles and works correctly for typical
//! FILESTREAM workloads.
//!
//! A future optimization could register the handle directly with tokio's IOCP reactor
//! for true completion-based async I/O (via overlapped `ReadFile`/`WriteFile`). This would
//! eliminate the per-operation thread dispatch overhead and improve throughput under high
//! concurrency. The [`open_options::ASYNC`] flag is provided to enable overlapped I/O at
//! the Win32 level for advanced users who implement their own IOCP wrapper around the
//! handle obtained from [`FileStream::into_tokio_file`].

use std::ffi::c_void;
use std::io;
use std::pin::Pin;
use std::sync::OnceLock;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::error::Error;

// =============================================================================
// Public types
// =============================================================================

/// Access mode for FILESTREAM BLOB data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FileStreamAccess {
    /// Read-only access to the BLOB.
    Read,
    /// Write-only access to the BLOB.
    Write,
    /// Read and write access to the BLOB.
    ReadWrite,
}

impl FileStreamAccess {
    /// Convert to the `SQL_FILESTREAM_DESIRED_ACCESS` constant.
    fn to_raw(self) -> u32 {
        match self {
            Self::Read => SQL_FILESTREAM_READ,
            Self::Write => SQL_FILESTREAM_WRITE,
            Self::ReadWrite => SQL_FILESTREAM_READWRITE,
        }
    }
}

/// Options for opening a FILESTREAM handle.
///
/// These flags are passed to the `OpenOptions` parameter of `OpenSqlFilestream`.
/// They can be combined with bitwise OR.
pub mod open_options {
    /// No special open options. I/O is performed via tokio's blocking thread pool.
    pub const NONE: u32 = 0x0000_0000;

    /// Request sequential scan optimization.
    ///
    /// Hints that the file will be read sequentially from beginning to end,
    /// allowing the OS to optimize read-ahead caching.
    pub const SEQUENTIAL_SCAN: u32 = 0x0000_0008;

    /// Enable overlapped (async) I/O at the Win32 level.
    ///
    /// This flag tells the Win32 subsystem to open the handle for overlapped I/O,
    /// which is required for true IOCP-based async. Note that `FileStream` currently
    /// wraps the handle in `tokio::fs::File` which uses `spawn_blocking` regardless
    /// of this flag. This option is provided for advanced users who extract the
    /// handle via [`FileStream::into_tokio_file`] and implement their own
    /// IOCP integration.
    pub const ASYNC: u32 = 0x0000_0001;
}

/// An open FILESTREAM BLOB handle with async read/write support.
///
/// This type wraps a Win32 file handle obtained from `OpenSqlFilestream`
/// in a [`tokio::fs::File`] for async I/O. It implements [`AsyncRead`] and
/// [`AsyncWrite`], making it compatible with the tokio ecosystem.
///
/// # Lifecycle
///
/// A `FileStream` is bound to the SQL Server transaction that was active when
/// it was opened. **The `FileStream` must be dropped before the transaction is
/// committed or rolled back.** Failure to do so will cause the commit/rollback
/// to fail.
///
/// # Example
///
/// ```rust,ignore
/// use tokio::io::AsyncReadExt;
/// use mssql_client::FileStreamAccess;
///
/// // Within an active transaction:
/// let mut stream = tx.open_filestream(&path, FileStreamAccess::Read).await?;
/// let mut data = Vec::new();
/// stream.read_to_end(&mut data).await?;
/// drop(stream); // Must drop before commit
/// tx.commit().await?;
/// ```
pub struct FileStream {
    inner: tokio::fs::File,
}

impl FileStream {
    /// Open a FILESTREAM BLOB using a path and transaction context.
    ///
    /// This is the low-level API. Most users should use
    /// [`Client<InTransaction>::open_filestream`] instead, which automatically
    /// obtains the transaction context.
    ///
    /// # Arguments
    ///
    /// * `path` — UNC path from the T-SQL `column.PathName()` function
    /// * `access` — Read, write, or read/write access mode
    /// * `txn_context` — Transaction context bytes from `GET_FILESTREAM_TRANSACTION_CONTEXT()`
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No FILESTREAM driver DLL is installed (`msoledbsql.dll` / `sqlncli11.dll`)
    /// - The `OpenSqlFilestream` call fails (invalid path, permissions, etc.)
    pub fn open(path: &str, access: FileStreamAccess, txn_context: &[u8]) -> Result<Self, Error> {
        Self::open_with_options(path, access, txn_context, open_options::NONE)
    }

    /// Open a FILESTREAM BLOB with custom open options.
    ///
    /// Like [`open`](Self::open), but allows specifying Win32 open flags via the
    /// `options` parameter. See [`open_options`] for available flags.
    ///
    /// # Arguments
    ///
    /// * `path` — UNC path from the T-SQL `column.PathName()` function
    /// * `access` — Read, write, or read/write access mode
    /// * `txn_context` — Transaction context bytes from `GET_FILESTREAM_TRANSACTION_CONTEXT()`
    /// * `options` — Bitwise OR of [`open_options`] flags
    pub fn open_with_options(
        path: &str,
        access: FileStreamAccess,
        txn_context: &[u8],
        options: u32,
    ) -> Result<Self, Error> {
        // Load the function pointer (cached after first call)
        let open_fn = load_open_sql_filestream()?;

        // Encode path as null-terminated UTF-16
        let path_wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();

        // SAFETY: open_fn is a valid function pointer obtained from GetProcAddress.
        // path_wide is a valid null-terminated UTF-16 string that outlives this call.
        // txn_context is a valid byte slice with correct length.
        // The returned handle is checked against INVALID_HANDLE_VALUE before use.
        let handle = unsafe {
            open_fn(
                path_wide.as_ptr(),
                access.to_raw(),
                options,
                txn_context.as_ptr(),
                txn_context.len(),
                std::ptr::null(), // no allocation size hint
            )
        };

        if handle == INVALID_HANDLE_VALUE || handle.is_null() {
            let err_code = unsafe { GetLastError() };
            let message = format_win32_error(err_code);
            return Err(Error::FileStream(format!(
                "OpenSqlFilestream failed: {message} (Win32 error {err_code})"
            )));
        }

        // Convert the raw handle to a tokio::fs::File for async I/O.
        //
        // SAFETY: The handle was just obtained from a successful OpenSqlFilestream call.
        // OwnedHandle takes ownership and will close the handle on drop.
        // tokio::fs::File::from_std wraps it for async I/O via spawn_blocking.
        let file = unsafe {
            use std::os::windows::io::{FromRawHandle, OwnedHandle, RawHandle};
            let owned = OwnedHandle::from_raw_handle(handle as RawHandle);
            let std_file = std::fs::File::from(owned);
            tokio::fs::File::from_std(std_file)
        };

        Ok(Self { inner: file })
    }

    /// Get a reference to the underlying tokio file.
    #[must_use]
    pub fn as_tokio_file(&self) -> &tokio::fs::File {
        &self.inner
    }

    /// Get a mutable reference to the underlying tokio file.
    pub fn as_tokio_file_mut(&mut self) -> &mut tokio::fs::File {
        &mut self.inner
    }

    /// Consume the `FileStream` and return the underlying tokio file.
    #[must_use]
    pub fn into_tokio_file(self) -> tokio::fs::File {
        self.inner
    }
}

impl AsyncRead for FileStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for FileStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

impl std::fmt::Debug for FileStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileStream")
            .field("inner", &self.inner)
            .finish()
    }
}

// =============================================================================
// FFI: OpenSqlFilestream constants
// =============================================================================

/// Read-only access.
const SQL_FILESTREAM_READ: u32 = 0;
/// Write-only access.
const SQL_FILESTREAM_WRITE: u32 = 1;
/// Read and write access.
const SQL_FILESTREAM_READWRITE: u32 = 2;

/// Invalid handle sentinel value (`(HANDLE)-1`).
const INVALID_HANDLE_VALUE: *mut c_void = -1_isize as *mut c_void;

// =============================================================================
// FFI: Runtime dynamic loading
// =============================================================================

/// Function pointer type for `OpenSqlFilestream`.
///
/// See: <https://learn.microsoft.com/en-us/sql/relational-databases/blob/access-filestream-data-with-opensqlfilestream>
type OpenSqlFilestreamFn = unsafe extern "system" fn(
    filestream_path: *const u16,       // LPCWSTR
    desired_access: u32,               // SQL_FILESTREAM_DESIRED_ACCESS
    open_options: u32,                 // ULONG
    filestream_txn_context: *const u8, // LPBYTE
    filestream_txn_context_len: usize, // SIZE_T (note: SSIZE_T in some docs, but usize on 64-bit)
    allocation_size: *const i64,       // PLARGE_INTEGER (nullable)
) -> *mut c_void; // HANDLE

// Win32 kernel32 imports for runtime DLL loading and error formatting.
unsafe extern "system" {
    fn LoadLibraryW(name: *const u16) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    fn GetLastError() -> u32;
    fn FormatMessageW(
        flags: u32,
        source: *const c_void,
        message_id: u32,
        language_id: u32,
        buffer: *mut u16,
        size: u32,
        arguments: *const c_void,
    ) -> u32;
}

/// Format a Win32 error code into a human-readable message.
fn format_win32_error(error_code: u32) -> String {
    const FORMAT_MESSAGE_FROM_SYSTEM: u32 = 0x0000_1000;
    const FORMAT_MESSAGE_IGNORE_INSERTS: u32 = 0x0000_0200;

    let mut buf = [0u16; 512];

    // SAFETY: FormatMessageW with FROM_SYSTEM | IGNORE_INSERTS is safe to call
    // with a stack-allocated buffer. It writes at most `buf.len()` wide chars.
    let len = unsafe {
        FormatMessageW(
            FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
            std::ptr::null(),
            error_code,
            0, // default language
            buf.as_mut_ptr(),
            buf.len() as u32,
            std::ptr::null(),
        )
    };

    if len == 0 {
        return format!("Unknown error (0x{error_code:08X})");
    }

    // Trim trailing \r\n
    let s = String::from_utf16_lossy(&buf[..len as usize]);
    s.trim_end().to_string()
}

/// Cached function pointer — resolved once on first use.
static OPEN_SQL_FILESTREAM: OnceLock<Result<OpenSqlFilestreamFn, String>> = OnceLock::new();

/// DLLs to search for `OpenSqlFilestream`, in priority order.
///
/// - `msoledbsql19.dll` — OLE DB Driver 19 for SQL Server (newest)
/// - `msoledbsql.dll` — OLE DB Driver 18 for SQL Server
/// - `sqlncli11.dll` — SQL Server Native Client 11 (deprecated but still common)
const DLL_SEARCH_ORDER: &[&str] = &["msoledbsql19.dll", "msoledbsql.dll", "sqlncli11.dll"];

/// Load the `OpenSqlFilestream` function pointer from the first available DLL.
///
/// The result is cached in a `OnceLock` so the DLL search happens at most once.
fn load_open_sql_filestream() -> Result<OpenSqlFilestreamFn, Error> {
    OPEN_SQL_FILESTREAM
        .get_or_init(|| {
            for dll_name in DLL_SEARCH_ORDER {
                // Encode DLL name as null-terminated UTF-16
                let dll_wide: Vec<u16> =
                    dll_name.encode_utf16().chain(std::iter::once(0)).collect();

                // SAFETY: LoadLibraryW with a valid null-terminated UTF-16 string.
                // Returns null on failure (DLL not found), non-null on success.
                let module = unsafe { LoadLibraryW(dll_wide.as_ptr()) };
                if module.is_null() {
                    continue;
                }

                // SAFETY: module is a valid HMODULE from LoadLibraryW.
                // The function name is a valid null-terminated ASCII string.
                let proc = unsafe { GetProcAddress(module, c"OpenSqlFilestream".as_ptr().cast()) };
                if proc.is_null() {
                    continue;
                }

                tracing::debug!(dll = dll_name, "Loaded OpenSqlFilestream");

                // SAFETY: proc is a valid function pointer obtained from GetProcAddress
                // for OpenSqlFilestream, which has the signature matching OpenSqlFilestreamFn.
                // The DLL remains loaded for the lifetime of the process (we intentionally
                // don't call FreeLibrary to keep the function pointer valid).
                let func: OpenSqlFilestreamFn = unsafe { std::mem::transmute(proc) };
                return Ok(func);
            }

            Err(format!(
                "FILESTREAM driver not found. Install the Microsoft OLE DB Driver for SQL Server. \
                 Searched: {}",
                DLL_SEARCH_ORDER.join(", ")
            ))
        })
        .clone()
        .map_err(Error::FileStream)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filestream_access_raw_values() {
        assert_eq!(FileStreamAccess::Read.to_raw(), 0);
        assert_eq!(FileStreamAccess::Write.to_raw(), 1);
        assert_eq!(FileStreamAccess::ReadWrite.to_raw(), 2);
    }

    #[test]
    fn test_dll_search_order() {
        // Verify the DLL search order is correct (newest first)
        assert_eq!(DLL_SEARCH_ORDER[0], "msoledbsql19.dll");
        assert_eq!(DLL_SEARCH_ORDER[1], "msoledbsql.dll");
        assert_eq!(DLL_SEARCH_ORDER[2], "sqlncli11.dll");
    }

    #[test]
    fn test_load_open_sql_filestream() {
        // This test verifies that the DLL loading works on machines with the driver.
        // On machines without any FILESTREAM driver, this will return an error.
        let result = load_open_sql_filestream();
        match result {
            Ok(_) => {
                // Driver found — verify the function pointer is cached
                let result2 = load_open_sql_filestream();
                assert!(result2.is_ok(), "Second call should also succeed (cached)");
            }
            Err(e) => {
                let msg = format!("{e}");
                assert!(
                    msg.contains("FILESTREAM driver not found"),
                    "Error should indicate missing driver: {msg}"
                );
            }
        }
    }
}
