//! Protocol-specific type adapters
//!
//! This module contains implementations of the `TypeAdapter` trait for
//! different protocols. Each adapter handles the protocol-specific logic
//! for parsing response schemas and mapping types to Rust equivalents.

pub mod bitcoin_core;
pub mod bitcoin_core_utils;
pub mod core_lightning;

pub use bitcoin_core::BitcoinCoreAdapter;
pub use core_lightning::CoreLightningAdapter;
