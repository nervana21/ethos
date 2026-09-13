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

/// Bitcoin Core type definitions and utilities
pub mod bitcoin_core {
    /// Bitcoin Core OpenRPC converter and version filtering (openrpc.json / getopenrpcinfo -> IR)
    pub mod openrpc;
    mod openrpc_type_disambiguation;
    /// Bitcoin Core type definitions and utilities
    pub mod types;
}

pub mod adapter_facade;
pub mod conversion_helpers;
pub mod normalization_registry;
pub mod protocol_adapter;
pub mod rpc_adapter;

// Re-export the main ProtocolAdapter types for convenience
pub use adapter_facade::*;
pub use bitcoin_core::types::{
    adapter_fallback_events_snapshot, clear_adapter_fallback_events, clear_fallback_events,
    fallback_events_snapshot, record_fallback_event, BitcoinCoreRpcType, BitcoinCoreTypeRegistry,
    FidelityFallbackEvent, GetBlockTemplateRequest, SendallRecipient,
};
pub use fuzz_types::{FuzzCase, FuzzResult, ProtocolAdapter as FuzzProtocolAdapter};
pub use protocol_adapter::*;
pub use rpc_adapter::RpcAdapter;

/// Type alias for Bitcoin Core RPC adapter
pub type BitcoinCoreRpcAdapter = RpcAdapter;
