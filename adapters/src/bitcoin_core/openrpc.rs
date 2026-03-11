// SPDX-License-Identifier: CC0-1.0

//! Bitcoin Core OpenRPC helpers (versioning and IR extraction).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ir::{
    FieldDef, FieldKey, ParamDef, ProtocolDef, ProtocolIR, ProtocolModule, RpcDef, TypeDef,
    TypeKind,
};
use path::{
    canonical_bitcoin_ir_path, find_project_root, get_ir_dir, resolve_ir_output_path,
    version_ir_filename,
};
use semantics::method_categorization;
use serde::Deserialize;
use types::ProtocolVersion;

use crate::conversion_helpers::{determine_requires_private_keys, sort_definitions_by_name};

/// RPCs whose wire result is a top-level JSON array.
///
/// For these methods, Bitcoin Core's OpenRPC metadata models the result as an
/// array via `x-bitcoin-results`. The Ethos IR mirrors that behavior by
/// emitting a `TypeDef` with `kind: Array` (see `TOP_LEVEL_ARRAY_METHODS`
/// handling in `convert_openrpc_method`) instead of wrapping the result in an
/// artificial object with a single field.
pub const TOP_LEVEL_ARRAY_METHODS: &[&str] = &[
    "deriveaddresses",
    "getaddednodeinfo",
    "getnodeaddresses",
    "getorphantxs",
    "getpeerinfo",
    "getrawmempool",
    "listbanned",
    "listlockunspent",
    "listreceivedbyaddress",
    "listreceivedbylabel",
    "listtransactions",
    "listunspent",
];

/// OpenRPC document produced by Bitcoin Core's `getopenrpcinfo`
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct OpenRpcDoc {
    #[serde(rename = "openrpc")]
    /// The OpenRPC specification version.
    pub(crate) open_rpc: String,
    /// The OpenRPC info object.
    pub(crate) info: OpenRpcInfo,
    /// The OpenRPC methods.
    pub(crate) methods: Vec<OpenRpcMethod>,
}

/// OpenRPC `info` object with Bitcoin-specific extensions
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct OpenRpcInfo {
    #[serde(default)]
    /// The title of the OpenRPC document.
    pub title: Option<String>,
    #[serde(default)]
    /// The version of the OpenRPC document.
    pub version: Option<String>,
    #[serde(default)]
    /// The description of the OpenRPC document.
    pub description: Option<String>,
    #[serde(rename = "x-bitcoin-version-full")]
    #[serde(default)]
    /// The full version of the OpenRPC document.
    pub x_bitcoin_version_full: Option<String>,
    #[serde(rename = "x-bitcoin-version-major")]
    #[serde(default)]
    /// The major version of the OpenRPC document.
    pub x_bitcoin_version_major: Option<u32>,
    #[serde(rename = "x-bitcoin-version-minor")]
    #[serde(default)]
    /// The minor version of the OpenRPC document.
    pub x_bitcoin_version_minor: Option<u32>,
    #[serde(rename = "x-bitcoin-version-build")]
    #[serde(default)]
    /// The build version of the OpenRPC document.
    pub x_bitcoin_version_build: Option<u32>,
    #[serde(rename = "x-bitcoin-timestamp-ms")]
    #[serde(default)]
    /// The timestamp of the OpenRPC document.
    pub x_bitcoin_timestamp_ms: Option<i64>,
}

/// OpenRPC method object with Bitcoin-specific extensions
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct OpenRpcMethod {
    /// The name of the OpenRPC method.
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    /// The description of the OpenRPC method.
    pub description: String,
    #[serde(default)]
    /// The parameters of the OpenRPC method.
    pub params: Vec<serde_json::Value>,
    #[serde(default)]
    /// The result of the OpenRPC method.
    pub result: Option<OpenRpcResult>,
    #[serde(rename = "x-bitcoin-category")]
    /// The category of the OpenRPC method.
    pub x_bitcoin_category: String,
    #[serde(rename = "x-bitcoin-examples")]
    #[serde(default)]
    /// The examples of the OpenRPC method.
    pub x_bitcoin_examples: Option<String>,
    #[serde(rename = "x-bitcoin-argument-names")]
    #[serde(default)]
    /// The argument names of the OpenRPC method.
    pub x_bitcoin_argument_names: Vec<String>,
    #[serde(rename = "x-bitcoin-arguments")]
    #[serde(default)]
    /// Arguments in the Bitcoin Core OpenRPC shape
    pub x_bitcoin_arguments: Vec<RawArgument>,
}

/// OpenRPC result wrapper that carries the legacy Bitcoin Core results
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct OpenRpcResult {
    #[serde(default)]
    /// The name of the OpenRPC result.
    pub name: Option<String>,
    #[serde(default)]
    /// The schema of the OpenRPC result.
    pub schema: Option<serde_json::Value>,
    #[serde(rename = "x-bitcoin-results")]
    #[serde(default)]
    /// The results of the OpenRPC result.
    pub x_bitcoin_results: Vec<RawResult>,
}

/// Raw argument from Bitcoin Core (OpenRPC x-bitcoin-arguments shape)
#[derive(Debug, Clone, Deserialize)]
pub struct RawArgument {
    /// List of possible names for this argument
    pub names: Vec<String>,
    /// The description of the argument.
    pub description: String,
    /// The one-line description of the argument.
    #[serde(default)]
    pub oneline_description: String,
    /// Whether this argument can also be passed positionally.
    #[serde(default)]
    pub also_positional: bool,
    /// The type string representations.
    #[serde(default)]
    pub type_str: Vec<String>,
    /// Whether this argument is required.
    pub required: bool,
    /// The default value for the argument, if any.
    #[serde(default)]
    pub default: Option<serde_json::Value>,
    /// The hint string for the default value.
    #[serde(default)]
    pub default_hint: Option<String>,
    /// Whether this argument is hidden from documentation.
    #[serde(default)]
    pub hidden: bool,
    /// The type of the argument (e.g., "string", "number", "object", "array").
    pub r#type: String,
    /// The nested inner arguments for complex types.
    #[serde(default)]
    pub inner: Vec<RawArgument>,
}

/// Raw result from Bitcoin Core
#[derive(Debug, Clone, Deserialize)]
pub struct RawResult {
    /// The type of the result (e.g., "string", "number", "object", "array").
    pub r#type: String,
    /// Whether this result field is optional.
    #[serde(default)]
    pub optional: bool,
    /// The description of what this result represents.
    pub description: String,
    /// Whether to skip type checking for this result.
    #[serde(default)]
    pub skip_type_check: bool,
    /// The name of the key for this result field.
    pub key_name: String,
    /// The condition under which this result is present.
    #[serde(default)]
    pub condition: String,
    /// The nested inner results for complex types.
    #[serde(default)]
    pub inner: Vec<RawResult>,
}

/// Maps a Bitcoin Core type to a protocol type.
///
/// This unified function handles both argument and result types.
fn map_protocol_type(bc_type: &str) -> String {
    match bc_type {
        "amount" => "amount".to_string(),
        "any" => "any".to_string(),
        "array" => "array".to_string(),
        "boolean" => "boolean".to_string(),
        "elision" => "elision".to_string(),
        "hex" => "hex".to_string(),
        "none" => "none".to_string(),
        "number" => "number".to_string(),
        "object" => "object".to_string(),
        "range" => "range".to_string(),
        "string" => "string".to_string(),
        "timestamp" => "timestamp".to_string(),
        // Exhaustive: unmapped types must be added explicitly above.
        unknown => panic!(
            "Unmapped Bitcoin Core type '{}'. Please add explicit handling for this type in map_protocol_type().",
            unknown
        ),
    }
}

/// Describes types that have a type string and inner elements.
trait HasTypeAndInner {
    fn has_object_inner(&self) -> bool;
}

impl HasTypeAndInner for RawArgument {
    fn has_object_inner(&self) -> bool { self.r#type == "object" && !self.inner.is_empty() }
}

impl HasTypeAndInner for RawResult {
    fn has_object_inner(&self) -> bool { self.r#type == "object" && !self.inner.is_empty() }
}

/// Extracts field information from inner elements.
trait InnerFieldInfo {
    /// The name of the field.
    fn field_name(&self) -> String;
    /// Whether the field is required.
    fn is_required(&self) -> bool;
    /// The default value of the field.
    fn default_value(&self) -> Option<String>;
}

impl InnerFieldInfo for RawArgument {
    /// The name of the field.
    fn field_name(&self) -> String { self.names.first().cloned().unwrap_or_default() }
    /// Whether the field is required.
    fn is_required(&self) -> bool { self.required }
    /// The default value of the field.
    fn default_value(&self) -> Option<String> {
        self.default.as_ref().map(|v| v.to_string()).or_else(|| self.default_hint.clone())
    }
}

impl InnerFieldInfo for RawResult {
    /// The name of the field.
    fn field_name(&self) -> String { self.key_name.clone() }
    /// Whether the field is required.
    fn is_required(&self) -> bool { !self.optional }
    /// The default value of the field.
    fn default_value(&self) -> Option<String> {
        None // Results don't have default values
    }
}

/// Determines the `TypeKind` from a type string and inner elements.
/// This unified function works for both arguments and results.
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
        // All other types (amount, boolean, hex, number, object, string, etc.) are primitives
        _ => TypeKind::Primitive,
    }
}

/// Builds fields from inner argument elements.
fn build_fields_from_inner<F>(inner: &[RawArgument], field_builder: F) -> Vec<FieldDef>
where
    F: Fn(&RawArgument) -> FieldDef,
{
    inner.iter().map(field_builder).collect()
}

/// Builds an array-of-objects wrapper structure.
fn build_array_of_objects_wrapper(inner_fields: Vec<FieldDef>) -> Vec<FieldDef> {
    let object_type = TypeDef {
        name: "object".to_string(),
        kind: TypeKind::Object,
        fields: Some(inner_fields),
        protocol_type: Some("object".to_string()),
        ..Default::default()
    };

    vec![FieldDef {
        key: FieldKey::Named("field".to_string()),
        field_type: object_type,
        required: true,
        description: String::new(),
        default_value: None,
        version_added: None,
        version_removed: None,
    }]
}

/// Strips the build/metadata suffix from a version string (e.g. "30.99.0-705399b1d57a" -> "30.99.0").
fn strip_version_suffix(version: &str) -> String {
    let trimmed = version.trim_start_matches('v').trim();
    let end = trimmed.find(|c: char| c == '-' || c == '+').unwrap_or(trimmed.len());
    trimmed[..end].to_string()
}

/// Returns true if the version string contains a build/metadata suffix (e.g. "-dirty", "-rc1", "+meta").
fn has_version_suffix(version: &str) -> bool {
    let trimmed = version.trim_start_matches('v').trim();
    trimmed.chars().any(|c| c == '-' || c == '+')
}

/// Returns true if a param/field with the given version_added and version_removed is visible
/// when generating for target_version. Used by codegen to filter params and result fields.
/// `None` means visible (no version metadata / assumed available in all versions).
pub fn item_visible_for_version(
    version_added: Option<&str>,
    version_removed: Option<&str>,
    target_version: &str,
) -> bool {
    let target_major = parse_version_for_ordering(target_version).major();
    if let Some(added) = version_added {
        if effective_major_for_comparison(added) > target_major {
            return false;
        }
    }
    if let Some(removed) = version_removed {
        if effective_major_for_comparison(removed) <= target_major {
            return false;
        }
    }
    true
}

/// Filters a `TypeDef`'s fields (and nested types) by version_added/version_removed for the
/// given target version.
pub fn filter_type_def_for_version(ty: &ir::TypeDef, target: &str) -> ir::TypeDef {
    let mut out = ty.clone();
    if let Some(fields) = &out.fields {
        let filtered: Vec<FieldDef> = fields
            .iter()
            .filter(|f| {
                item_visible_for_version(
                    f.version_added.as_deref(),
                    f.version_removed.as_deref(),
                    target,
                )
            })
            .map(|f| {
                let mut field = f.clone();
                field.field_type = filter_type_def_for_version(&f.field_type, target);
                field
            })
            .collect();
        out.fields = Some(filtered);
    }
    out
}

/// Filters RPC params (and their nested types) by version_added/version_removed for the
/// given target version.
pub fn filter_params_for_version(params: &[ir::ParamDef], target: &str) -> Vec<ir::ParamDef> {
    params
        .iter()
        .filter(|p| {
            item_visible_for_version(
                p.version_added.as_deref(),
                p.version_removed.as_deref(),
                target,
            )
        })
        .map(|p| {
            let mut param = p.clone();
            param.param_type = filter_type_def_for_version(&p.param_type, target);
            param
        })
        .collect()
}

/// Computes the effective major version for inclusion comparison. We only use the major version: when building 30.2.8,
/// include methods whose version_added is in major 30 or earlier. Unreleased (30.99.x or with a build suffix such as "-dirty")
/// is treated as next major (31) so it is excluded when targeting 30. Supports "30" (major-only) and "30.2.8".
pub fn effective_major_for_comparison(version: &str) -> u32 {
    let stripped = strip_version_suffix(version);
    if let Ok(pv) = ProtocolVersion::from_string(&stripped) {
        let unreleased = pv.minor == 99 || has_version_suffix(version);
        return if unreleased { pv.major.saturating_add(1) } else { pv.major };
    }
    stripped.trim().parse::<u32>().unwrap_or(u32::MAX)
}

/// Normalizes version_added for storage in IR: one or two numbers (e.g. 17, 28, 30, or 0.17).
/// Unreleased (30.99.x or with a build suffix such as "-dirty") becomes the next major (31).
pub(super) fn normalize_version_added_for_storage(version: &str) -> String {
    let stripped = strip_version_suffix(version);
    if let Ok(pv) = ProtocolVersion::from_string(&stripped) {
        let unreleased = pv.minor == 99 || has_version_suffix(version);
        if unreleased {
            return format!("{}", pv.major.saturating_add(1));
        }
        if pv.major == 0 {
            return format!("0.{}", pv.minor);
        }
        return format!("{}", pv.major);
    }
    stripped.trim().to_string()
}

/// Parses a version string for comparison. Uses the shared `ProtocolVersion` (major.minor.patch).
/// Only used for target versions (releases); unparseable values are treated as 0.0.0.
fn parse_version_for_ordering(version: &str) -> ProtocolVersion {
    let normalized = strip_version_suffix(version);
    ProtocolVersion::from_string(&normalized).unwrap_or_default()
}

/// Extracts the version string from an OpenRPC document.
///
/// Tries to extract version from:
/// 1. `info.x-bitcoin-version-full` or `info.version` (removes 'v' prefix if present)
/// 2. `info.x-bitcoin-version-major` and `info.x-bitcoin-version-minor` (formats as "major.minor")
///
/// Returns an error if no version information is found.
pub fn extract_version_from_openrpc(doc: &OpenRpcDoc) -> Result<String, String> {
    if let Some(ref v) =
        doc.info.x_bitcoin_version_full.clone().or_else(|| doc.info.version.clone())
    {
        return Ok(v.trim_start_matches('v').to_string());
    }
    if let (Some(major), Some(minor)) =
        (doc.info.x_bitcoin_version_major, doc.info.x_bitcoin_version_minor)
    {
        return Ok(format!("{}.{}", major, minor));
    }
    Err("Could not extract version from OpenRPC document. Info must have 'version' or 'x-bitcoin-version-full', or 'x-bitcoin-version-major'/'x-bitcoin-version-minor'.".to_string())
}

/// Loads IR from a path and builds a method name -> version_added map from it.
pub fn load_ir_and_version_map_from_path(
    ir_file_path: &std::path::Path,
) -> Result<(ProtocolIR, HashMap<String, String>), Box<dyn std::error::Error>> {
    let ir = ProtocolIR::from_file(ir_file_path)?;
    let mut method_to_version: HashMap<String, String> = HashMap::new();

    // Extract version_added from each RPC method in the canonical IR (keep earlier by major)
    for rpc in ir.get_rpc_methods() {
        if let Some(ref version_added) = rpc.version_added {
            let normalized = normalize_version_added_for_storage(version_added);
            method_to_version
                .entry(rpc.name.clone())
                .and_modify(|existing_version| {
                    let existing_major = effective_major_for_comparison(existing_version);
                    let new_major = effective_major_for_comparison(version_added);
                    if new_major < existing_major {
                        *existing_version = normalized.clone();
                    }
                })
                .or_insert_with(|| normalized);
        }
    }
    Ok((ir, method_to_version))
}

/// Loads a lookup map of method name -> earliest version it was added from a specific IR file.
pub fn load_method_version_map_from_path(
    ir_file_path: &std::path::Path,
) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    load_ir_and_version_map_from_path(ir_file_path).map(|(_, m)| m)
}

/// Loads a lookup map of method name -> earliest version it was added.
/// Reads version information from the canonical IR file and creates a `HashMap` for efficient lookup.
pub fn load_method_version_map() -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let project_root = find_project_root()?;
    load_method_version_map_from_path(&canonical_bitcoin_ir_path(&project_root))
}

/// Looks up the version when a method was first added from a preloaded version map.
///
/// Returns None if the method is not in the map. The map is built once from the canonical IR
/// so we never overwrite an existing version_added with the OpenRPC version for methods that
/// were already in the canonical IR.
fn get_method_version_added_from_map(
    method_name: &str,
    version_map: &HashMap<String, String>,
) -> Option<String> {
    version_map.get(method_name).cloned()
}

/// Finds a matching field in `existing_fields` for `our` by name, with index fallback.
fn find_matching_field<'a>(
    our: &FieldDef,
    existing_fields: &'a [ir::FieldDef],
    index: usize,
) -> Option<&'a ir::FieldDef> {
    existing_fields
        .iter()
        .find(|e| e.key.json_key() == our.key.json_key())
        .or_else(|| existing_fields.get(index))
}

/// Copies version_added/version_removed from existing `TypeDef` fields into our `TypeDef` (by field name or index, recursively).
fn merge_version_into_type_def(our: &mut ir::TypeDef, existing: &ir::TypeDef) {
    if let (Some(our_fields), Some(existing_fields)) =
        (our.fields.as_mut(), existing.fields.as_ref())
    {
        for (index, our_f) in our_fields.iter_mut().enumerate() {
            let ex_f = find_matching_field(our_f, existing_fields, index);
            if let Some(ex_f) = ex_f {
                if ex_f.version_added.is_some() {
                    our_f.version_added = ex_f.version_added.clone();
                }
                if ex_f.version_removed.is_some() {
                    our_f.version_removed = ex_f.version_removed.clone();
                }
                merge_version_into_type_def(&mut our_f.field_type, &ex_f.field_type);
            }
        }
    }
}

/// Copies version_added/version_removed from an existing `ParamDef` into our `ParamDef` and recurses into `param_type`.
fn merge_version_into_param(our: &mut ir::ParamDef, existing: &ir::ParamDef) {
    if existing.version_added.is_some() {
        our.version_added = existing.version_added.clone();
    }
    if existing.version_removed.is_some() {
        our.version_removed = existing.version_removed.clone();
    }
    merge_version_into_type_def(&mut our.param_type, &existing.param_type);
}

/// Copies version_removed and nested version fields from an existing `RpcDef` into our `RpcDef` (method-level version_added already set).
fn merge_version_from_existing_rpc(our: &mut ir::RpcDef, existing: &ir::RpcDef) {
    if existing.version_removed.is_some() {
        our.version_removed = existing.version_removed.clone();
    }
    for our_p in our.params.iter_mut() {
        if let Some(ex_p) = existing.params.iter().find(|e| e.name == our_p.name) {
            merge_version_into_param(our_p, ex_p);
        }
    }
    if let (Some(our_res), Some(ex_res)) = (our.result.as_mut(), existing.result.as_ref()) {
        merge_version_into_type_def(our_res, ex_res);
    }
}

/// Extracts version-specific IR from canonical IR.
///
/// Filters the canonical IR to only include methods available in the target version.
/// Comparison uses major version only: building 30.2.8 includes methods present in major 30.
/// Methods with `version_added = None` (unreleased) are excluded. Unreleased (e.g. 30.99-)
/// is treated as next major (31) so excluded when targeting 30.
pub fn extract_version_ir(canonical_ir: ProtocolIR, target_version: &str) -> ProtocolIR {
    let target_major = parse_version_for_ordering(target_version).major();
    let mut definitions = Vec::new();

    for module in canonical_ir.modules() {
        for def in module.definitions() {
            match def {
                ProtocolDef::RpcMethod(rpc) => {
                    // Include only if the method was added in the target major or earlier.
                    let was_available = if let Some(ref v) = rpc.version_added {
                        effective_major_for_comparison(v) <= target_major
                    } else {
                        false
                    };

                    // Exclude methods that were removed on or before the target major.
                    // Use `effective_major_for_comparison` so unreleased removals (e.g. 30.99 or with
                    // build suffixes) are treated as the next major and remain available when
                    // targeting the current major.
                    let not_removed = if let Some(ref v) = rpc.version_removed {
                        effective_major_for_comparison(v) > target_major
                    } else {
                        true
                    };

                    if was_available && not_removed {
                        let mut rpc_for_version = rpc.clone();
                        rpc_for_version.params =
                            filter_params_for_version(&rpc_for_version.params, target_version);
                        if let Some(result) = &rpc_for_version.result {
                            rpc_for_version.result =
                                Some(filter_type_def_for_version(result, target_version));
                        }
                        definitions.push(ProtocolDef::RpcMethod(rpc_for_version));
                    }
                }
                other => definitions.push(other.clone()),
            }
        }
    }

    // Sort definitions by method name for deterministic output.
    sort_definitions_by_name(&mut definitions);

    ProtocolIR::new(vec![ProtocolModule::new(
        "rpc".to_string(),
        "Bitcoin RPC API".to_string(),
        definitions,
    )])
}

/// Loads and parses an OpenRPC document from a file.
fn load_openrpc_doc(path: &PathBuf) -> Result<OpenRpcDoc, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let doc: OpenRpcDoc = serde_json::from_str(&content)?;
    Ok(doc)
}

/// Provides common logic for building a `TypeDef` from a type string.
fn build_base_type_def(type_str: &str) -> (String, String) {
    let protocol_type = map_protocol_type(type_str);
    let type_name = match type_str {
        "array" => "array".to_string(),
        "object" => "object".to_string(),
        _ => map_protocol_type(type_str),
    };
    (type_name, protocol_type)
}

/// Converts a raw argument to a `TypeDef`.
fn convert_argument_to_type_def(raw: &RawArgument) -> TypeDef {
    let (type_name, protocol_type) = build_base_type_def(&raw.r#type);
    let kind = determine_type_kind(&raw.r#type, &raw.inner);

    let mut type_def = TypeDef {
        name: type_name,
        description: raw.description.clone(),
        kind: kind.clone(),
        protocol_type: Some(protocol_type),
        ..Default::default()
    };

    // Handle nested structures
    if matches!(kind, TypeKind::Object) && !raw.inner.is_empty() {
        let fields = build_fields_from_inner(&raw.inner, |inner| FieldDef {
            key: FieldKey::Named(inner.field_name()),
            field_type: convert_argument_to_type_def(inner),
            required: inner.is_required(),
            description: inner.description.clone(),
            default_value: inner.default_value(),
            version_added: None,
            version_removed: None,
        });

        type_def.fields = Some(if raw.r#type == "array" {
            build_array_of_objects_wrapper(fields)
        } else {
            fields
        });
    }

    type_def
}

/// Converts a raw result to a `TypeDef`.
/// `parent_key`: when recursing, the key of the parent field (e.g. "vin") so we can name array element types.
fn convert_result(raw: &RawResult, parent_key: Option<&str>) -> TypeDef {
    let (type_name, protocol_type) = build_base_type_def(&raw.r#type);
    let kind = determine_type_kind(&raw.r#type, &raw.inner);

    let mut type_def = TypeDef {
        name: type_name,
        description: raw.description.clone(),
        kind: kind.clone(),
        protocol_type: Some(protocol_type),
        condition: if raw.condition.is_empty() { None } else { Some(raw.condition.clone()) },
        ..Default::default()
    };

    // Handle nested structures
    if matches!(kind, TypeKind::Object) && !raw.inner.is_empty() {
        let parent = if raw.key_name.is_empty() { parent_key } else { Some(raw.key_name.as_str()) };
        let fields: Vec<FieldDef> = raw
            .inner
            .iter()
            .enumerate()
            .map(|(i, inner)| {
                let name = if inner.key_name.is_empty() {
                    format!("field_{}", i)
                } else {
                    inner.field_name()
                };
                FieldDef {
                    key: FieldKey::Named(name),
                    field_type: convert_result(inner, parent),
                    required: inner.is_required(),
                    description: inner.description.clone(),
                    default_value: inner.default_value(),
                    version_added: None,
                    version_removed: None,
                }
            })
            .collect();

        type_def.fields = Some(if raw.r#type == "array" {
            build_array_of_objects_wrapper(fields)
        } else {
            fields
        });
    }

    // Canonical names for decoded-tx nested types so codegen emits shared types from IR
    if matches!(kind, TypeKind::Object) && !raw.inner.is_empty() {
        match raw.key_name.as_str() {
            "scriptPubKey" => type_def.name = "DecodedScriptPubKey".to_string(),
            "scriptSig" => type_def.name = "DecodedScriptSig".to_string(),
            "prevout" => type_def.name = "DecodedPrevout".to_string(),
            _ => {}
        }
        // Name array element types when we're the element of vin/vout
        if let Some(p) = parent_key {
            match p {
                "vin" => type_def.name = "DecodedVin".to_string(),
                "vout" => type_def.name = "DecodedVout".to_string(),
                _ => {}
            }
        }
    }

    type_def
}

/// Merges multiple results into a single object `TypeDef`.
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
                let key_name = if !inner.key_name.is_empty() {
                    inner.key_name.clone()
                } else {
                    ensure_unique_name(format!("field_{}", idx))
                };
                let name = ensure_unique_name(key_name);

                // If we have conditional results (simple type + object), make all fields optional
                // because the response type depends on the condition (e.g., verbose parameter)
                let is_required = if has_conditional_results {
                    false // Make optional when we have conditional results
                } else {
                    !inner.optional
                };

                let parent =
                    if result.key_name.is_empty() { None } else { Some(result.key_name.as_str()) };
                fields.push(FieldDef {
                    key: FieldKey::Named(name),
                    field_type: convert_result(inner, parent),
                    required: is_required,
                    description: inner.description.clone(),
                    default_value: None,
                    version_added: None,
                    version_removed: None,
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
                    ensure_unique_name(format!("field_{}", idx))
                }
            };

            let name = ensure_unique_name(base_field_name);
            fields.push(FieldDef {
                key: FieldKey::Named(name),
                field_type: convert_result(result, None),
                required: !result.optional,
                description: result.description.clone(),
                default_value: None,
                version_added: None,
                version_removed: None,
            });
        }
    }

    TypeDef {
        name: "object".to_string(),
        kind: TypeKind::Object,
        fields: Some(fields),
        protocol_type: Some("object".to_string()),
        ..Default::default()
    }
}

/// Converts a raw argument to a `ParamDef`.
fn convert_argument(raw: RawArgument) -> ParamDef {
    let param_name = raw.names.first().cloned().unwrap_or_default();

    ParamDef {
        name: param_name.clone(),
        param_type: convert_argument_to_type_def(&raw),
        required: raw.required,
        description: raw.description,
        default_value: raw.default.map(|v| v.to_string()).or_else(|| raw.default_hint),
        version_added: None,
        version_removed: None,
    }
}

/// Converts an OpenRPC method to an `RpcDef`.
///
/// version_added should be determined by the caller to avoid redundant lookups.
fn convert_openrpc_method(method: OpenRpcMethod, version_added: Option<String>) -> RpcDef {
    let arguments = method.x_bitcoin_arguments;
    let results = method.result.map(|r| r.x_bitcoin_results).unwrap_or_default();

    let params: Vec<ParamDef> = arguments.into_iter().map(convert_argument).collect();

    let result = if results.is_empty() {
        None
    } else if TOP_LEVEL_ARRAY_METHODS.contains(&method.name.as_str()) {
        // For top-level array RPCs, treat the whole result as an array in IR.
        // Prefer the first array-typed variant; fall back to the first result.
        let canonical = results.iter().find(|r| r.r#type == "array").unwrap_or(&results[0]);

        // Derive the element type from the canonical array result:
        // - When inner results describe per-element structure, use the first inner entry.
        // - Otherwise fall back to a generic `any` primitive so codegen can still infer `Vec<Value>`.
        let element_type = if canonical.r#type == "array" && !canonical.inner.is_empty() {
            let elem = &canonical.inner[0];
            convert_result(elem, None)
        } else {
            TypeDef {
                name: "any".to_string(),
                description: canonical.description.clone(),
                kind: TypeKind::Primitive,
                protocol_type: Some("any".to_string()),
                ..Default::default()
            }
        };

        // Convention for top-level array results in IR:
        // represent them as `TypeKind::Array` with a single anonymous index-0 field.
        // The version-specific response generator relies on this shape in
        // `array_element_type_from_ir` to detect and emit array wrappers.
        Some(TypeDef {
            name: "array".to_string(),
            description: canonical.description.clone(),
            kind: TypeKind::Array,
            fields: Some(vec![FieldDef {
                key: FieldKey::Named("field_0".to_string()),
                field_type: element_type,
                required: !canonical.optional,
                description: canonical.description.clone(),
                default_value: None,
                version_added: None,
                version_removed: None,
            }]),
            protocol_type: Some("array".to_string()),
            ..Default::default()
        })
    } else if results.len() == 1 {
        Some(convert_result(&results[0], None))
    } else {
        Some(merge_results_to_object(&results))
    };

    let category = method.x_bitcoin_category.clone();
    let access_level = method_categorization::access_level_for(&category, &method.name);
    let requires_private_keys = determine_requires_private_keys(&category, &method.name);

    let examples: Vec<String> = method
        .x_bitcoin_examples
        .as_ref()
        .filter(|s| !s.is_empty())
        .map(|s| vec![s.clone()])
        .unwrap_or_default();

    RpcDef {
        name: method.name,
        description: method.description,
        params,
        result,
        category,
        access_level,
        requires_private_keys,
        version_added,
        version_removed: None,
        examples: if examples.is_empty() { None } else { Some(examples) },
        hidden: if method.x_bitcoin_category.to_lowercase() == "hidden" {
            Some(true)
        } else {
            None
        },
    }
}

/// Converts OpenRPC (Bitcoin Core) to `ProtocolIR` using a preloaded version map.
///
/// Use this when you already have the canonical IR loaded (e.g. for merge) to avoid re-reading.
pub fn convert_to_protocol_ir_with_version_map(
    doc: OpenRpcDoc,
    version: Option<String>,
    version_map: &HashMap<String, String>,
) -> ProtocolIR {
    use ir::{ProtocolDef, ProtocolModule};

    let mut definitions = Vec::new();

    for method in doc.methods {
        let version_added = {
            let version_from_map = get_method_version_added_from_map(&method.name, version_map);
            let raw = version_from_map.or_else(|| version.as_ref().cloned());
            raw.map(|v| normalize_version_added_for_storage(&v))
        };

        let rpc_def = convert_openrpc_method(method, version_added);
        definitions.push(ProtocolDef::RpcMethod(rpc_def));
    }

    // Sort definitions by method name for deterministic output
    sort_definitions_by_name(&mut definitions);

    let module = ProtocolModule::new("rpc".to_string(), "Bitcoin RPC API".to_string(), definitions);

    ProtocolIR::new(vec![module])
}

/// Converts OpenRPC (Bitcoin Core) to `ProtocolIR` with an optional version.
///
/// `version_added` is preserved from the canonical IR (resources/ir/bitcoin.ir.json) when the
/// method already exists there; only methods not in the canonical IR get the current document
/// version. The version map is loaded once so we never incorrectly overwrite an existing
/// version_added (e.g. 30.2) with the OpenRPC version (e.g. 30.99) due to a failed or
/// inconsistent re-read.
pub fn convert_to_protocol_ir_with_version(doc: OpenRpcDoc, version: Option<String>) -> ProtocolIR {
    let version_map = load_method_version_map().unwrap_or_default();
    convert_to_protocol_ir_with_version_map(doc, version, &version_map)
}

/// Loads canonical IR, applies an OpenRPC update (preserving param/field version_*), and writes back to the same path.
///
/// Single place for "update canonical from OpenRPC" semantics. Uses per-definition merge:
/// for each method in the converted IR, merges version_* from existing IR then writes into
/// the rpc module; never clones the whole module.
pub fn update_canonical_ir_from_openrpc(
    canonical_path: &Path,
    doc: OpenRpcDoc,
    version: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let (mut existing_ir, version_map) = load_ir_and_version_map_from_path(canonical_path)?;
    let protocol_ir =
        convert_to_protocol_ir_with_version_map(doc, Some(version.to_string()), &version_map);

    let existing_rpcs: HashMap<String, &ir::RpcDef> =
        existing_ir.get_rpc_methods().into_iter().map(|r| (r.name.clone(), r)).collect();

    let converted_module = protocol_ir.modules().first().expect("convert produces one rpc module");
    let mut new_defs = Vec::with_capacity(converted_module.definitions().len());
    for def in converted_module.definitions() {
        match def {
            ProtocolDef::RpcMethod(ref rpc) => {
                let mut rpc = rpc.clone();
                if let Some(existing_rpc) = existing_rpcs.get(&rpc.name) {
                    merge_version_from_existing_rpc(&mut rpc, existing_rpc);
                }
                new_defs.push(ProtocolDef::RpcMethod(rpc));
            }
            other => new_defs.push(other.clone()),
        }
    }

    if let Some(rpc_module) = existing_ir.modules_mut().iter_mut().find(|m| m.name() == "rpc") {
        *rpc_module.definitions_mut() = new_defs;
    } else {
        existing_ir.modules_mut().push(ProtocolModule::new(
            "rpc".to_string(),
            "Bitcoin RPC API".to_string(),
            new_defs,
        ));
    }

    existing_ir.to_file(canonical_path)?;
    Ok(())
}

/// Runs the `process_bitcoin_openrpc` binary entry point.
///
/// Usage patterns:
/// 1. Convert OpenRPC to IR: process_bitcoin_openrpc <openrpc_file> [output_file]
/// 2. Extract version-specific IR: process_bitcoin_openrpc <version> [output_file]
///
/// Library entry point for the `process_bitcoin_openrpc` binary.
pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 2 {
        eprintln!("Usage:");
        eprintln!(
            "  {} <openrpc_file> [output_file]                     # Convert OpenRPC to IR",
            args[0]
        );
        eprintln!("  {} <version> [output_file]                         # Extract version-specific IR from canonical IR", args[0]);
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  {} openrpc.json output.ir.json", args[0]);
        eprintln!("  {} 30.2", args[0]);
        eprintln!("  {} 30.2 v30_2_0_bitcoin.ir.json", args[0]);
        std::process::exit(1);
    }

    let first_arg = &args[1];

    // Check if first argument is a version string (contains only digits and dots, or starts with 'v')
    let is_version =
        first_arg.trim_start_matches('v').chars().all(|c| c.is_ascii_digit() || c == '.');

    if is_version {
        // Mode 2: Extract version-specific IR from canonical IR
        let version = first_arg.trim_start_matches('v');
        let project_root = find_project_root()?;
        let canonical_ir_path = canonical_bitcoin_ir_path(&project_root);

        let canonical_ir = ProtocolIR::from_file(&canonical_ir_path)?;
        let version_ir = extract_version_ir(canonical_ir, version);

        let output_file = if args.len() >= 3 {
            PathBuf::from(&args[2])
        } else {
            get_ir_dir()?.join(version_ir_filename(version, "bitcoin"))
        };
        let output_resolved = resolve_ir_output_path(&project_root, &output_file);

        version_ir.to_file(&output_resolved)?;
        println!("✓ Extracted version-specific IR: {}", output_resolved.display());
    } else {
        // Mode 1: Convert OpenRPC to IR
        let openrpc_file = PathBuf::from(first_arg);
        let doc = load_openrpc_doc(&openrpc_file)?;
        let version = extract_version_from_openrpc(&doc)?;

        let project_root = find_project_root()?;
        let canonical_ir_path = canonical_bitcoin_ir_path(&project_root);
        let output_file = if args.len() >= 3 {
            PathBuf::from(&args[2])
        } else {
            get_ir_dir()?.join(version_ir_filename(&version, "bitcoin"))
        };
        let output_resolved = resolve_ir_output_path(&project_root, &output_file);
        let writing_canonical = output_resolved == canonical_ir_path;

        if writing_canonical {
            update_canonical_ir_from_openrpc(&output_resolved, doc, &version)?;
        } else {
            let protocol_ir = convert_to_protocol_ir_with_version(doc, Some(version.clone()));
            protocol_ir.to_file(&output_resolved)?;
        }

        println!("✓ Converted OpenRPC to IR: {}", output_resolved.display());
        println!("  Version: {}", version);
    }

    Ok(())
}
