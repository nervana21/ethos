// SPDX-License-Identifier: CC0-1.0

//! Bitcoin Core Schema Converter
//!
//! This module converts a raw schema.json output from Bitcoin Core's schema.cpp
//! into the ProtocolIR format used by the Ethos codebase.
//!
//! The raw schema.json has the following structure:
//! ```json
//! {
//!   "version": "v0.1.0",
//!   "version_major": 0,
//!   "version_minor": 1,
//!   "version_build": 0,
//!   "timestamp_ms": 1231006505000,
//!   "rpcs": {
//!     "getbestblockhash": {
//!       "name": "getbestblockhash",
//!       "category": "blockchain",
//!       "description": "Returns the hash of the best (tip) block in the longest blockchain.",
//!       "arguments": [],
//!       "results": [...]
//!     }
//!   }
//! }
//! ```
//!
//! This is converted to ProtocolIR format with proper TypeDef structures, field mappings, etc.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use ir::{FieldDef, ParamDef, ProtocolDef, ProtocolIR, ProtocolModule, RpcDef, TypeDef, TypeKind};
use path::{find_project_root, get_ir_dir, parse_version_components, version_ir_filename};
use semantics::method_categorization;
use serde::Deserialize;

/// Raw schema structure from Bitcoin Core's schema.cpp
#[derive(Debug, Clone, Deserialize)]
pub struct RawSchema {
    /// Version string (e.g., "v30.2.0")
    #[serde(default)]
    pub version: Option<String>,
    /// Version major number
    #[serde(default)]
    pub version_major: Option<u32>,
    /// Version minor number
    #[serde(default)]
    pub version_minor: Option<u32>,
    /// Version build number
    #[serde(default)]
    pub version_build: Option<u32>,
    /// Timestamp when schema was generated (milliseconds since Unix epoch)
    #[serde(default)]
    #[serde(rename = "timestamp_ms")]
    pub timestamp_ms: Option<i64>,
    /// Map of RPC method names to their definitions
    pub rpcs: BTreeMap<String, RawRpcMethod>,
}

/// Raw RPC method from Bitcoin Core
#[derive(Debug, Clone, Deserialize)]
pub struct RawRpcMethod {
    /// Name of the RPC method
    pub name: String,
    /// Category of the RPC method (e.g., "wallet", "blockchain")
    pub category: String,
    /// Description of what the RPC method does
    pub description: String,
    /// Example usage strings for the method
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_examples")]
    pub examples: Vec<String>,
    /// Ordered list of argument names
    #[serde(default)]
    pub argument_names: Vec<String>,
    /// List of argument definitions
    pub arguments: Vec<RawArgument>,
    /// List of result definitions
    pub results: Vec<RawResult>,
}

/// Deserialize examples field which can be either a string or an array of strings
fn deserialize_examples<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Examples {
        String(String),
        Array(Vec<String>),
    }

    match Examples::deserialize(deserializer)? {
        Examples::String(s) => Ok(vec![s]),
        Examples::Array(v) => Ok(v),
    }
}

/// Raw argument from Bitcoin Core
#[derive(Debug, Clone, Deserialize)]
pub struct RawArgument {
    /// List of possible names for this argument
    pub names: Vec<String>,
    /// Full description of the argument
    pub description: String,
    /// One-line description of the argument
    #[serde(default)]
    pub oneline_description: String,
    /// Whether this argument can also be passed positionally
    #[serde(default)]
    pub also_positional: bool,
    /// Type string representations
    #[serde(default)]
    pub type_str: Vec<String>,
    /// Whether this argument is required
    pub required: bool,
    /// Default value for the argument, if any
    #[serde(default)]
    pub default: Option<serde_json::Value>,
    /// Hint string for the default value
    #[serde(default)]
    pub default_hint: Option<String>,
    /// Whether this argument is hidden from documentation
    #[serde(default)]
    pub hidden: bool,
    /// Type of the argument (e.g., "string", "number", "object", "array")
    pub r#type: String,
    /// Nested inner arguments for complex types
    #[serde(default)]
    pub inner: Vec<RawArgument>,
}

/// Raw result from Bitcoin Core
#[derive(Debug, Clone, Deserialize)]
pub struct RawResult {
    /// Type of the result (e.g., "string", "number", "object", "array")
    pub r#type: String,
    /// Whether this result field is optional
    #[serde(default)]
    pub optional: bool,
    /// Description of what this result represents
    pub description: String,
    /// Whether to skip type checking for this result
    #[serde(default)]
    pub skip_type_check: bool,
    /// Name of the key for this result field
    pub key_name: String,
    /// Condition under which this result is present
    #[serde(default)]
    pub condition: String,
    /// Nested inner results for complex types
    #[serde(default)]
    pub inner: Vec<RawResult>,
}

/// Map Bitcoin Core type to protocol type
/// This unified function handles both argument and result types
fn map_protocol_type(bc_type: &str) -> String {
    match bc_type {
        // Common types
        "object" => "object".to_string(),
        "array" => "array".to_string(),
        "string" => "string".to_string(),
        "number" => "number".to_string(),
        "boolean" => "boolean".to_string(),
        "amount" => "amount".to_string(),
        "hex" => "hex".to_string(),
        // Result-specific types
        "none" => "none".to_string(),
        "any" => "any".to_string(),
        "timestamp" => "timestamp".to_string(),
        "elision" => "elision".to_string(),
        // Argument-specific types
        "range" => "range".to_string(),
        // Unknown types should cause a panic to ensure all types are explicitly handled
        unknown => panic!(
            "Unmapped Bitcoin Core type '{}'. Please add explicit handling for this type in map_protocol_type().",
            unknown
        ),
    }
}

/// Trait for types that have a type string and inner elements
trait HasTypeAndInner {
    fn has_object_inner(&self) -> bool;
}

impl HasTypeAndInner for RawArgument {
    fn has_object_inner(&self) -> bool { self.r#type == "object" && !self.inner.is_empty() }
}

impl HasTypeAndInner for RawResult {
    fn has_object_inner(&self) -> bool { self.r#type == "object" && !self.inner.is_empty() }
}

/// Trait for extracting field information from inner elements
trait InnerFieldInfo {
    fn field_name(&self) -> String;
    fn is_required(&self) -> bool;
    fn default_value(&self) -> Option<String>;
}

impl InnerFieldInfo for RawArgument {
    fn field_name(&self) -> String { self.names.first().cloned().unwrap_or_default() }
    fn is_required(&self) -> bool { self.required }
    fn default_value(&self) -> Option<String> {
        self.default.as_ref().map(|v| v.to_string()).or_else(|| self.default_hint.clone())
    }
}

impl InnerFieldInfo for RawResult {
    fn field_name(&self) -> String { self.key_name.clone() }
    fn is_required(&self) -> bool { !self.optional }
    fn default_value(&self) -> Option<String> {
        None // Results don't have default values
    }
}

/// Determine TypeKind from type string and inner elements
/// Unified function that works for both arguments and results
fn determine_type_kind<T: HasTypeAndInner>(bc_type: &str, inner: &[T]) -> TypeKind {
    match bc_type {
        "array" => {
            if inner.is_empty() {
                TypeKind::Array
            } else {
                // Check if inner elements are objects (have type "object" and their own inner elements)
                // If inner elements are primitives, it's still an Array
                // If inner elements are objects, it's an Object (array of objects pattern)
                let is_array_of_objects = inner.iter().any(|item| item.has_object_inner());
                if is_array_of_objects {
                    TypeKind::Object
                } else {
                    TypeKind::Array
                }
            }
        }
        "object" => TypeKind::Object,
        // All other types (hex, string, number, boolean, amount, etc.) are primitives
        _ => TypeKind::Primitive,
    }
}

/// Helper to build fields from inner argument elements
fn build_fields_from_inner<F>(inner: &[RawArgument], field_builder: F) -> Vec<FieldDef>
where
    F: Fn(&RawArgument) -> FieldDef,
{
    let mut fields: Vec<FieldDef> = inner.iter().map(field_builder).collect();
    fields.sort_by(|a, b| a.name.cmp(&b.name));
    fields
}

/// Generate a field name from a description, using common patterns
fn generate_field_name_from_description(description: &str, index: usize) -> String {
    let desc_lower = description.to_lowercase();
    if desc_lower.contains("hash") {
        format!("hash_{}", index)
    } else if desc_lower.contains("address") {
        format!("address_{}", index)
    } else if desc_lower.contains("tx") || desc_lower.contains("transaction") {
        format!("transaction_{}", index)
    } else if desc_lower.contains("block") {
        format!("block_{}", index)
    } else {
        format!("field_{}", index)
    }
}

/// Helper to build array-of-objects wrapper structure
fn build_array_of_objects_wrapper(inner_fields: Vec<FieldDef>) -> Vec<FieldDef> {
    let object_type = TypeDef {
        name: "object".to_string(),
        description: String::new(),
        kind: TypeKind::Object,
        fields: Some(inner_fields),
        variants: None,
        base_type: None,
        protocol_type: Some("object".to_string()),
        canonical_name: None,
        condition: None,
    };

    vec![FieldDef {
        name: "field".to_string(),
        field_type: object_type,
        required: true,
        description: String::new(),
        default_value: None,
    }]
}

/// Determine if method requires private key access
/// Wallet methods that involve signing or key operations typically require private keys
fn determine_requires_private_keys(category: &str, method_name: &str) -> bool {
    // Only wallet category methods can require private keys
    if category.to_lowercase() != "wallet" {
        return false;
    }

    let name_lower = method_name.to_lowercase();

    // Methods that clearly require private keys (signing, key management)
    name_lower.contains("sign")
        || name_lower.contains("privkey")
        || name_lower == "dumpprivkey"
        || name_lower == "importprivkey"
        || name_lower == "walletpassphrase"
        || name_lower == "walletlock"
        || name_lower == "encryptwallet"
        || name_lower == "walletpassphrasechange"
        || name_lower.starts_with("sign")
        || name_lower.starts_with("dump")
        || name_lower.starts_with("import")
}

/// Format: "major.minor" or "major.minor.build" -> major * 10000 + minor * 100 + build
/// Examples: "30.2" -> 300200, "30.2.1" -> 300201, "0.17" -> 1700
pub fn calculate_version_order(version: &str) -> u32 {
    let (major, minor, patch) = parse_version_components(version);
    major * 10000 + minor * 100 + patch
}

/// Extract version string from RawSchema
///
/// Tries to extract version from:
/// 1. `version` field (removes 'v' prefix if present)
/// 2. `version_major` and `version_minor` fields (formats as "major.minor")
///
/// Returns an error if no version information is found.
pub fn extract_version_from_schema(schema: &RawSchema) -> Result<String, String> {
    if let Some(ref schema_version) = schema.version {
        Ok(schema_version.trim_start_matches('v').to_string())
    } else if let (Some(major), Some(minor)) = (schema.version_major, schema.version_minor) {
        Ok(format!("{}.{}", major, minor))
    } else {
        Err("Could not extract version from schema. Schema must have 'version' or 'version_major'/'version_minor' fields.".to_string())
    }
}

/// Load a lookup map of method name -> earliest version it was added
/// Reads version information from the canonical IR file and creates a HashMap for efficient lookup
pub fn load_method_version_map() -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let project_root = find_project_root()?;
    let ir_file_path = project_root.join("resources/ir/bitcoin.ir.json");

    // Load the canonical IR file
    let ir = ProtocolIR::from_file(&ir_file_path)?;

    let mut method_to_version: HashMap<String, String> = HashMap::new();

    // Extract version_added from each RPC method in the canonical IR
    for rpc in ir.get_rpc_methods() {
        if let Some(ref version_added) = rpc.version_added {
            // Only record if we haven't seen this method yet, or if this version is earlier
            method_to_version
                .entry(rpc.name.clone())
                .and_modify(|existing_version| {
                    // If we already have a version, keep the earlier one
                    let existing_order = calculate_version_order(existing_version);
                    let new_order = calculate_version_order(version_added);
                    if new_order < existing_order {
                        *existing_version = version_added.clone();
                    }
                })
                .or_insert_with(|| version_added.clone());
        }
    }

    Ok(method_to_version)
}

/// Look up the version when a method was first added from the canonical IR
///
/// This queries the version map loaded from the canonical IR file to find when
/// a method was first introduced. Returns None if the method is not found or
/// if the version map cannot be loaded.
fn get_method_version_added(method_name: &str) -> Option<String> {
    match load_method_version_map() {
        Ok(version_map) => version_map.get(method_name).cloned(),
        Err(_) => None, // If we can't load version map, return None
    }
}

/// Recursively normalize field ordering in a TypeDef to match schema conversion output
///
/// This ensures that fields are sorted by name at all levels, matching the behavior
/// of convert_to_protocol_ir_with_version which sorts fields during conversion.
fn normalize_type_def_fields(type_def: &mut TypeDef) {
    if let Some(ref mut fields) = type_def.fields {
        // Sort fields by name
        fields.sort_by(|a, b| a.name.cmp(&b.name));
        // Recursively normalize nested fields
        for field in fields.iter_mut() {
            normalize_type_def_fields(&mut field.field_type);
        }
    }
}

/// Normalize field ordering in an RPC method definition
///
/// This ensures that all fields (in params and results) are sorted consistently.
fn normalize_rpc_method_fields(rpc: &mut RpcDef) {
    // Normalize param fields
    for param in rpc.params.iter_mut() {
        normalize_type_def_fields(&mut param.param_type);
    }
    // Normalize result fields
    if let Some(ref mut result) = rpc.result {
        normalize_type_def_fields(result);
    }
}

/// Sort protocol definitions by RPC method name for deterministic output
///
/// This ensures consistent ordering of definitions across different code paths.
/// Non-RPC definitions maintain their relative order.
fn sort_definitions_by_name(definitions: &mut Vec<ProtocolDef>) {
    definitions.sort_by(|a, b| {
        let name_a = match a {
            ProtocolDef::RpcMethod(ref rpc) => &rpc.name,
            _ => return std::cmp::Ordering::Equal,
        };
        let name_b = match b {
            ProtocolDef::RpcMethod(ref rpc) => &rpc.name,
            _ => return std::cmp::Ordering::Equal,
        };
        name_a.cmp(name_b)
    });
}

/// Extract version-specific IR from canonical IR
///
/// Filters the canonical IR to only include methods available in the target version.
/// Methods with `version_added = None` (unreleased) are excluded.
/// Methods with `version_added > target_version` are excluded (added after target version).
///
/// The canonical IR contains methods from all versions with their `version_added` metadata.
/// This function filters to only include methods where version_added <= target_version.
pub fn extract_version_ir(canonical_ir: ProtocolIR, target_version: &str) -> ProtocolIR {
    let target_order = calculate_version_order(target_version);
    let mut definitions = Vec::new();

    for module in canonical_ir.modules() {
        for def in module.definitions() {
            match def {
                ProtocolDef::RpcMethod(rpc) => {
                    // Determine if method was available in target version
                    let was_available = if let Some(ref v) = rpc.version_added {
                        let added_order = calculate_version_order(v);
                        if added_order <= target_order {
                            // version_added <= target_version, so method was available
                            true
                        } else {
                            // version_added > target_version means method was added after target version
                            // This is expected - just exclude it from the version-specific IR
                            false
                        }
                    } else {
                        // version_added is None means unreleased - exclude from version-specific IR
                        false
                    };

                    // Check if method was removed before target version
                    let not_removed = if let Some(ref v) = rpc.version_removed {
                        calculate_version_order(v) > target_order
                    } else {
                        true
                    };

                    if was_available && not_removed {
                        let mut rpc_clone = rpc.clone();
                        // Normalize field ordering to match schema conversion output
                        normalize_rpc_method_fields(&mut rpc_clone);
                        definitions.push(ProtocolDef::RpcMethod(rpc_clone));
                    }
                }
                other => definitions.push(other.clone()),
            }
        }
    }

    // Sort definitions by method name for deterministic output
    sort_definitions_by_name(&mut definitions);

    ProtocolIR::new(vec![ProtocolModule::new(
        "rpc".to_string(),
        "Bitcoin RPC API".to_string(),
        definitions,
    )])
}

/// Load and parse a RawSchema from a file path
pub fn load_raw_schema(path: &PathBuf) -> Result<RawSchema, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let schema: RawSchema = serde_json::from_str(&content)?;
    Ok(schema)
}

/// Common logic for building TypeDef from type string
fn build_base_type_def(type_str: &str) -> (String, String) {
    let protocol_type = map_protocol_type(type_str);
    let type_name = match type_str {
        "array" => "array".to_string(),
        "object" => "object".to_string(),
        _ => map_protocol_type(type_str),
    };
    (type_name, protocol_type)
}

/// Convert a raw argument to TypeDef
fn convert_argument_to_type_def(raw: &RawArgument) -> TypeDef {
    let (type_name, protocol_type) = build_base_type_def(&raw.r#type);
    let kind = determine_type_kind(&raw.r#type, &raw.inner);

    let mut type_def = TypeDef {
        name: type_name,
        description: raw.description.clone(),
        kind: kind.clone(),
        fields: None,
        variants: None,
        base_type: None,
        protocol_type: Some(protocol_type),
        canonical_name: None,
        condition: None, // Arguments don't have conditions
    };

    // Handle nested structures
    if matches!(kind, TypeKind::Object) && !raw.inner.is_empty() {
        let fields = build_fields_from_inner(&raw.inner, |inner| FieldDef {
            name: inner.field_name(),
            field_type: convert_argument_to_type_def(inner),
            required: inner.is_required(),
            description: inner.description.clone(),
            default_value: inner.default_value(),
        });

        type_def.fields = Some(if raw.r#type == "array" {
            build_array_of_objects_wrapper(fields)
        } else {
            fields
        });
    }

    type_def
}

/// Convert a raw result to TypeDef
fn convert_result(raw: &RawResult) -> TypeDef {
    let (type_name, protocol_type) = build_base_type_def(&raw.r#type);
    let kind = determine_type_kind(&raw.r#type, &raw.inner);

    let mut type_def = TypeDef {
        name: type_name,
        description: raw.description.clone(),
        kind: kind.clone(),
        fields: None,
        variants: None,
        base_type: None,
        protocol_type: Some(protocol_type),
        canonical_name: None,
        condition: if raw.condition.is_empty() { None } else { Some(raw.condition.clone()) },
    };

    // Handle nested structures
    if matches!(kind, TypeKind::Object) && !raw.inner.is_empty() {
        let mut fields: Vec<FieldDef> = raw
            .inner
            .iter()
            .map(|inner| FieldDef {
                name: inner.field_name(),
                field_type: convert_result(inner),
                required: inner.is_required(),
                description: inner.description.clone(),
                default_value: inner.default_value(),
            })
            .collect();
        fields.sort_by(|a, b| a.name.cmp(&b.name));

        type_def.fields = Some(if raw.r#type == "array" {
            build_array_of_objects_wrapper(fields)
        } else {
            fields
        });
    }

    type_def
}

/// Merge multiple results into a single object TypeDef
fn merge_results_to_object(results: &[RawResult]) -> TypeDef {
    let mut fields: Vec<FieldDef> = Vec::new();
    let mut field_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Helper to ensure unique field names
    let mut ensure_unique_name = |base_name: String| -> String {
        let base_name_clone = base_name.clone();
        let mut name = base_name;
        let mut counter = 0;
        while field_names.contains(&name) {
            counter += 1;
            name = format!("{}_{}", base_name_clone, counter);
        }
        field_names.insert(name.clone());
        name
    };

    // Check if we have any object results with inner fields
    let has_object_with_inner = results.iter().any(|r| r.r#type == "object" && !r.inner.is_empty());

    // Check if we have simple type results (hex/string) that might conflict with object results
    let has_simple_type_result = results
        .iter()
        .any(|r| (r.r#type == "hex" || r.r#type == "string") && !r.key_name.is_empty());

    // Track if we're dealing with conditional results (different return types based on conditions)
    // This happens when we have both simple type results and object results
    let has_conditional_results = has_object_with_inner && has_simple_type_result;

    // First, collect field names from object results to check for duplicates
    let mut object_field_names: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    if has_object_with_inner {
        for result in results.iter() {
            if result.r#type == "object" && !result.inner.is_empty() {
                for inner in &result.inner {
                    if !inner.key_name.is_empty() {
                        object_field_names.insert(inner.key_name.clone());
                    }
                }
            }
        }
    }

    for (idx, result) in results.iter().enumerate() {
        // If this result is an object with inner fields, expand those inner fields
        // instead of creating a single field for the object
        if result.r#type == "object" && !result.inner.is_empty() {
            // Expand inner fields directly into the parent object
            for inner in &result.inner {
                let base_field_name = if !inner.key_name.is_empty() {
                    inner.key_name.clone()
                } else {
                    generate_field_name_from_description(&inner.description, fields.len())
                };

                let field_name = ensure_unique_name(base_field_name);

                // If we have conditional results (simple type + object), make all fields optional
                // because the response type depends on the condition (e.g., verbose parameter)
                let is_required = if has_conditional_results {
                    false // Make optional when we have conditional results
                } else {
                    !inner.optional
                };

                fields.push(FieldDef {
                    name: field_name,
                    field_type: convert_result(inner),
                    required: is_required,
                    description: inner.description.clone(),
                    default_value: None,
                });
            }
        } else {
            // For non-object results or objects without inner fields
            // Skip simple type results (hex/string) if we have object results with inner fields,
            // as the object result likely contains the same information plus more
            if has_object_with_inner && (result.r#type == "hex" || result.r#type == "string") {
                // Check if this field name is already present in the object result
                let simple_field_name = if !result.key_name.is_empty() {
                    result.key_name.clone()
                } else {
                    continue; // Skip if no key_name
                };

                // If the field is already in the object result, skip this simple type result
                // (the object version is more complete)
                if object_field_names.contains(&simple_field_name) {
                    continue;
                }
            }

            // Create a single field for this result
            let base_field_name = if !result.key_name.is_empty() {
                result.key_name.clone()
            } else {
                // Special case: if description contains "address" (plural), use "addresses"
                let desc_lower = result.description.to_lowercase();
                if desc_lower.contains("address") && !desc_lower.contains("address_") {
                    "addresses".to_string()
                } else {
                    generate_field_name_from_description(&result.description, idx)
                }
            };

            let field_name = ensure_unique_name(base_field_name);

            fields.push(FieldDef {
                name: field_name,
                field_type: convert_result(result),
                required: !result.optional,
                description: result.description.clone(),
                default_value: None,
            });
        }
    }

    // Sort fields by name for deterministic output
    fields.sort_by(|a, b| a.name.cmp(&b.name));

    TypeDef {
        name: "object".to_string(),
        description: String::new(),
        kind: TypeKind::Object,
        fields: Some(fields),
        variants: None,
        base_type: None,
        protocol_type: Some("object".to_string()),
        canonical_name: None,
        condition: None,
    }
}

/// Convert a raw argument to ParamDef
fn convert_argument(raw: RawArgument) -> ParamDef {
    let param_name = raw.names.first().cloned().unwrap_or_default();

    ParamDef {
        name: param_name.clone(),
        param_type: convert_argument_to_type_def(&raw),
        required: raw.required,
        description: raw.description,
        default_value: raw.default.map(|v| v.to_string()).or_else(|| raw.default_hint),
    }
}

/// Convert a raw RPC method to RpcDef
///
/// Note: version_added should be determined by the caller to avoid redundant lookups.
fn convert_rpc_method(raw: RawRpcMethod, version_added: Option<String>) -> RpcDef {
    // Convert arguments, preserving original order (RPC argument order is significant)
    let params: Vec<ParamDef> = raw.arguments.into_iter().map(convert_argument).collect();

    let result = if raw.results.is_empty() {
        None
    } else if raw.results.len() == 1 {
        Some(convert_result(&raw.results[0]))
    } else {
        // Multiple results - merge into object
        Some(merge_results_to_object(&raw.results))
    };

    // Determine access level based on category and method name
    let category = raw.category.clone();
    let access_level = method_categorization::access_level_for(&category, &raw.name);

    // Determine if method requires private keys (wallet methods that involve signing)
    let requires_private_keys = determine_requires_private_keys(&category, &raw.name);

    RpcDef {
        name: raw.name,
        description: raw.description,
        params,
        result,
        category: raw.category,
        access_level,
        requires_private_keys,
        version_added,
        version_removed: None, // None means still supported
        examples: if raw.examples.is_empty() { None } else { Some(raw.examples) },
        hidden: if category.to_lowercase() == "hidden" { Some(true) } else { None },
    }
}

/// Convert raw Bitcoin Core schema to ProtocolIR with optional version
pub fn convert_to_protocol_ir_with_version(raw: RawSchema, version: Option<String>) -> ProtocolIR {
    use ir::{ProtocolDef, ProtocolModule};

    let mut definitions = Vec::new();

    for (_, rpc) in raw.rpcs {
        // Determine version_added based on available information
        // Check the version map first to see if method existed in earlier version
        let version_added = {
            let version_from_map = get_method_version_added(&rpc.name);
            if let Some(version) = version_from_map {
                // Method found in version map, use that version
                Some(version)
            } else if let Some(ref current_version) = version {
                // Method not in version map, but we have a current version - use it
                Some(current_version.clone())
            } else {
                // Method not in version map and no current version provided - cannot determine
                None
            }
        };

        let rpc_def = convert_rpc_method(rpc, version_added);
        definitions.push(ProtocolDef::RpcMethod(rpc_def));
    }

    // Sort definitions by method name for deterministic output
    sort_definitions_by_name(&mut definitions);

    // Create module using the public constructor
    let module = ProtocolModule::new("rpc".to_string(), "Bitcoin RPC API".to_string(), definitions);

    ProtocolIR::new(vec![module])
}

/// Main function for process_bitcoin_schema binary
///
/// Usage patterns:
/// 1. Convert schema to IR: process_bitcoin_schema <schema_file> [output_file]
/// 2. Extract version-specific IR: process_bitcoin_schema <version> [output_file]
#[allow(dead_code)] // False positive: main is used as binary entry point
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage:");
        eprintln!(
            "  {} <schema_file> [output_file]                    # Convert schema to IR",
            args[0]
        );
        eprintln!("  {} <version> [output_file]                         # Extract version-specific IR from canonical IR", args[0]);
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  {} schema.json output.ir.json", args[0]);
        eprintln!("  {} 30.2", args[0]);
        eprintln!("  {} 30.2 v30_2_0_bitcoin.ir.json", args[0]);
        std::process::exit(1);
    }

    let first_arg = &args[1];

    // Check if first argument is a version string (contains only digits and dots, or starts with 'v')
    let is_version =
        first_arg.trim_start_matches('v').chars().all(|c| c.is_ascii_digit() || c == '.');

    if is_version {
        // Mode 3: Extract version-specific IR from canonical IR
        let version = first_arg.trim_start_matches('v');
        let project_root = find_project_root()?;
        let canonical_ir_path = project_root.join("resources/ir/bitcoin.ir.json");

        // Load canonical IR
        let canonical_ir = ProtocolIR::from_file(&canonical_ir_path)?;

        // Extract version-specific IR
        let version_ir = extract_version_ir(canonical_ir, version);

        // Determine output file
        let output_file = if args.len() >= 3 {
            PathBuf::from(&args[2])
        } else {
            // Auto-generate filename using normalized version format
            let ir_dir = get_ir_dir()?;
            ir_dir.join(version_ir_filename(version, "bitcoin"))
        };

        // Save version-specific IR
        version_ir.to_file(&output_file)?;
        println!("✓ Extracted version-specific IR: {}", output_file.display());
    } else {
        // Mode 1: Convert schema to IR
        let schema_file = PathBuf::from(first_arg);
        let raw_schema = load_raw_schema(&schema_file)?;

        // Extract version from schema
        let version = extract_version_from_schema(&raw_schema)?;

        // Convert to ProtocolIR
        let protocol_ir = convert_to_protocol_ir_with_version(raw_schema, Some(version.clone()));

        // Determine output file
        let output_file = if args.len() >= 3 {
            PathBuf::from(&args[2])
        } else {
            // Auto-generate filename using normalized version format
            let ir_dir = get_ir_dir()?;
            ir_dir.join(version_ir_filename(&version, "bitcoin"))
        };

        // Save IR
        protocol_ir.to_file(&output_file)?;
        println!("✓ Converted schema to IR: {}", output_file.display());
        println!("  Version: {}", version);
    }

    Ok(())
}
