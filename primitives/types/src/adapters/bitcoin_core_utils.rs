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
        assert_eq!(normalize_field_name("block_hash"), "blockhash");
        assert_eq!(normalize_field_name("block-hash"), "blockhash");
        assert_eq!(normalize_field_name("BlockHash"), "blockhash");
        assert_eq!(normalize_field_name("tx_id"), "txid");
        assert_eq!(normalize_field_name("txid"), "txid");
        assert_eq!(normalize_field_name("address"), "address");
    }

    #[test]
    fn test_map_parameter_type_to_rust() {
        // String/hex types with specific field names
        assert_eq!(map_parameter_type_to_rust("string", "blockhash"), "bitcoin::BlockHash");
        assert_eq!(map_parameter_type_to_rust("hex", "blockhash"), "bitcoin::BlockHash");
        assert_eq!(map_parameter_type_to_rust("string", "txid"), "bitcoin::Txid");
        assert_eq!(map_parameter_type_to_rust("hex", "txid"), "bitcoin::Txid");
        assert_eq!(map_parameter_type_to_rust("string", "address"), "bitcoin::Address");
        assert_eq!(map_parameter_type_to_rust("string", "scriptpubkey"), "bitcoin::ScriptBuf");
        assert_eq!(map_parameter_type_to_rust("hex", "scriptpubkey"), "bitcoin::ScriptBuf");
        assert_eq!(map_parameter_type_to_rust("string", "script_pubkey"), "bitcoin::ScriptBuf");
        assert_eq!(map_parameter_type_to_rust("string", "script"), "bitcoin::ScriptBuf");
        assert_eq!(map_parameter_type_to_rust("hex", "script"), "bitcoin::ScriptBuf");
        assert_eq!(map_parameter_type_to_rust("string", "redeemscript"), "bitcoin::ScriptBuf");
        assert_eq!(map_parameter_type_to_rust("hex", "redeem_script"), "bitcoin::ScriptBuf");
        assert_eq!(map_parameter_type_to_rust("string", "witnessscript"), "bitcoin::ScriptBuf");
        assert_eq!(map_parameter_type_to_rust("hex", "witness_script"), "bitcoin::ScriptBuf");
        assert_eq!(map_parameter_type_to_rust("string", "generic"), "String");
        assert_eq!(map_parameter_type_to_rust("hex", "generic"), "String");

        // Number types
        assert_eq!(map_parameter_type_to_rust("number", "any"), "i64");
        assert_eq!(map_parameter_type_to_rust("int", "any"), "i64");
        assert_eq!(map_parameter_type_to_rust("integer", "any"), "i64");

        // Boolean types
        assert_eq!(map_parameter_type_to_rust("boolean", "any"), "bool");
        assert_eq!(map_parameter_type_to_rust("bool", "any"), "bool");

        // Composite types
        assert_eq!(map_parameter_type_to_rust("object", "any"), "serde_json::Value");
        assert_eq!(map_parameter_type_to_rust("array", "any"), "Vec<serde_json::Value>");
        assert_eq!(map_parameter_type_to_rust("range", "any"), "serde_json::Value");
    }
}
