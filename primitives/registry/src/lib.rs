#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]

//! Protocol Registry — a lightweight database for protocol method definitions.
//!
//! This crate provides in-memory registries that store and query protocol
//! method specifications during compilation. It's designed for build-time usage
//! in code generation and analysis.

pub mod ir_resolver;
pub mod type_alias_registry;

use std::collections::BTreeMap;

use ir::RpcDef;

/// A registry of protocol method definitions.
///
/// Stores and provides access to protocol method specifications during compilation.
#[derive(Default)]
pub struct ProtocolRegistry {
    /// Map from method name to RPC definition
    methods: BTreeMap<String, RpcDef>,
}

impl ProtocolRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self { Self::default() }

    /// Add a method to the registry.
    pub fn insert(&mut self, method: RpcDef) { self.methods.insert(method.name.clone(), method); }
}

/// Read-only interface to the `ProtocolRegistry`.
///
/// Provides a clean API for querying protocol method definitions without
/// exposing mutation capabilities.
pub trait ProtocolRegistryReader {
    /// Get all method names in the registry.
    fn list_methods(&self) -> Vec<&str>;

    /// Get a method definition by name.
    ///
    /// Returns `None` if no method with the given name exists.
    fn get_method(&self, name: &str) -> Option<&RpcDef>;

    /// Get the total number of methods in the registry.
    fn method_count(&self) -> usize;
}

/// Implement the interface for `ProtocolRegistry`.
impl ProtocolRegistryReader for ProtocolRegistry {
    fn list_methods(&self) -> Vec<&str> { self.methods.keys().map(|s| s.as_str()).collect() }

    fn get_method(&self, name: &str) -> Option<&RpcDef> { self.methods.get(name) }

    fn method_count(&self) -> usize { self.methods.len() }
}

// Re-export the type alias registry for type canonicalization
pub use type_alias_registry::{TypeAliasRegistry, TypeAliasRegistryReader};
