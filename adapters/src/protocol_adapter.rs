//! Protocol Adapter Trait
//!
//! This module defines the ProtocolAdapter trait that all protocol
//! implementations must implement to translate their specific dialect into
//! the canonical Protocol IR.

use std::path::Path;

use ir::ProtocolIR;
use thiserror::Error;

/// Capability constants for protocol adapters
pub const CAP_RPC: &str = "rpc";
/// P2P network protocol capability
pub const CAP_P2P: &str = "p2p";
/// PSBT capability
pub const CAP_PSBT: &str = "psbt";

#[derive(Debug, Error)]
/// Errors that can occur during protocol extraction
pub enum ProtocolAdapterError {
    /// I/O error while reading files
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// JSON serialization/deserialization error
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// Generic message-based error
    #[error("{0}")]
    Message(String),
    /// Requested implementation/backend is not available in this build
    #[error("Unsupported implementation: {0}")]
    UnsupportedImplementation(String),
}

/// Result alias for protocol adapter operations
pub type ProtocolAdapterResult<T> = std::result::Result<T, ProtocolAdapterError>;

/// Protocol Adapter trait for translating protocol implementations to Protocol IR
///
/// This trait represents the interface that all protocol implementations
/// (Bitcoin Core, BDK, LND, etc.) must implement to participate in the Ethos
/// compilation process. Each adapter translates its specific dialect into the
/// canonical Protocol IR.
pub trait ProtocolAdapter {
    /// Get the name of this protocol implementation
    ///
    /// Examples: "bitcoin-core", "bdk", "lnd", "electrum"
    fn name(&self) -> &'static str;

    /// Get the version of this protocol implementation
    ///
    /// Examples: "latest", "0.32.0", "v0.18.0"
    fn version(&self) -> String;

    /// Extract the Bitcoin protocol IR from this implementation
    ///
    /// This method should analyze the implementation's schema/specification
    /// and extract the relevant parts of the Bitcoin protocol that it implements.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the implementation's schema or specification files
    ///
    /// # Returns
    ///
    /// Returns a `ProtocolAdapterResult<ProtocolIR>` containing the extracted
    /// protocol IR, or an error if extraction fails.
    fn extract_protocol_ir(&self, path: &Path) -> ProtocolAdapterResult<ProtocolIR>;

    /// Get the capabilities of this adapter
    ///
    /// Returns a list of protocol modules that this adapter can extract
    /// (e.g., ["rpc", "p2p", "psbt"])
    fn capabilities(&self) -> Vec<&'static str>;

    /// Check if this adapter supports a specific protocol module
    ///
    /// # Arguments
    ///
    /// * `module` - The protocol module name to check
    ///
    /// # Returns
    ///
    /// Returns `true` if this adapter can extract the specified module
    fn supports_module(&self, module: &str) -> bool { self.capabilities().contains(&module) }
}

/// Protocol-agnostic trait for loading IR from different Bitcoin protocol implementations
pub trait IrLoader {
    /// Load Protocol IR from the given path
    fn load_ir(&self, path: &Path) -> ProtocolAdapterResult<ProtocolIR>;
}

/// Blanket implementation: any type implementing ProtocolAdapter automatically implements IrLoader
impl<T: ProtocolAdapter> IrLoader for T {
    fn load_ir(&self, path: &Path) -> ProtocolAdapterResult<ProtocolIR> {
        self.extract_protocol_ir(path)
    }
}

// Re-export types from shared fuzz types crate
pub use fuzz_types::{FuzzCase, FuzzResult};
