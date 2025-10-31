//! Core Lightning type adapter
//!
//! This module implements the `TypeAdapter` trait for Core Lightning RPC methods.
//! It handles Core Lightning-specific type mappings and response type parsing.

use ir::{RpcDef, TypeDef};

use crate::type_adapter::TypeAdapter;
use crate::MethodResult;

/// Core Lightning type adapter implementation.
///
/// This adapter handles Core Lightning's normalized `MethodResult` format
/// and provides type mappings for Lightning Network protocol types (msat, sat, hex).
pub struct CoreLightningAdapter;

impl TypeAdapter for CoreLightningAdapter {
    fn protocol_name(&self) -> &str { "core_lightning" }

    fn parse_response_schema(&self, rpc: &RpcDef) -> Option<Vec<MethodResult>> {
        if let Some(result_type) = &rpc.result {
            let method_result = self.convert_typedef_to_method_result(result_type);
            Some(vec![method_result])
        } else {
            None
        }
    }

    fn map_type_to_rust(&self, result: &crate::MethodResult) -> String {
        // Core Lightning-specific type mappings
        match &result.type_[..] {
            // Lightning Network amount types
            "sat" | "satoshi" | "satoshis" => "u64".to_string(),
            "msat" | "millisatoshis" => "u64".to_string(),

            // Lightning Network data types
            "hex" => "String".to_string(),

            // Lightning Network numeric types
            "u32" => "u32".to_string(),
            "u64" => "u64".to_string(),

            // Standard types
            "string" => "String".to_string(),
            "number" | "integer" => "i64".to_string(),
            "boolean" | "bool" => "bool".to_string(),
            "object" => "serde_json::Value".to_string(),
            "array" => "serde_json::Value".to_string(),
            "none" => "()".to_string(),

            unknown => panic!(
                "Unmapped Core Lightning result type '{}' for result with key_name '{}'. \
				Add a type mapping in CoreLightningAdapter::map_type_to_rust()",
                unknown, result.key_name
            ),
        }
    }

    fn map_parameter_type_to_rust(&self, param_type: &str, _param_name: &str) -> String {
        // Core Lightning-specific parameter type mappings
        match param_type {
            "string" => "String".to_string(),
            "number" | "int" | "integer" => "i64".to_string(),
            "boolean" | "bool" => "bool".to_string(),
            "hex" => "String".to_string(), // Core Lightning hex parameters are strings
            "sat" | "satoshi" | "satoshis" => "u64".to_string(),
            "msat" | "millisatoshis" => "u64".to_string(),
            "u32" => "u32".to_string(),
            "u64" => "u64".to_string(),
            "object" => "serde_json::Value".to_string(),
            "array" => "Vec<serde_json::Value>".to_string(),
            _ => "serde_json::Value".to_string(),
        }
    }

    // Removed has_strongly_typed_response - no longer used

    fn generate_implementation_types(&self) -> Option<String> {
        Some(
            r#"use serde::{Deserialize, Serialize};

// Re-export commonly used types for convenience
pub use bitcoin::{PublicKey, Script};

/// Represents a short channel ID in the Lightning Network
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ShortChannelId {
    /// The block height
    pub block_height: u32,
    /// The transaction index
    pub tx_index: u16,
    /// The output index
    pub output_index: u16,
}

impl ShortChannelId {
    /// Create a new ShortChannelId from its components
    pub fn new(block_height: u32, tx_index: u16, output_index: u16) -> Self {
        Self {
            block_height,
            tx_index,
            output_index,
        }
    }

    /// Parse a short channel ID from a string (format: blockheight:txindex:outputindex)
    pub fn from_str(s: &str) -> Result<Self, String> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 3 {
            return Err(format!("Invalid short channel ID format: {}", s));
        }

        let block_height = parts[0].parse::<u32>()
            .map_err(|e| format!("Invalid block height: {}", e))?;
        let tx_index = parts[1].parse::<u16>()
            .map_err(|e| format!("Invalid tx index: {}", e))?;
        let output_index = parts[2].parse::<u16>()
            .map_err(|e| format!("Invalid output index: {}", e))?;

        Ok(Self::new(block_height, tx_index, output_index))
    }

    /// Convert to string format
    pub fn to_string(&self) -> String {
        format!("{}:{}:{}", self.block_height, self.tx_index, self.output_index)
    }
}

/// Lightning Network amount in millisatoshis
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct MilliSatoshi(pub u64);

impl MilliSatoshi {
    /// Create from satoshis
    pub fn from_sat(sat: u64) -> Self {
        Self(sat * 1000)
    }

    /// Convert to satoshis (rounded down)
    pub fn to_sat(&self) -> u64 {
        self.0 / 1000
    }
}

/// Lightning Network amount in satoshis
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Satoshi(pub u64);

impl Satoshi {
    /// Create from millisatoshis
    pub fn from_msat(msat: u64) -> Self {
        Self(msat / 1000)
    }

    /// Convert to millisatoshis
    pub fn to_msat(&self) -> u64 {
        self.0 * 1000
    }
}
"#
            .to_string(),
        )
    }
}

impl CoreLightningAdapter {
    /// Convert a TypeDef to MethodResult format
    /// Recursively handles nested structures by populating the `inner` field
    /// when a field's type itself has nested fields.
    fn convert_typedef_to_method_result(&self, type_def: &TypeDef) -> MethodResult {
        // Convert TypeDef fields to MethodResult format
        let inner_results = if let Some(fields) = &type_def.fields {
            fields
                .iter()
                .map(|field| {
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
                        optional: !field.required,
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
