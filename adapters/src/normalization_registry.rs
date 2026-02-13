//! Normalization Registry for Differential Fuzzing
//!
//! This module provides unified normalization logic with configurable rules
//! stored in JSON metadata. It handles field name mapping, unit conversions,
//! and volatile field filtering for consistent comparison across adapters.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use types::Implementation;

/// Errors that can occur during normalization
#[derive(Debug, Error)]
pub enum NormalizationError {
    /// IO error occurred
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON parsing error occurred
    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    /// Normalization rule not found
    #[error("Normalization rule not found: {0}")]
    RuleNotFound(String),

    /// Invalid normalization rule
    #[error("Invalid normalization rule: {0}")]
    InvalidRule(String),
}

/// A normalization rule for field processing
#[derive(Debug, Clone)]
pub struct NormalizationRule {
    /// The field path this rule applies to
    pub field_path: String,
    /// Whether to drop this field entirely
    pub drop_field: bool,
    /// The canonical field name to use
    pub canonical_name: Option<String>,
    /// The canonical value type
    pub value_type: Option<ValueType>,
    /// Unit conversion rules
    pub unit_conversion: Option<UnitConversion>,
    /// Custom transformation function name
    pub transform: Option<String>,
}

/// Value type for normalization
#[derive(Debug, Clone)]
pub enum ValueType {
    /// String value type
    String,
    /// Number value type
    Number,
    /// Boolean value type
    Boolean,
    /// Object value type
    Object,
    /// Array value type
    Array,
}

/// Unit conversion rules
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UnitConversion {
    /// Source unit pattern
    pub from_pattern: String,
    /// Target unit
    pub to_unit: String,
    /// Conversion factor
    pub factor: f64,
}

/// Normalization metadata for tracking applied rules
#[derive(Debug, Clone)]
pub struct NormalizationMetadata {
    /// Rules that were applied
    pub applied_rules: Vec<String>,
    /// Fields that were dropped
    pub dropped_fields: Vec<String>,
    /// Fields that were renamed
    pub renamed_fields: HashMap<String, String>,
    /// Unit conversions performed
    pub unit_conversions: Vec<String>,
}

/// Adapter kind for method name translation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterKind {
    /// Bitcoin Core adapter
    BitcoinCore,
    /// Core Lightning adapter
    CoreLightning,
    /// LND adapter
    Lnd,
    /// Rust Lightning adapter
    RustLightning,
}

impl From<Implementation> for AdapterKind {
    fn from(impl_: Implementation) -> Self {
        match impl_ {
            Implementation::BitcoinCore => AdapterKind::BitcoinCore,
            Implementation::CoreLightning => AdapterKind::CoreLightning,
            Implementation::Lnd => AdapterKind::Lnd,
            Implementation::RustLightning => AdapterKind::RustLightning,
        }
    }
}

/// Registry for normalization rules
#[derive(Default, Debug, Clone)]
pub struct NormalizationRegistry {
    /// Field name mappings
    field_mappings: HashMap<String, String>,
    /// Unit conversion rules
    unit_conversions: HashMap<String, UnitConversion>,
    /// Volatile fields to drop
    volatile_fields: Vec<String>,
    /// Method name mappings per adapter
    method_mappings: HashMap<AdapterKind, HashMap<String, String>>,
}

impl NormalizationRegistry {
    /// Load normalization rules from a JSON file
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, NormalizationError> {
        let content = fs::read_to_string(path)?;
        let rules: Value = serde_json::from_str(&content)?;
        let mut registry = Self::default();
        registry.load_rules_from_json(&rules)?;
        Ok(registry)
    }

    /// Construct a registry from a named preset in resources/adapters/normalization/{preset}.json
    pub fn from_preset(preset: &str) -> Result<Self, NormalizationError> {
        let path = format!("resources/adapters/normalization/{}.json", preset);
        Self::from_file(&path)
    }

    /// Construct a registry for a specific adapter type using a conventional preset name
    pub fn for_adapter(adapter: AdapterKind) -> Result<Self, NormalizationError> {
        let preset = match adapter {
            AdapterKind::BitcoinCore => "bitcoin",
            AdapterKind::CoreLightning => "lightning",
            AdapterKind::Lnd => "lightning",
            AdapterKind::RustLightning => "lightning",
        };
        Self::from_preset(preset)
    }

    /// Load rules from JSON configuration
    fn load_rules_from_json(&mut self, rules: &Value) -> Result<(), NormalizationError> {
        if let Some(field_mappings) = rules.get("field_mappings") {
            if let Some(mappings) = field_mappings.as_object() {
                for (from, to) in mappings {
                    if let Some(to_str) = to.as_str() {
                        self.add_field_mapping(from, to_str);
                    }
                }
            }
        }

        if let Some(unit_conversions) = rules.get("unit_conversions") {
            if let Some(conversions) = unit_conversions.as_object() {
                for (field, conversion) in conversions {
                    if let Some(conv_obj) = conversion.as_object() {
                        if let (Some(from_pattern), Some(to_unit), Some(factor)) = (
                            conv_obj.get("from_pattern").and_then(|v| v.as_str()),
                            conv_obj.get("to_unit").and_then(|v| v.as_str()),
                            conv_obj.get("factor").and_then(|v| v.as_f64()),
                        ) {
                            self.add_unit_conversion(
                                field,
                                UnitConversion {
                                    from_pattern: from_pattern.to_string(),
                                    to_unit: to_unit.to_string(),
                                    factor,
                                },
                            );
                        }
                    }
                }
            }
        }

        if let Some(volatile_fields) = rules.get("volatile_fields") {
            if let Some(fields) = volatile_fields.as_array() {
                for field in fields {
                    if let Some(field_str) = field.as_str() {
                        self.add_volatile_field(field_str);
                    }
                }
            }
        }

        if let Some(method_mappings_value) = rules.get("method_mappings") {
            let parsed: HashMap<AdapterKind, HashMap<String, String>> =
                serde_json::from_value(method_mappings_value.clone())?;
            for (adapter, mappings) in parsed {
                for (canonical, adapter_specific) in mappings {
                    self.add_method_mapping(adapter, &canonical, &adapter_specific);
                }
            }
        }

        Ok(())
    }

    /// Add a field name mapping
    pub fn add_field_mapping(&mut self, from: &str, to: &str) {
        self.field_mappings.insert(from.to_string(), to.to_string());
    }

    /// Add a method name mapping for a specific adapter
    pub fn add_method_mapping(
        &mut self,
        adapter: AdapterKind,
        canonical: &str,
        adapter_specific: &str,
    ) {
        self.method_mappings
            .entry(adapter)
            .or_default()
            .insert(canonical.to_string(), adapter_specific.to_string());
    }

    /// Translate a canonical method name to adapter-specific method name
    pub fn to_adapter_method(&self, adapter: AdapterKind, canonical: &str) -> String {
        if let Some(adapter_mappings) = self.method_mappings.get(&adapter) {
            if let Some(mapped) = adapter_mappings.get(canonical) {
                return mapped.clone();
            }
        }
        canonical.to_string()
    }

    /// Add a unit conversion rule
    pub fn add_unit_conversion(&mut self, field: &str, conversion: UnitConversion) {
        self.unit_conversions.insert(field.to_string(), conversion);
    }

    /// Add a volatile field to drop
    pub fn add_volatile_field(&mut self, field: &str) {
        self.volatile_fields.push(field.to_string());
    }

    /// Normalize a JSON value using the registry rules
    pub fn normalize_value(&self, value: &Value) -> (Value, NormalizationMetadata) {
        let mut metadata = NormalizationMetadata {
            applied_rules: Vec::new(),
            dropped_fields: Vec::new(),
            renamed_fields: HashMap::new(),
            unit_conversions: Vec::new(),
        };

        let normalized = self.normalize_value_recursive(value, "", &mut metadata);
        (normalized, metadata)
    }

    /// Recursively normalize a JSON value
    fn normalize_value_recursive(
        &self,
        value: &Value,
        path: &str,
        metadata: &mut NormalizationMetadata,
    ) -> Value {
        match value {
            Value::Object(map) => {
                let mut normalized_map = serde_json::Map::new();

                for (key, val) in map {
                    let new_path =
                        if path.is_empty() { key.clone() } else { format!("{}.{}", path, key) };

                    // Check if field should be dropped
                    if self.should_drop_field(key) {
                        metadata.dropped_fields.push(new_path.clone());
                        continue;
                    }

                    // Apply field name mapping
                    let normalized_key = self.get_canonical_field_name(key);
                    if normalized_key != *key {
                        metadata.renamed_fields.insert(key.clone(), normalized_key.clone());
                    }

                    // Normalize the value
                    let normalized_value = self.normalize_value_recursive(val, &new_path, metadata);

                    // Apply unit conversions
                    let final_value =
                        self.apply_unit_conversions(&normalized_key, &normalized_value, metadata);

                    normalized_map.insert(normalized_key, final_value);
                }

                Value::Object(normalized_map)
            }
            Value::Array(arr) => {
                let normalized_arr: Vec<Value> = arr
                    .iter()
                    .enumerate()
                    .map(|(i, val)| {
                        let new_path = format!("{}[{}]", path, i);
                        self.normalize_value_recursive(val, &new_path, metadata)
                    })
                    .collect();
                Value::Array(normalized_arr)
            }
            _ => value.clone(),
        }
    }

    /// Check if a field should be dropped
    fn should_drop_field(&self, field_name: &str) -> bool {
        self.volatile_fields
            .iter()
            .any(|volatile| field_name.contains(volatile) || volatile.as_str().contains(field_name))
    }

    /// Get the canonical field name
    fn get_canonical_field_name(&self, field_name: &str) -> String {
        self.field_mappings.get(field_name).cloned().unwrap_or_else(|| field_name.to_string())
    }

    /// Apply unit conversions to a value
    fn apply_unit_conversions(
        &self,
        field_name: &str,
        value: &Value,
        metadata: &mut NormalizationMetadata,
    ) -> Value {
        if let Some(conversion) = self.unit_conversions.get(field_name) {
            if let Value::String(s) = value {
                // Simple string pattern matching for unit conversion
                if s.ends_with("msat") {
                    if let Some(msat_str) = s.strip_suffix("msat") {
                        if let Ok(num) = msat_str.parse::<f64>() {
                            let converted = (num * conversion.factor) as u64;
                            metadata
                                .unit_conversions
                                .push(format!("{}: {} -> {}", field_name, s, converted));
                            return Value::Number(converted.into());
                        }
                    }
                }
            }
        }
        value.clone()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_method_mapping() {
        let mut registry = NormalizationRegistry::default();
        registry.add_method_mapping(
            AdapterKind::BitcoinCore,
            "GetBlockChainInfo",
            "getblockchaininfo",
        );
        registry.add_method_mapping(AdapterKind::BitcoinCore, "GetBlockCount", "getblockcount");

        assert_eq!(
            registry.to_adapter_method(AdapterKind::BitcoinCore, "GetBlockChainInfo"),
            "getblockchaininfo"
        );
        assert_eq!(
            registry.to_adapter_method(AdapterKind::BitcoinCore, "GetBlockCount"),
            "getblockcount"
        );

        // Test unknown method passthrough
        assert_eq!(
            registry.to_adapter_method(AdapterKind::BitcoinCore, "UnknownMethod"),
            "UnknownMethod"
        );
    }

    #[test]
    fn test_field_mapping() {
        let mut registry = NormalizationRegistry::default();
        registry.add_field_mapping("msatoshi", "amount_msat");

        let input = json!({
            "msatoshi": 1000,
            "other_field": "value"
        });

        let (normalized, metadata) = registry.normalize_value(&input);

        assert!(normalized.get("amount_msat").is_some());
        assert!(normalized.get("msatoshi").is_none());
        assert!(metadata.renamed_fields.contains_key("msatoshi"));
    }

    #[test]
    fn test_volatile_field_dropping() {
        let mut registry = NormalizationRegistry::default();
        registry.add_volatile_field("timestamp");

        let input = json!({
            "timestamp": 1234567890,
            "amount": 1000
        });

        let (normalized, metadata) = registry.normalize_value(&input);

        assert!(normalized.get("timestamp").is_none());
        assert!(normalized.get("amount").is_some());
        assert!(metadata.dropped_fields.contains(&"timestamp".to_string()));
    }
}
