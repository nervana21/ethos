#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]

//! Shared types for fuzzing infrastructure
//!
//! This crate contains the common types used across the fuzzing system,
//! ensuring consistency and avoiding duplication.

use std::collections::HashMap;

use serde_json::Value;

/// A fuzz case containing the input to be tested
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FuzzCase {
    /// The method name to call
    pub method_name: String,
    /// Parameters for the method call
    pub parameters: HashMap<String, Value>,
    /// Expected result type (for validation)
    pub expected_result_type: Option<String>,
}

/// Result from executing a fuzz case against a protocol adapter
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FuzzResult {
    /// The adapter that produced this result
    pub adapter_name: String,
    /// The raw response from the adapter
    pub raw_response: Value,
    /// Whether the call succeeded
    pub success: bool,
    /// Error message if the call failed
    pub error: Option<String>,
    /// Normalized error for semantic comparison
    pub normalized_error: Option<NormalizedError>,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
}

/// Normalized representation of an RPC error for semantic comparison
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum NormalizedError {
    /// JSON-RPC error with standard code
    RpcError {
        /// The JSON-RPC error code (e.g., -32601 for method not found)
        code: i32,
        /// Extracted method name if it's a method-not-found error
        method: Option<String>,
    },
    /// Adapter/client not configured or available
    ClientUnavailable,
    /// Other error that couldn't be normalized
    Other(String),
}

impl NormalizedError {
    /// Try to parse from an error string
    pub fn from_error_string(error: &str) -> Self {
        // Parse JSON-RPC error code from patterns like: "code":-32601
        if let Some(code) = Self::extract_rpc_code(error) {
            let method = if code == -32601 { Self::extract_method_name(error) } else { None };
            return Self::RpcError { code, method };
        }

        // Check for client unavailable patterns
        if error.contains("not configured")
            || error.contains("not available")
            || error.contains("not initialized")
        {
            return Self::ClientUnavailable;
        }

        Self::Other(error.to_string())
    }

    /// Check if two errors are semantically equivalent
    pub fn is_equivalent(&self, other: &Self) -> bool {
        match (self, other) {
            // Same RPC error code (and method if present)
            (Self::RpcError { code: c1, method: m1 }, Self::RpcError { code: c2, method: m2 }) =>
                c1 == c2 && m1 == m2,
            // Both client unavailable
            (Self::ClientUnavailable, Self::ClientUnavailable) => true,
            // Otherwise must match exactly
            _ => self == other,
        }
    }

    fn extract_rpc_code(error: &str) -> Option<i32> {
        // Look for "code":-32601 pattern
        let start = error.find(r#""code""#)?;
        let colon = error[start..].find(':')?;
        let number_start = start + colon + 1;

        // Find the end of the number
        let rest = &error[number_start..];
        let number_str = rest.split(|c: char| !c.is_numeric() && c != '-').next()?.trim();

        number_str.parse().ok()
    }

    fn extract_method_name(error: &str) -> Option<String> {
        // Look for patterns like:
        // "Unknown command 'ListChannels'"
        // "Unknown method: ListChannels"
        // "method ListChannels not found"

        for pattern in &["command '", "command: '", "method: ", "method '"] {
            if let Some(start) = error.find(pattern) {
                let method_start = start + pattern.len();
                let method_end = error[method_start..]
                    .find(|c: char| c == '\'' || c == '"' || c.is_whitespace())
                    .map(|i| method_start + i)
                    .unwrap_or(error.len());

                let method = &error[method_start..method_end];
                if !method.is_empty() {
                    return Some(method.to_string());
                }
            }
        }

        None
    }
}

use async_trait::async_trait;

/// Trait for protocol adapters that can be used in differential fuzzing
#[async_trait]
pub trait ProtocolAdapter: Send + Sync {
    /// Get the name of this adapter
    fn name(&self) -> &'static str;

    /// Apply a fuzz case to this adapter and return the result
    async fn apply_fuzz_case(
        &self,
        case: &FuzzCase,
    ) -> Result<FuzzResult, Box<dyn std::error::Error + Send + Sync>>;

    /// Normalize output for comparison (handles field name differences, etc.)
    /// Default implementation returns the value unchanged
    fn normalize_output(&self, value: &Value) -> Value { value.clone() }
}

/// Trait for Bitcoin protocol adapters that can be used in differential fuzzing
#[async_trait]
pub trait BitcoinProtocolAdapter: ProtocolAdapter {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_json_rpc_method_not_found() {
        let error1 = r#"RPC error: {"code":-32601,"message":"Unknown command 'ListChannels'"}"#;
        let error2 = r#"RPC error: {"code":-32601,"message":"Unknown method: ListChannels"}"#;

        let norm1 = NormalizedError::from_error_string(error1);
        let norm2 = NormalizedError::from_error_string(error2);

        assert!(norm1.is_equivalent(&norm2));
    }

    #[test]
    fn test_normalize_client_unavailable() {
        let error1 = "LND RPC client not configured";
        let error2 = "Client not available";

        let norm1 = NormalizedError::from_error_string(error1);
        let norm2 = NormalizedError::from_error_string(error2);

        assert!(norm1.is_equivalent(&norm2));
    }
}
