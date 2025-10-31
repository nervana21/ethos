#![allow(missing_docs)]

//! Generated core_lightning RPC client library.
//!
//! This library provides a strongly-typed interface to the core_lightning RPC API.
//! It is generated from the core_lightning RPC API documentation.

// Core modules
/// Configuration management for the Core Lightning client
pub mod config;
/// Client trait definitions and implementations
pub mod client_trait;
/// Node management utilities for testing
pub mod node;
/// Test configuration utilities
pub mod test_config;
/// Transport layer for RPC communication
pub mod transport;
/// Response type definitions
pub mod responses;
/// Common type definitions
pub mod types;

// Re-exports for ergonomic access
pub use config::Config;
pub use client_trait::client::{{CLIENT_NAME}};
pub use node::CoreLightningNodeManager;
pub use bitcoin::Network;
pub use test_config::TestConfig;
pub use responses::*;
pub use types::{PublicKey, ShortChannelId};
pub use transport::{
    DefaultTransport,
    TransportError,
    RpcClient,
};
