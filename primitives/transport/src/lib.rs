#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]

//! # `ethos-transport` — Foundational Communication Layer
//!
//! This crate defines the **core transport abstraction** used throughout
//! Ethos.
//!
//! It provides the fundamental interface (`Transport` trait) for all forms
//! of network or interprocess communication — including HTTP, IPC, P2P, and
//! other future backends.  Every message exchanged between formal protocol
//! specifications and live systems must traverse a transport that implements
//! this trait.
//!
//! ## Core Concepts
//!
//! ### `Transport` Trait
//! Defines how messages are sent (`send`) and batched (`send_batch`), returning
//! deserialized [`serde_json::Value`]s rather than typed RPC responses.
//! Backends such as `ethos-http`, `ethos-ipc`, or `ethos-p2p`
//! implement this trait to perform their actual I/O work.
//!
//! ### `TransportError`
//! Enumerates all possible classes of errors encountered during communication,
//! ensuring consistent reporting and recoverability across layers.
//!
//! ### `DynTransport`
//! A type-erased (`Arc<dyn Transport>`) wrapper for ergonomic sharing and
//! polymorphism — allowing clients, tests, and compilers to operate over
//! any transport backend without knowing which one is in use.
//!
//! ### `JsonRpcResponse`
//! A minimal struct for representing JSON-RPC responses in a backend-agnostic
//! way. Used mainly in tests or low-level utilities that need to deserialize
//! raw responses before higher-level decoding occurs.
//!
//! ## Feature Flags
//! - `reqwest`: Enables `From<reqwest::Error>` for `TransportError` without
//!   imposing a hard dependency on `reqwest` for all consumers.
//!
//! ### `BatchTransport`
//! Some backends support JSON-RPC batching, allowing multiple method calls
//! to be transmitted as one envelope.
//!
//! ## Example
//! ```no_run
//! use transport::{Transport, DynTransport, TransportError};
//! use serde_json::json;
//!
//! async fn demo(transport: DynTransport) -> Result<(), TransportError> {
//!     let info = transport.send("getblockchaininfo", &[]).await?;
//!     println!("chain = {}", info["chain"]);
//!     Ok(())
//! }
//! ```

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Type alias for structured error handling in transport operations.
pub type Result<T> = std::result::Result<T, TransportError>;

/// Canonical error type for all transport implementations.
///
/// Each variant corresponds to a distinct communication or parsing
/// failure mode.  This enum intentionally avoids leaking backend-
/// specific details, so that higher layers can reason uniformly about
/// network, serialization, and RPC failures.
#[derive(thiserror::Error, Debug)]
pub enum TransportError {
    /// An HTTP-level failure (connection refused, timeout, or bad status code).
    #[error("HTTP transport error: {0}")]
    Http(String),

    /// An IPC (inter-process communication) failure, typically during socket I/O.
    #[error("IPC transport error: {0}")]
    Ipc(String),

    /// Failure to serialize or deserialize a JSON payload.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// The remote endpoint returned an explicit JSON-RPC error object.
    #[error("RPC error: {0}")]
    Rpc(String),

    /// The JSON-RPC response was missing the expected `result` field.
    #[error("Missing result field")]
    MissingResult,

    /// The response did not conform to the expected JSON-RPC envelope format.
    #[error("Invalid response format: {0}")]
    InvalidFormat(String),

    /// Any other error not covered by the specific variants above.
    #[error("Other error: {0}")]
    Other(String),
}

impl From<serde_json::Error> for TransportError {
    fn from(err: serde_json::Error) -> Self { TransportError::Serialization(err.to_string()) }
}

/// The base transport trait for single-message delivery.
///
/// For protocols that support batching, use [`BatchTransport`].
#[async_trait]
pub trait Transport: Send + Sync {
    /// Sends a single message or RPC call.
    ///
    /// The request is identified by its `method` name and an ordered list
    /// of parameters serialized as [`serde_json::Value`]s. Implementations
    /// should return the value of the `"result"` field from the corresponding
    /// JSON-RPC response, or an appropriate [`TransportError`].
    async fn send(&self, method: &str, params: &[Value]) -> Result<Value>;

    /// Returns the configured endpoint or connection descriptor.
    ///
    /// For network transports, this is usually the URL or socket path.
    /// For mock or in-memory transports, it may be a symbolic name.
    fn endpoint(&self) -> &str;
}

/// Extension trait for transports that support batching.
///
/// Some backends support JSON-RPC batching, allowing multiple method calls
/// to be transmitted as one envelope.
#[async_trait]
pub trait BatchTransport: Transport {
    /// Sends a batch of requests in a single message.
    ///
    /// This method allows multiple JSON-RPC frames to be transmitted together.
    /// The return value is expected to be a vector of raw JSON `result` values,
    /// preserving the order of requests.
    async fn send_batch(&self, batch: &[Value]) -> Result<Vec<Value>>;
}

/// Type alias for a shared, dynamically dispatched transport instance.
///
/// This enables pluggable backends at runtime without generic parameters:
/// ```
/// use transport::{DynTransport, Transport};
/// use std::sync::Arc;
///
/// fn use_transport(t: DynTransport) {
///     println!("Using endpoint: {}", t.endpoint());
/// }
/// ```
pub type DynTransport = Arc<dyn Transport>;

/// Minimal structure representing a JSON-RPC response envelope.
///
/// This struct exists primarily to facilitate testing, schema validation,
/// and debugging of raw responses prior to conversion into typed results.
///
/// Fields correspond directly to those defined in the JSON-RPC 2.0 specification.
#[derive(Debug)]
pub struct JsonRpcResponse {
    /// The value returned by the RPC call, if successful.
    pub result: Option<Value>,
    /// The error object returned by the server, if any.
    pub error: Option<Value>,
    /// The unique identifier correlating request and response.
    pub id: Value,
}

/// Transport configuration for communication backends.
///
/// This struct defines how to connect to a protocol endpoint, including
/// the transport mechanism, connection details, and authentication settings.
///
/// # Examples
///
/// HTTP transport with basic authentication:
/// ```
/// use transport::TransportConfig;
///
/// let config = TransportConfig {
///     transport_type: "http".to_string(),
///     endpoint: "http://127.0.0.1:18443".to_string(),
///     auth: Some(transport::AuthConfig {
///         auth_type: "basic".to_string(),
///         username: Some("rpcuser".to_string()),
///         password: Some("rpcpassword".to_string()),
///         cert_path: None,
///         token: None,
///     }),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    /// Transport mechanism (e.g., "http", "ipc", "p2p", "websocket")
    pub transport_type: String,
    /// Connection endpoint (URL, socket path, etc.)
    pub endpoint: String,
    /// Authentication settings (optional)
    pub auth: Option<AuthConfig>,
}

/// Authentication configuration for transport connections.
///
/// Supports multiple authentication methods including basic auth,
/// certificate-based auth, token auth, or no authentication.
///
/// # Examples
///
/// Basic authentication:
/// ```
/// use transport::AuthConfig;
///
/// let auth = AuthConfig {
///     auth_type: "basic".to_string(),
///     username: Some("rpcuser".to_string()),
///     password: Some("rpcpassword".to_string()),
///     cert_path: None,
///     token: None,
/// };
/// ```
///
/// Certificate-based authentication:
/// ```
/// use transport::AuthConfig;
/// use std::path::PathBuf;
///
/// let auth = AuthConfig {
///     auth_type: "certificate".to_string(),
///     username: None,
///     password: None,
///     cert_path: Some(PathBuf::from("/path/to/tls.cert")),
///     token: None,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Auth type (e.g., "basic", "certificate", "token", "none")
    pub auth_type: String,
    /// Username (for basic auth)
    pub username: Option<String>,
    /// Password (for basic auth)
    pub password: Option<String>,
    /// Certificate path (for cert auth)
    pub cert_path: Option<PathBuf>,
    /// Token (for token auth)
    pub token: Option<String>,
}

/// Gets a random free port assigned by the OS.
///
/// This function binds to `127.0.0.1:0`, which causes the OS to assign
/// an available port. The listener is then dropped and the port number
/// is returned.
///
/// # Errors
///
/// Returns an error if binding to the address fails.
///
/// # Examples
///
/// ```
/// use transport::get_random_free_port;
///
/// let port = get_random_free_port()?;
/// println!("Using port: {}", port);
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn get_random_free_port() -> std::io::Result<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from() {
        let err = serde_json::from_str::<serde_json::Value>("not-json")
            .expect_err("Expected JSON parsing to fail");
        let terr: TransportError = err.into();

        match terr {
            TransportError::Serialization(msg) => assert!(!msg.is_empty()),
            _ => panic!("expected Serialization error variant"),
        }
    }

    #[test]
    fn test_get_random_free_port() {
        let port = get_random_free_port().expect("Should get a free port");

        assert!(port > 0);
    }
}
