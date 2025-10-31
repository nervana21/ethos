//! LND Type Registry
//!
//! This module provides type mapping for LND RPC methods and parameters.

use ::types::{Argument, MethodResult};

/// LND-specific type registry for mapping RPC types
pub struct LndTypeRegistry;

impl LndTypeRegistry {
    /// Map an LND argument type to a Rust type
    pub fn map_argument_type(arg: &Argument) -> (String, bool) {
        let type_name = &arg.type_;
        let is_optional = !arg.required;

        match type_name.as_str() {
            "string" => ("String".to_string(), is_optional),
            "int64" => ("i64".to_string(), is_optional),
            "uint64" => ("u64".to_string(), is_optional),
            "int32" => ("i32".to_string(), is_optional),
            "uint32" => ("u32".to_string(), is_optional),
            "bool" => ("bool".to_string(), is_optional),
            "bytes" => ("Vec<u8>".to_string(), is_optional),
            "PublicKey" => ("PublicKey".to_string(), is_optional),
            "ShortChannelId" => ("ShortChannelId".to_string(), is_optional),
            "Satoshis" => ("u64".to_string(), is_optional),
            "MilliSatoshis" => ("u64".to_string(), is_optional),
            _ => ("String".to_string(), is_optional),
        }
    }

    /// Map an LND result type to a Rust type
    pub fn map_result_type(result: &MethodResult) -> (String, bool) {
        let type_name = &result.type_;
        let is_optional = result.optional;

        match type_name.as_str() {
            "string" => ("String".to_string(), is_optional),
            "int64" => ("i64".to_string(), is_optional),
            "uint64" => ("u64".to_string(), is_optional),
            "int32" => ("i32".to_string(), is_optional),
            "uint32" => ("u32".to_string(), is_optional),
            "bool" => ("bool".to_string(), is_optional),
            "bytes" => ("Vec<u8>".to_string(), is_optional),
            "PublicKey" => ("PublicKey".to_string(), is_optional),
            "ShortChannelId" => ("ShortChannelId".to_string(), is_optional),
            "Satoshis" => ("u64".to_string(), is_optional),
            "MilliSatoshis" => ("u64".to_string(), is_optional),
            _ => ("String".to_string(), is_optional),
        }
    }
}

#[cfg(test)]
mod tests {
    use ::types::{Argument, MethodResult};

    use super::*;

    #[test]
    fn test_map_argument_type() {
        let arg = Argument {
            names: vec!["test_param".to_string()],
            description: "Test parameter".to_string(),
            oneline_description: "Test parameter".to_string(),
            also_positional: false,
            type_str: None,
            required: true,
            hidden: false,
            type_: "string".to_string(),
        };
        let (rust_type, is_optional) = LndTypeRegistry::map_argument_type(&arg);
        assert_eq!(rust_type, "String");
        assert!(!is_optional);
    }

    #[test]
    fn test_map_result_type() {
        let result = MethodResult {
            type_: "int64".to_string(),
            optional: false,
            description: "Test result".to_string(),
            key_name: "result".to_string(),
            condition: "".to_string(),
            inner: vec![],
        };
        let (rust_type, is_optional) = LndTypeRegistry::map_result_type(&result);
        assert_eq!(rust_type, "i64");
        assert!(!is_optional);
    }
}
