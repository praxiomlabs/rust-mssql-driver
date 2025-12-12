//! Connection state types for type-state pattern.
//!
//! The type-state pattern ensures at compile time that certain operations
//! can only be performed when the connection is in the appropriate state.
//!
//! ## State Transitions
//!
//! ```text
//! Disconnected -> Connected (via TCP connect)
//! Connected -> Ready (via authentication)
//! Ready -> InTransaction (via begin_transaction())
//! Ready -> Streaming (via query() that returns stream)
//! InTransaction -> Ready (via commit() or rollback())
//! InTransaction -> Streaming (via query() within transaction)
//! Streaming -> Ready (via stream completion or cancellation)
//! Streaming -> InTransaction (via stream completion within transaction)
//! ```

use std::marker::PhantomData;

/// Marker trait for connection states.
///
/// This trait is sealed to prevent external implementations,
/// ensuring that only the states defined in this crate are valid.
pub trait ConnectionState: private::Sealed {}

/// Connection is not yet established.
///
/// In this state, only `connect()` can be called.
pub struct Disconnected;

/// TCP connection established, awaiting authentication.
///
/// In this intermediate state:
/// - TCP connection is open
/// - TLS negotiation may be in progress or complete
/// - Login/authentication has not yet completed
///
/// This state is mostly internal; users typically go directly from
/// `Disconnected` to `Ready` via `Client::connect()`.
pub struct Connected;

/// Connection is established and ready for queries.
///
/// In this state, queries can be executed and transactions can be started.
pub struct Ready;

/// Connection is in a transaction.
///
/// In this state, queries execute within the transaction context.
/// The transaction must be explicitly committed or rolled back.
pub struct InTransaction;

/// Connection is actively streaming results.
///
/// In this state, the connection is processing a result set.
/// No other operations can be performed until the stream is
/// consumed or cancelled.
pub struct Streaming;

impl ConnectionState for Disconnected {}
impl ConnectionState for Connected {}
impl ConnectionState for Ready {}
impl ConnectionState for InTransaction {}
impl ConnectionState for Streaming {}

mod private {
    pub trait Sealed {}
    impl Sealed for super::Disconnected {}
    impl Sealed for super::Connected {}
    impl Sealed for super::Ready {}
    impl Sealed for super::InTransaction {}
    impl Sealed for super::Streaming {}
}

/// Type-level state transition marker.
///
/// This is used internally to track state transitions at compile time.
#[derive(Debug)]
pub struct StateMarker<S: ConnectionState> {
    _state: PhantomData<S>,
}

impl<S: ConnectionState> StateMarker<S> {
    pub(crate) fn new() -> Self {
        Self {
            _state: PhantomData,
        }
    }
}

impl<S: ConnectionState> Default for StateMarker<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: ConnectionState> Clone for StateMarker<S> {
    fn clone(&self) -> Self {
        Self::new()
    }
}

impl<S: ConnectionState> Copy for StateMarker<S> {}

/// Internal protocol state for runtime management.
///
/// While connection states are tracked at compile-time via type-state,
/// the protocol layer has runtime state that must be managed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolState {
    /// Awaiting response from server.
    AwaitingResponse,
    /// Processing token stream.
    ProcessingTokens,
    /// Draining remaining tokens after cancellation.
    Draining,
    /// Connection is in a broken state due to protocol error.
    Poisoned,
}

impl Default for ProtocolState {
    fn default() -> Self {
        Self::AwaitingResponse
    }
}

impl ProtocolState {
    /// Check if the connection is in a usable state.
    #[must_use]
    pub fn is_usable(&self) -> bool {
        !matches!(self, Self::Poisoned)
    }

    /// Check if the connection is actively processing.
    #[must_use]
    pub fn is_busy(&self) -> bool {
        matches!(self, Self::ProcessingTokens | Self::Draining)
    }
}
