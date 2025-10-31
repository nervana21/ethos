//! Core Lightning Type Registry
//!
//! This module provides type mapping for Core Lightning RPC arguments and results,
//! converting Lightning-specific types to Rust types.

use ::types::{Argument, MethodResult};

/// Type registry for Core Lightning RPC methods
pub struct CoreLightningTypeRegistry;

impl CoreLightningTypeRegistry {
    /// Map Core Lightning argument types to Rust types
    pub fn map_argument_type(arg: &Argument) -> (&'static str, bool) {
        let arg_type = arg.type_.to_lowercase();

        match arg_type.as_str() {
            "hex" | "string" => ("String", false),
            "u32" | "u64" | "integer" => ("u64", false),
            "boolean" | "bool" => ("bool", false),
            "pubkey" | "publickey" => ("PublicKey", false),
            "short_channel_id" | "shortchannelid" => ("ShortChannelId", false),
            "msat" | "amount" => ("u64", false), // millisatoshi
            "feerate" => ("String", false), // feerate can be "slow", "normal", "urgent", or a number
            "json" => ("serde_json::Value", false),
            "array" => ("Vec<String>", false),
            unknown => panic!(
                "Unmapped Core Lightning argument type '{}' for argument '{}'. \
				Add a type mapping in CoreLightningTypeRegistry::map_argument_type()",
                unknown,
                arg.names.first().map(|n| n.as_str()).unwrap_or("<unnamed>")
            ),
        }
    }

    /// Map Core Lightning result types to Rust types
    pub fn map_result_type(result: &MethodResult) -> (&'static str, bool) {
        let result_type = result.type_.to_lowercase();

        match result_type.as_str() {
            "hex" | "string" => ("String", false),
            "u32" | "u64" | "integer" => ("u64", false),
            "boolean" | "bool" => ("bool", false),
            "pubkey" | "publickey" => ("PublicKey", false),
            "short_channel_id" | "shortchannelid" => ("ShortChannelId", false),
            "msat" | "amount" => ("u64", false), // millisatoshi
            "feerate" => ("String", false), // feerate can be "slow", "normal", "urgent", or a number
            "json" => ("serde_json::Value", false),
            "array" => ("Vec<String>", false),
            unknown => panic!(
                "Unmapped Core Lightning result type '{}' for result with key_name '{}'. \
				Add a type mapping in CoreLightningTypeRegistry::map_result_type()",
                unknown, result.key_name
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use ::types::{Argument, MethodResult};

    use super::*;

    #[test]
    fn test_map_argument_types() {
        let hex_arg = Argument {
            names: vec!["message".to_string()],
            description: "Hex message".to_string(),
            oneline_description: "".to_string(),
            also_positional: false,
            type_str: None,
            type_: "hex".to_string(),
            required: true,
            hidden: false,
        };
        let (rust_type, is_optional) = CoreLightningTypeRegistry::map_argument_type(&hex_arg);
        assert_eq!(rust_type, "String");
        assert!(!is_optional);
        let pubkey_arg = Argument {
            names: vec!["pubkey".to_string()],
            description: "Public key".to_string(),
            oneline_description: "".to_string(),
            also_positional: false,
            type_str: None,
            type_: "pubkey".to_string(),
            required: true,
            hidden: false,
        };
        let (rust_type, is_optional) = CoreLightningTypeRegistry::map_argument_type(&pubkey_arg);
        assert_eq!(rust_type, "PublicKey");
        assert!(!is_optional);
    }

    #[test]
    fn test_map_result_types() {
        let string_result = MethodResult {
            type_: "string".to_string(),
            optional: false,
            description: "Result string".to_string(),
            key_name: "result".to_string(),
            condition: String::new(),
            inner: Vec::new(),
        };
        let (rust_type, is_optional) = CoreLightningTypeRegistry::map_result_type(&string_result);
        assert_eq!(rust_type, "String");
        assert!(!is_optional);
        let msat_result = MethodResult {
            type_: "msat".to_string(),
            optional: false,
            description: "Amount in millisatoshis".to_string(),
            key_name: "amount".to_string(),
            condition: String::new(),
            inner: Vec::new(),
        };
        let (rust_type, is_optional) = CoreLightningTypeRegistry::map_result_type(&msat_result);
        assert_eq!(rust_type, "u64");
        assert!(!is_optional);
    }
}
