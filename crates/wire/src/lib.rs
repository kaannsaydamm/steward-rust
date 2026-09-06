//! Steward Wire v2: the transport-independent app-server protocol (Omega
//! Phase 2 / §34). Envelopes are serde types; gRPC and WebSocket transports
//! convert to/from them without owning protocol semantics.

pub mod envelope;
pub mod version;

pub use envelope::{AppCommand, AppError, ClientEnvelope, ServerEnvelope, StateSnapshot};
pub use version::{ProtocolVersion, HANDSHAKE};
