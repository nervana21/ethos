#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]

//! Core Type System for Protocol Compiler
//!
//! This crate defines the fundamental type system and data structures used
//! throughout the Protocol Compiler. It provides comprehensive type definitions
//! for protocol methods, their metadata, safety classifications, and type mapping utilities.
//! The system is designed to be protocol-agnostic, supporting multiple communication
//! patterns (RPC, P2P, Lightning, Stratum, etc.) through the adapter pattern.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Type-safe implementation names for Bitcoin protocol implementations.
pub mod implementation;
/// Node metadata for implementation-specific node management
pub mod node_metadata;
/// Protocol version representation and parsing
pub mod version;
/// Re-export the `Implementation` enum for convenience.
pub use implementation::{Implementation, Protocol};
/// Re-export the `ProtocolVersion` type for convenience.
pub use version::{ProtocolVersion, VersionError};

/// Protocol-specific type adapters
pub mod adapters;
/// Error types for protocol operations
pub mod errors;
/// Type adapter trait for protocol-specific response type generation
pub mod type_adapter;

/// Re-export ParseError for external use
pub use errors::ParseError;

/// Protocol method argument specification.
///
/// This struct represents a complete specification for a protocol method
/// argument, including type information, documentation, and usage constraints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Argument {
    /// Names of the argument - alternative identifiers
    pub names: Vec<String>,
    /// Description of the argument - full documentation
    pub description: String,
    /// One-line description of the argument - concise help text
    #[serde(default, rename = "oneline_description")]
    pub oneline_description: String,
    /// Whether the argument can also be passed positionally
    #[serde(default, rename = "also_positional")]
    pub also_positional: bool,
    /// Type string representation - alternative type descriptions
    #[serde(default, rename = "type_str")]
    pub type_str: Option<Vec<String>>,
    /// Whether the argument is required for the method call
    pub required: bool,
    /// Whether the argument is hidden from documentation
    #[serde(default)]
    pub hidden: bool,
    /// Type of the argument - primary type identifier
    #[serde(rename = "type")]
    pub type_: String,
}

/// Protocol method result specification.
///
/// This struct represents the specification for a return value from a
/// protocol method, including type information, optionality, and conditional presence.
/// It supports nested structures through the `inner` field for complex return types.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct MethodResult {
    /// Type of the result - primary type identifier
    #[serde(rename = "type")]
    pub type_: String,
    /// Whether the result is optional in the response
    #[serde(default, rename = "optional")]
    pub optional: bool,
    /// Description of the result - human-readable content description
    pub description: String,
    /// Key name for the result - JSON key in the response
    #[serde(default, rename = "key_name")]
    pub key_name: String,
    /// Condition for when this result is present - optional condition
    #[serde(default)]
    pub condition: String,
    /// Inner results for nested structures - recursive result specifications
    #[serde(default)]
    pub inner: Vec<MethodResult>,
}

impl MethodResult {
    /// Creates a new `MethodResult` with the specified parameters.
    ///
    /// This constructor provides a convenient way to create a `MethodResult` with
    /// all fields explicitly specified. It's useful when you have complete
    /// information about the result specification.
    pub fn new(
        type_: String,
        optional: bool,
        description: String,
        key_name: String,
        condition: String,
        inner: Vec<MethodResult>,
    ) -> Self {
        Self { type_, optional, description, key_name, condition, inner }
    }

    /// Returns whether the result is required (computed from optional).
    ///
    /// This is a convenience method that returns the inverse of the `optional` field.
    /// It provides a more intuitive way to check if a result is mandatory in the response.
    pub fn required(&self) -> bool { !self.optional }
}

/// Complete protocol API definition container.
///
/// This struct serves as the root container for all protocol method definitions
/// in a given API version. It provides efficient lookup and iteration capabilities
/// for method analysis, code generation, and documentation generation.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ApiDefinition {
    /// Protocol name - establishes the underlying protocol (bitcoin, lightning)
    #[serde(default)]
    pub protocol: Protocol,
    /// API schema version - for tracking the schema format itself (deprecated, use ProtocolVersion instead)
    #[serde(default)]
    pub version: ProtocolVersion,
    /// List of methods as raw JSON values for protocol-specific processing
    pub methods: std::collections::BTreeMap<String, serde_json::Value>,
}

impl ApiDefinition {
    /// Creates a new API definition with a specific implementation and version
    ///
    /// # Errors
    ///
    /// Returns `SchemaError::InvalidVersion` if the version string is empty or invalid
    pub fn with_implementation_and_version(
        implementation: Implementation,
        version: ProtocolVersion,
    ) -> std::result::Result<Self, SchemaError> {
        if version.as_str().trim().is_empty() {
            return Err(SchemaError::InvalidVersion("Version string cannot be empty".to_string()));
        }
        // Convert Implementation to Protocol
        let protocol = match implementation {
            Implementation::BitcoinCore => Protocol::Bitcoin,
            Implementation::CoreLightning | Implementation::Lnd | Implementation::RustLightning =>
                Protocol::Lightning,
        };
        Ok(Self { protocol, version, methods: std::collections::BTreeMap::new() })
    }

    /// Gets a method by name as raw JSON
    pub fn get_method(&self, name: &str) -> Option<&serde_json::Value> { self.methods.get(name) }

    /// Gets a mutable reference to a method by name
    pub fn get_method_mut(&mut self, name: &str) -> Option<&mut serde_json::Value> {
        self.methods.get_mut(name)
    }

    /// Adds a method to the API definition
    pub fn add_method(&mut self, name: String, method: serde_json::Value) {
        self.methods.insert(name, method);
    }

    /// Removes a method from the API definition
    pub fn remove_method(&mut self, name: &str) -> Option<serde_json::Value> {
        self.methods.remove(name)
    }

    /// Returns an iterator over all methods
    pub fn iter_methods(&self) -> impl Iterator<Item = (&String, &serde_json::Value)> {
        self.methods.iter()
    }

    /// Returns the number of methods in the API definition
    pub fn method_count(&self) -> usize { self.methods.len() }

    /// Checks if the API definition is empty
    pub fn is_empty(&self) -> bool { self.methods.is_empty() }
}

/// Error types for schema operations
#[derive(Error, Debug)]
pub enum SchemaError {
    /// JSON parsing error
    #[error("Failed to parse JSON: {0}")]
    JsonParse(#[from] serde_json::Error),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid version string
    #[error("Invalid version string: {0}")]
    InvalidVersion(String),
}

/// Result type for schema operations
pub type Result<T> = std::result::Result<T, SchemaError>;

/// Type mapping registry for protocol to Rust type conversion.
///
/// This utility struct provides static methods for mapping protocol types
/// to appropriate Rust types during code generation. It handles the conversion
/// between protocol type systems and Rust's type system.
pub struct TypeRegistry;

impl TypeRegistry {
    /// Map protocol argument type to Rust type using generic type mapping.
    ///
    /// This method handles basic JSON schema types (string, number, boolean, object, array)
    /// and maps them to corresponding Rust types. This should only be used for transport-level
    /// code generation when protocol-specific adapters aren't available.
    ///
    /// For protocol-specific type mappings, use `map_argument_type_with_adapter()` instead.
    ///
    /// # Arguments
    /// * `arg` - The argument specification containing type and requirement information
    ///
    /// # Returns
    /// A tuple containing:
    /// - `String`: The corresponding Rust type name for the argument's type
    /// - `bool`: Whether the argument should be wrapped in `Option<T>` (true if not required)
    ///
    /// # Panics
    /// Panics if the argument type is not one of the basic JSON schema types (string, number,
    /// int, integer, boolean, bool, object, array). Use a protocol-specific adapter for
    /// protocol-specific types.
    pub fn map_argument_type(arg: &Argument) -> (String, bool) {
        let base_type = match arg.type_.as_str() {
            "string" => "String",
            "number" | "int" | "integer" => "i64",
            "boolean" | "bool" => "bool",
            "object" => "serde_json::Value",
            "array" => "Vec<serde_json::Value>",
            unknown => panic!(
				"Unmapped argument type '{}' for argument '{}'. Use map_argument_type_with_adapter() \
				with a protocol-specific adapter instead.",
				unknown,
				arg.names.first().map(|n| n.as_str()).unwrap_or("<unnamed>")
			),
        };
        (base_type.to_string(), !arg.required)
    }

    /// Map protocol result type to Rust type using protocol-specific adapter.
    ///
    /// This method bridges the gap between protocol-specific type systems and Rust's
    /// type system by delegating to a protocol adapter that understands the protocol's
    /// type semantics. Each protocol (Bitcoin Core, Core Lightning, etc.) can define
    /// its own type mappings for specialized types like "difficulty" → f64 or "msat" → u64.
    /// The adapter pattern allows the code generation system to remain protocol-agnostic
    /// while supporting protocol-specific optimizations and type safety.
    ///
    /// # Arguments
    /// * `result` - The method result specification containing type and metadata
    /// * `adapter` - Protocol-specific adapter that knows how to map types
    ///
    /// # Returns
    /// A tuple containing:
    /// - `String`: The corresponding Rust type name
    /// - `bool`: Whether the result should be wrapped in `Option<T>` (true if optional)
    pub fn map_result_type(
        result: &MethodResult,
        adapter: &dyn type_adapter::TypeAdapter,
    ) -> (String, bool) {
        let base_type = adapter.map_type_to_rust(result);
        (base_type, result.optional)
    }

    /// Map protocol argument type to Rust type using protocol-specific adapter.
    ///
    /// This method bridges the gap between protocol-specific parameter types and Rust's
    /// type system by delegating to a protocol adapter that understands the protocol's
    /// type semantics. Each protocol (Bitcoin Core, Core Lightning, etc.) can define
    /// its own parameter type mappings for specialized types like "hex" → String.
    /// The adapter pattern allows the code generation system to remain protocol-agnostic
    /// while supporting protocol-specific optimizations and type safety.
    ///
    /// # Arguments
    /// * `arg` - The argument specification containing type and metadata
    /// * `adapter` - Protocol-specific adapter that knows how to map parameter types
    ///
    /// # Returns
    /// A tuple containing:
    /// - `String`: The corresponding Rust type name
    /// - `bool`: Whether the argument should be wrapped in `Option<T>` (true if not required)
    pub fn map_argument_type_with_adapter(
        arg: &Argument,
        adapter: &dyn type_adapter::TypeAdapter,
    ) -> (String, bool) {
        let base_type = adapter.map_parameter_type_to_rust(&arg.type_, &arg.names[0]);
        (base_type, !arg.required)
    }
}
