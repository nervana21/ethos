//! Shared Bitcoin Core type mapping utilities
//!
//! This module provides shared utilities for Bitcoin Core type mapping that are used
//! by both the `TypeAdapter` implementation in this crate and the `BitcoinCoreTypeRegistry`
//! in the `adapters` crate.

/// Normalize field name for matching (lowercase, strip special characters)
///
/// This function normalizes field names by:
/// - Converting to lowercase
/// - Removing underscores, hyphens, and spaces
///
/// This normalization is used to match field names against categorization rules,
/// allowing rules to match regardless of naming conventions (e.g., "block_hash",
/// "block-hash", "blockhash" all match).
///
/// # Examples
///
/// ```
/// use types::adapters::bitcoin_core_utils::normalize_field_name;
///
/// assert_eq!(normalize_field_name("block_hash"), "blockhash");
/// assert_eq!(normalize_field_name("block-hash"), "blockhash");
/// assert_eq!(normalize_field_name("BlockHash"), "blockhash");
/// assert_eq!(normalize_field_name("tx_id"), "txid");
/// ```
pub fn normalize_field_name(name: &str) -> String {
    name.chars().filter(|c| !matches!(c, '_' | '-' | ' ')).flat_map(|c| c.to_lowercase()).collect()
}

/// Map Bitcoin Core parameter type to Rust type based on type and field name
///
/// This function implements the parameter-specific type mapping rules for Bitcoin Core.
/// It handles the conversion from Bitcoin Core RPC parameter types to appropriate Rust types.
///
/// The mapping rules prioritize specific field-name matches (e.g., "blockhash" → `bitcoin::BlockHash`)
/// over generic type mappings (e.g., "string" → `String`).
///
/// # Arguments
///
/// * `param_type` - The parameter type name (e.g., "string", "number", "hex")
/// * `param_name` - The parameter name for context-specific mapping
///
/// # Returns
///
/// Rust type as a string (e.g., "String", "i64", "bitcoin::BlockHash")
pub fn map_parameter_type_to_rust(param_type: &str, param_name: &str) -> String {
    let normalized_param = normalize_field_name(param_name);

    match param_type {
        "string" | "hex" => {
            // Specific field-name rules for strongly-typed Bitcoin types
            // Fall back to String for generic string/hex parameters
            match normalized_param.as_str() {
                "address" => "bitcoin::Address".to_string(),
                "blockhash" => "bitcoin::BlockHash".to_string(),
                "txid" => "bitcoin::Txid".to_string(),
                "scriptpubkey" | "script_pubkey" => "bitcoin::ScriptBuf".to_string(),
                "script" => "bitcoin::ScriptBuf".to_string(),
                "redeemscript" | "redeem_script" => "bitcoin::ScriptBuf".to_string(),
                "witnessscript" | "witness_script" => "bitcoin::ScriptBuf".to_string(),
                _ => "String".to_string(),
            }
        }
        "number" | "int" | "integer" => {
            // All numbers are i64 by default (including signed integers that can be negative)
            // Specific field names like "changepos", "confirmations", "nblocks" can accept -1
            "i64".to_string()
        }
        "boolean" | "bool" => "bool".to_string(),
        "object" => "serde_json::Value".to_string(),
        "array" => "Vec<serde_json::Value>".to_string(),
        "range" => "serde_json::Value".to_string(),
        _ => "serde_json::Value".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_field_name() {
        let normalized = normalize_field_name("A_b- C");
        assert_eq!(normalized, "abc");
    }

    #[test]
    fn test_map_parameter_type_to_rust() {
        let address = map_parameter_type_to_rust("string", "address");
        assert_eq!(address, "bitcoin::Address");

        let blockhash = map_parameter_type_to_rust("string", "blockhash");
        assert_eq!(blockhash, "bitcoin::BlockHash");

        let txid = map_parameter_type_to_rust("string", "txid");
        assert_eq!(txid, "bitcoin::Txid");

        let script_pubkey = map_parameter_type_to_rust("string", "script_pubkey");
        assert_eq!(script_pubkey, "bitcoin::ScriptBuf");

        let script = map_parameter_type_to_rust("string", "script");
        assert_eq!(script, "bitcoin::ScriptBuf");

        let redeemscript = map_parameter_type_to_rust("string", "redeemscript");
        assert_eq!(redeemscript, "bitcoin::ScriptBuf");

        let witnessscript = map_parameter_type_to_rust("string", "witnessscript");
        assert_eq!(witnessscript, "bitcoin::ScriptBuf");

        let generic_string = map_parameter_type_to_rust("string", "generic");
        assert_eq!(generic_string, "String");

        let number = map_parameter_type_to_rust("number", "any");
        assert_eq!(number, "i64");

        let bool_type = map_parameter_type_to_rust("bool", "any");
        assert_eq!(bool_type, "bool");

        let object = map_parameter_type_to_rust("object", "any");
        assert_eq!(object, "serde_json::Value");

        let array = map_parameter_type_to_rust("array", "any");
        assert_eq!(array, "Vec<serde_json::Value>");

        let range = map_parameter_type_to_rust("range", "any");
        assert_eq!(range, "serde_json::Value");

        let unknown = map_parameter_type_to_rust("unknown", "any");
        assert_eq!(unknown, "serde_json::Value");
    }
}
