//! Bitcoin Core type adapter
//!
//! This module implements the `TypeAdapter` trait for Bitcoin Core RPC methods.
//! It handles Bitcoin Core-specific type mappings and response type parsing.

use ir::{RpcDef, TypeDef};

use super::bitcoin_core_utils;
use crate::type_adapter::TypeAdapter;
use crate::MethodResult;

/// Bitcoin Core type adapter implementation.
///
/// This adapter handles Bitcoin Core's normalized `MethodResult` format
/// and provides Bitcoin Core-specific type mappings for fields like difficulty,
/// verification progress, and other Bitcoin Core numeric types.
pub struct BitcoinCoreAdapter;

impl TypeAdapter for BitcoinCoreAdapter {
    fn protocol_name(&self) -> &str { "bitcoin_core" }

    fn parse_response_schema(&self, rpc: &RpcDef) -> Option<Vec<MethodResult>> {
        if let Some(result_type) = &rpc.result {
            // Convert TypeDef to MethodResult format
            let method_result = self.convert_typedef_to_method_result(result_type);
            Some(vec![method_result])
        } else {
            None
        }
    }

    fn map_type_to_rust(&self, result: &crate::MethodResult) -> String {
        // Bitcoin Core-specific type mappings
        match (&result.type_[..], result.key_name.as_str()) {
            // Bitcoin Core-specific floating point fields
            ("number", "difficulty") => "f64".to_string(),
            ("number", "verificationprogress") => "f64".to_string(),
            ("number", "relayfee") => "f64".to_string(),
            ("number", "incrementalfee") => "f64".to_string(),
            ("number", "incrementalrelayfee") => "f64".to_string(),
            ("number", "networkhashps") => "f64".to_string(),
            ("number", "mempoolminfee") => "f64".to_string(),
            ("number", "minrelaytxfee") => "f64".to_string(),
            ("amount", "mempoolminfee") => "f64".to_string(),
            ("amount", "minrelaytxfee") => "f64".to_string(),
            ("amount", "total_fee") => "f64".to_string(),
            ("amount", "blockmintxfee") => "f64".to_string(),
            ("boolean", "permitbaremultisig") => "Option<bool>".to_string(),

            // Bitcoin Core-specific hex fields (transaction IDs, block hashes, etc.)
            ("hex", _) => "String".to_string(),

            // Handle methods that return difficulty values directly (like getdifficulty)
            ("number", "") if result.description.contains("difficulty") => "f64".to_string(),

            // Standard type mappings
            ("string", _) => "String".to_string(),
            ("number" | "int" | "integer", _) => "i64".to_string(),
            ("boolean" | "bool", _) => "bool".to_string(),
            ("array", _) => "Vec<serde_json::Value>".to_string(),
            ("object", _) => "serde_json::Value".to_string(),
            ("none", _) => "()".to_string(),
            _ => "serde_json::Value".to_string(),
        }
    }

    fn map_parameter_type_to_rust(&self, param_type: &str, param_name: &str) -> String {
        bitcoin_core_utils::map_parameter_type_to_rust(param_type, param_name)
    }

    fn generate_implementation_types(&self) -> Option<String> {
        Some(
            "use serde::{Deserialize, Serialize};
use bitcoin::BlockHash;

/// Represents either a Bitcoin block hash or block height
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HashOrHeight {
    /// Bitcoin block hash
    Hash(BlockHash),
    /// Block height as an integer
    Height(i64),
}
"
            .to_string(),
        )
    }
}

impl BitcoinCoreAdapter {
    /// Convert a TypeDef to MethodResult format
    /// Recursively handles nested structures by populating the `inner` field
    /// when a field's type itself has nested fields.
    fn convert_typedef_to_method_result(&self, type_def: &TypeDef) -> MethodResult {
        // Convert TypeDef fields to MethodResult format
        let inner_results = if let Some(fields) = &type_def.fields {
            fields
                .iter()
                .map(|field| {
                    let is_optional_override = matches!(field.name.as_str(), "permitbaremultisig");

                    // Recursively handle nested structures: if the field's type has its own fields,
                    // recursively convert them to nested MethodResult entries
                    let nested_inner = if field.field_type.fields.is_some() {
                        // Recursively convert the nested type's fields
                        self.convert_typedef_to_method_result(&field.field_type).inner
                    } else {
                        Vec::new()
                    };

                    MethodResult {
                        type_: field.field_type.name.clone(),
                        optional: !field.required || is_optional_override,
                        description: field.description.clone(),
                        key_name: field.name.clone(),
                        condition: String::new(),
                        inner: nested_inner,
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        MethodResult {
            type_: type_def.name.clone(),
            optional: false,
            description: type_def.description.clone(),
            key_name: String::new(),
            condition: String::new(),
            inner: inner_results,
        }
    }
}
