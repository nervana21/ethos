#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]

//! Protocol Adapter Library
//!
//! This module provides a unified set of interface adapters to translate protocol dialects
//! into a shared intermediate representation (IR) understood by higher-level components.
//! Each adapter implements the canonical RPC interface for its respective protocol dialect.
//! The design supports extensibility: add a new protocol by implementing an adapter and
//! registering it in the registry.

use std::path::Path;

use ir::ProtocolIR;

/// Bitcoin Core type definitions and utilities
pub mod bitcoin_core {
    /// Bitcoin Core schema converter (raw schema.json -> ProtocolIR)
    pub mod schema;
    /// Bitcoin Core type definitions and utilities
    pub mod types;
}

pub mod adapter_facade;
pub mod normalization_registry;
pub mod protocol_adapter;
pub mod rpc_adapter;

// Re-export the main ProtocolAdapter types for convenience
pub use adapter_facade::*;
pub use bitcoin_core::types::{BitcoinCoreRpcType, BitcoinCoreTypeRegistry};
pub use fuzz_types::{FuzzCase, FuzzResult, ProtocolAdapter as FuzzProtocolAdapter};
pub use protocol_adapter::*;
pub use rpc_adapter::RpcAdapter;

/// Type alias for Bitcoin Core RPC adapter
pub type BitcoinCoreRpcAdapter = RpcAdapter;

/// Protocol-agnostic trait for loading IR from different Bitcoin protocol implementations
pub trait IrLoader {
    /// Load Protocol IR from the given path
    fn load_ir(&self, path: &Path) -> ProtocolAdapterResult<ProtocolIR>;
}

impl<T: ProtocolAdapter> IrLoader for T {
    fn load_ir(&self, path: &Path) -> ProtocolAdapterResult<ProtocolIR> {
        self.extract_protocol_ir(path)
    }
}
