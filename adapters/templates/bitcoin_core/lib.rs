#![allow(missing_docs)]

//! Generated Bitcoin Core RPC client library.
//!
//! This library provides a strongly-typed interface to the Bitcoin Core RPC API.
//! It is generated from the Bitcoin Core RPC API documentation.

// Core modules
pub mod config;
pub mod client_trait;
pub mod node;
pub mod test_config;
pub mod bitcoin_core_clients;
pub mod transport;
pub mod responses;
pub mod types;

// Re-exports for ergonomic access
pub use config::Config;
pub use client_trait::client::BitcoinClient;
pub use node::BitcoinNodeManager;
pub use bitcoin::Network;
pub use test_config::TestConfig;
pub use bitcoin_core_clients::client::BitcoinTestClient;
pub use responses::*;
pub use transport::{
    DefaultTransport,
    TransportError,
    RpcClient,
};
