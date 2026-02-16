//! Type Adapter Trait for Protocol-Specific Type Mapping
//!
//! This module defines the `TypeAdapter` trait that allows each protocol
//! to define its own response type mapping strategy. This makes the code
//! generation protocol-agnostic and extensible to new protocols.

use ir::RpcDef;

/// Trait for protocol-specific type adapters that handle response type generation.
///
/// Each protocol (e.g. Bitcoin Core) implements this trait
/// to define how it parses response types from IR and maps them to Rust equivalents.
/// This allows the code generation system to be protocol-agnostic while
/// supporting protocol-specific optimizations and type mappings.
///
/// ## Usage
///
/// ```rust
/// use types::type_adapter::TypeAdapter;
/// use types::MethodResult;
/// use ir::RpcDef;
///
/// struct MyProtocolAdapter;
///
/// impl TypeAdapter for MyProtocolAdapter {
///     fn protocol_name(&self) -> &str { "my_protocol" }
///
///     fn parse_response_schema(&self, rpc: &RpcDef) -> Option<Vec<MethodResult>> {
///         // Parse protocol-specific schema format
///         None
///     }
///
///     fn map_type_to_rust(&self, result: &MethodResult) -> String {
///         // Map protocol types to Rust types
///         "String".to_string()
///     }
/// }
/// ```
pub trait TypeAdapter: Send + Sync {
    /// Protocol name for logging and debugging purposes.
    ///
    /// Should return a short, descriptive name like "bitcoin_core", etc.
    fn protocol_name(&self) -> &str;

    /// Parse protocol-specific response schema into normalized MethodResult format.
    ///
    /// This method takes a protocol method and extracts the response schema
    /// in the protocol's native format, then converts it to the normalized
    /// `MethodResult` format used by the code generation system.
    ///
    /// # Arguments
    /// * `rpc` - The RPC method definition to parse
    ///
    /// # Returns
    /// * `Some(Vec<MethodResult>)` - Parsed response schema if available
    /// * `None` - If no structured response schema exists
    fn parse_response_schema(&self, rpc: &RpcDef) -> Option<Vec<crate::MethodResult>>;

    /// Map protocol-specific type to Rust type.
    ///
    /// This method handles the conversion from protocol-specific type names
    /// to appropriate Rust types. Each protocol can define its own mappings
    /// for specialized types (e.g., Bitcoin's "difficulty" → f64).
    ///
    /// # Arguments
    /// * `result` - The method result containing type, field name, and description
    ///
    /// # Returns
    /// Rust type as a string (e.g., "String", "u64", "f64")
    fn map_type_to_rust(&self, result: &crate::MethodResult) -> String;

    /// Map protocol-specific parameter type to Rust type.
    ///
    /// This method handles the conversion from protocol-specific parameter types
    /// to appropriate Rust types. Each protocol can define its own mappings
    /// for specialized parameter types (e.g., Bitcoin's "string" → String, "hex" → String).
    ///
    /// # Arguments
    /// * `param_type` - The parameter type name (e.g., "string", "number", "boolean")
    /// * `param_name` - The parameter name for context-specific mapping
    ///
    /// # Returns
    /// Rust type as a string (e.g., "String", "i64", "bool")
    fn map_parameter_type_to_rust(&self, param_type: &str, _param_name: &str) -> String {
        // Default implementation falls back to generic mapping
        // Ordered by type complexity: primitives (bool → number → string) then composites (array → object)
        match param_type {
            "boolean" | "bool" => "bool".to_string(),
            "number" | "int" | "integer" => "i64".to_string(),
            "string" => "String".to_string(),
            "array" => "Vec<serde_json::Value>".to_string(),
            "object" => "serde_json::Value".to_string(),
            _ => "serde_json::Value".to_string(),
        }
    }

    /// Generate implementation-specific types
    ///
    /// This method allows each adapter to generate its own implementation-specific
    /// types (like HashOrHeight for Bitcoin Core) that are not part of the generic
    /// protocol but are specific to that implementation's API.
    ///
    /// # Returns
    ///
    /// Returns a string containing the Rust code for implementation-specific types,
    /// or None if this implementation doesn't need any custom types.
    fn generate_implementation_types(&self) -> Option<String> { None }
}
