// SPDX-License-Identifier: CC0-1.0

//! Bitcoin Core OpenRPC helpers (versioning and IR extraction).
//!
//! After OpenRPC → IR conversion, `openrpc_type_disambiguation` assigns distinct `TypeDef::name`
//! values where the schema overloads one name for different object shapes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ir::{
    FieldDef, FieldKey, ParamDef, ProtocolDef, ProtocolIR, ProtocolModule, RpcDef,
    RpcResultDiscriminator, TypeDef, TypeKind, UnionVariantDef,
};
use normalization::bitcoin_canonical_from_adapter_method;
use path::{
    canonical_bitcoin_ir_path, find_project_root, get_ir_dir, resolve_ir_output_path,
    version_ir_filename,
};
use semantics::method_categorization;
use serde::Deserialize;
use types::ProtocolVersion;

use crate::conversion_helpers::{determine_requires_private_keys, sort_definitions_by_name};

/// Param / field names that are integer-only across the RPC surface despite Core's
/// OpenRPC often declaring `type: number` (ported from `corpus/rust-btc-codegen`).
pub static INTEGER_PARAM_NAMES: &[&str] = &[
    "height",
    "verbosity",
    "verbose",
    "minconf",
    "maxconf",
    "conf_target",
    "nblocks",
    "blocks",
    "count",
    "num_blocks",
    "n",
    "version",
    "locktime",
    "port",
    "timeout",
    "millis",
    "block_timeout",
    "node_id",
    "rescan_height",
    "start_height",
    "stop_height",
    "depth",
    "index",
    "nout",
    "vout",
    "skip",
    "nodeid",
    "id",
    "uid",
    "peer_id",
    "timestamp",
    "confirmations",
    "size",
    "vsize",
    "weight",
    "strippedsize",
    "time",
    "mediantime",
    "nonce",
    "sequence",
];

fn default_openrpc_category() -> String { "misc".to_string() }

fn is_integerish_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    if INTEGER_PARAM_NAMES.iter().any(|e| *e == n) {
        return true;
    }
    n.ends_with("_height")
        || n.ends_with("_count")
        || n.ends_with("_index")
        || n.ends_with("_version")
        || n.ends_with("_id")
        || n.ends_with("conf")
}

/// Whether Core's `type: number` for this name should be treated as an integer domain.
pub fn openrpc_name_is_integer_domain(name: &str) -> bool { is_integerish_name(name) }

/// Canonical PascalCase method name used as a stable prefix for generated types.
fn canonical_method_pascal(method_name: &str) -> String {
    bitcoin_canonical_from_adapter_method(method_name, None)
        .unwrap_or_else(|_| normalization::suggest_canonical_key(method_name))
}

fn result_key_pascal_suffix(key: &str) -> String {
    let mut out = String::new();
    let mut upper = true;
    for ch in key.chars() {
        if ch.is_ascii_alphanumeric() {
            if upper {
                out.push(ch.to_ascii_uppercase());
                upper = false;
            } else {
                out.push(ch);
            }
        } else {
            upper = true;
        }
    }
    out
}

fn sanitize_identity_segment(input: &str) -> String {
    let mut out = String::new();
    let mut prev_us = false;
    for ch in input.chars() {
        let mapped = if ch.is_ascii_alphanumeric() { ch.to_ascii_lowercase() } else { '_' };
        if mapped == '_' {
            if !prev_us {
                out.push(mapped);
            }
            prev_us = true;
        } else {
            out.push(mapped);
            prev_us = false;
        }
    }
    out.trim_matches('_').to_string()
}

fn annotate_type_identity(
    type_def: &mut TypeDef,
    method_name: Option<&str>,
    parent_key: Option<&str>,
    raw: &RawResult,
) {
    if !matches!(
        type_def.kind,
        TypeKind::Object | TypeKind::Array | TypeKind::Union | TypeKind::Map
    ) {
        return;
    }
    let Some(method) = method_name else { return };
    let method_seg = sanitize_identity_segment(&canonical_method_pascal(method));
    let kind_seg = sanitize_identity_segment(&format!("{:?}", type_def.kind));
    let key_seg = if !raw.key_name.is_empty() {
        sanitize_identity_segment(&raw.key_name)
    } else {
        sanitize_identity_segment(parent_key.unwrap_or("result"))
    };
    let cond_seg = if raw.condition.is_empty() {
        "always".to_string()
    } else {
        sanitize_identity_segment(&raw.condition)
    };
    let identity = format!("{method_seg}__{kind_seg}__{key_seg}__{cond_seg}");
    if type_def.type_identity.is_none() {
        // Prefer a method-scoped PascalCase name for rust_emit_name(); keep the slug only when
        // the type is still anonymous (`object` / `array`).
        if type_def.name != "object"
            && type_def.name != "array"
            && type_def.name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        {
            type_def.type_identity = Some(type_def.name.clone());
        } else {
            type_def.type_identity = Some(identity);
        }
    }
}

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
    #[serde(rename = "x-bitcoin-category", default = "default_openrpc_category")]
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
        // Tuple-shaped JSON array (fixed arity); same wire encoding as `array`.
        "array-fixed" => "array".to_string(),
        "boolean" => "boolean".to_string(),
        // JSON `false` or a JSON object (e.g. `getwalletinfo` `scanning`).
        "bool-or-object" => "bool-or-object".to_string(),
        "elision" => "elision".to_string(),
        "hex" => "hex".to_string(),
        "none" => "none".to_string(),
        "number" => "number".to_string(),
        "object" => "object".to_string(),
        // Dynamic-key JSON objects (OpenRPC); inner entries are illustrative, not a fixed struct.
        "object-dynamic" => "object-dynamic".to_string(),
        // Mutually exclusive JSON object shapes (Core `object-one-of`); branches live in `inner`.
        "object-one-of" => "object-one-of".to_string(),
        "range" => "range".to_string(),
        "string" => "string".to_string(),
        // JSON string or JSON array of strings (e.g. `warnings` on getblockchaininfo).
        "string-or-string-array" => "string-or-string-array".to_string(),
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
    /// Single object template whose children carry JSON key names (wire: JSON array of objects).
    fn is_named_object_array_template(&self) -> bool;
}

impl HasTypeAndInner for RawArgument {
    fn has_object_inner(&self) -> bool { self.r#type == "object" && !self.inner.is_empty() }

    fn is_named_object_array_template(&self) -> bool { false }
}

impl HasTypeAndInner for RawResult {
    fn has_object_inner(&self) -> bool { self.r#type == "object" && !self.inner.is_empty() }

    fn is_named_object_array_template(&self) -> bool {
        self.r#type == "object"
            && !self.inner.is_empty()
            && self.inner.iter().any(|x| !x.key_name.is_empty())
    }
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
        "array" | "array-fixed" => {
            if inner.is_empty() {
                TypeKind::Array
            } else if inner.len() == 1 && inner[0].is_named_object_array_template() {
                // JSON array of objects with real field keys (e.g. listaddressgroupings rows).
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
        "object-dynamic" => TypeKind::Map,
        // Arguments should not use this; results are handled in `convert_result` before `kind` is used.
        "object-one-of" => TypeKind::Union,
        // Results only; lowered to a two-branch union in `convert_result`.
        "string-or-string-array" => TypeKind::Union,
        "bool-or-object" => TypeKind::Union,
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
        emit_in_struct: None,
        force_optional: None,
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
    if let Some(uvars) = &ty.union_variants {
        out.union_variants = Some(
            uvars
                .iter()
                .map(|uv| {
                    let mut v = uv.clone();
                    v.type_def = filter_type_def_for_version(&uv.type_def, target);
                    v
                })
                .collect(),
        );
    }
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
    if let Some(mv) = &ty.map_value {
        out.map_value = Some(Box::new(filter_type_def_for_version(mv, target)));
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
    if let (Some(our_uv), Some(ex_uv)) = (&mut our.union_variants, existing.union_variants.as_ref())
    {
        for (i, our_v) in our_uv.iter_mut().enumerate() {
            if let Some(ex_v) = ex_uv.get(i) {
                merge_version_into_type_def(&mut our_v.type_def, &ex_v.type_def);
            }
        }
    }
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
                if ex_f.emit_in_struct.is_some() {
                    our_f.emit_in_struct = ex_f.emit_in_struct;
                }
                if ex_f.force_optional.is_some() {
                    our_f.force_optional = ex_f.force_optional;
                }
                merge_version_into_type_def(&mut our_f.field_type, &ex_f.field_type);
            }
        }
    }
    if let (Some(our_mv), Some(ex_mv)) = (&mut our.map_value, existing.map_value.as_deref()) {
        merge_version_into_type_def(our_mv.as_mut(), ex_mv);
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
        "array" | "array-fixed" => "array".to_string(),
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
            emit_in_struct: None,
            force_optional: None,
        });

        type_def.fields = Some(if matches!(raw.r#type.as_str(), "array" | "array-fixed") {
            build_array_of_objects_wrapper(fields)
        } else {
            fields
        });
    }

    type_def
}

/// Converts a raw result to a `TypeDef`.
/// `parent_key`: when recursing, the key of the parent field (e.g. "vin") so we can name array element types.
/// `method_name`: RPC method name (e.g. "decodepsbt") so we can assign stable type names for codegen.
/// `schema_hint`: OpenRPC `result.schema` when converting the method's top-level result (for oneOf branch metadata).
fn convert_result(
    raw: &RawResult,
    parent_key: Option<&str>,
    method_name: Option<&str>,
    schema_hint: Option<&serde_json::Value>,
) -> TypeDef {
    if raw.r#type == "object-one-of" && !raw.inner.is_empty() {
        let m = method_name.unwrap_or("rpc");
        let disambig =
            if !raw.key_name.is_empty() { Some(raw.key_name.as_str()) } else { parent_key };
        let union_type_name = if disambig.is_none() {
            format!("{}ResultUnion", canonical_method_pascal(m))
        } else {
            format!(
                "{}{}ObjectOneOfUnion",
                canonical_method_pascal(m),
                result_key_pascal_suffix(disambig.unwrap())
            )
        };
        let mut union = build_union_from_raw_results(&raw.inner, m, schema_hint, &union_type_name);
        annotate_type_identity(&mut union, method_name, parent_key, raw);
        return union;
    }

    if raw.r#type == "string-or-string-array" {
        let m = method_name.unwrap_or("rpc");
        let method_pascal = canonical_method_pascal(m);
        let field_hint = if !raw.key_name.is_empty() {
            raw.key_name.as_str()
        } else {
            parent_key.unwrap_or("value")
        };
        let union_name = format!(
            "{}{}StringOrStringArrayUnion",
            method_pascal,
            result_key_pascal_suffix(field_hint)
        );

        let mut string_branch = TypeDef {
            name: "string".to_string(),
            description: raw.description.clone(),
            kind: TypeKind::Primitive,
            protocol_type: Some("string".to_string()),
            ..Default::default()
        };
        let elem_desc = raw
            .inner
            .first()
            .map(|r| r.description.clone())
            .unwrap_or_else(|| "warning".to_string());
        let mut array_branch = TypeDef {
            name: "array".to_string(),
            description: raw.description.clone(),
            kind: TypeKind::Array,
            protocol_type: Some("array".to_string()),
            fields: Some(vec![FieldDef {
                key: FieldKey::Named("field_0".to_string()),
                field_type: TypeDef {
                    name: "string".to_string(),
                    description: elem_desc,
                    kind: TypeKind::Primitive,
                    protocol_type: Some("string".to_string()),
                    ..Default::default()
                },
                required: true,
                description: String::new(),
                default_value: None,
                version_added: None,
                version_removed: None,
                emit_in_struct: None,
                force_optional: None,
            }]),
            ..Default::default()
        };
        let mut anon_1 = 0usize;
        uniquify_anonymous_types(&mut string_branch, &method_pascal, 1, &mut anon_1);
        let mut anon_2 = 0usize;
        uniquify_anonymous_types(&mut array_branch, &method_pascal, 2, &mut anon_2);

        let mut td = TypeDef {
            name: union_name,
            description: raw.description.clone(),
            kind: TypeKind::Union,
            union_variants: Some(vec![
                UnionVariantDef {
                    name: "String".to_string(),
                    description: "Deprecated single-string form (`-deprecatedrpc=warnings`)"
                        .to_string(),
                    condition: None,
                    type_def: string_branch,
                },
                UnionVariantDef {
                    name: "StringArray".to_string(),
                    description: "Array of warning strings".to_string(),
                    condition: None,
                    type_def: array_branch,
                },
            ]),
            protocol_type: Some("string-or-string-array".to_string()),
            condition: if raw.condition.is_empty() { None } else { Some(raw.condition.clone()) },
            ..Default::default()
        };
        annotate_type_identity(&mut td, method_name, parent_key, raw);
        return td;
    }

    if raw.r#type == "bool-or-object" {
        let m = method_name.unwrap_or("rpc");
        let method_pascal = canonical_method_pascal(m);
        let field_hint = if !raw.key_name.is_empty() {
            raw.key_name.as_str()
        } else {
            parent_key.unwrap_or("value")
        };
        let union_name =
            format!("{}{}BoolOrObjectUnion", method_pascal, result_key_pascal_suffix(field_hint));

        let mut bool_branch = TypeDef {
            name: "boolean".to_string(),
            description: raw.description.clone(),
            kind: TypeKind::Primitive,
            protocol_type: Some("boolean".to_string()),
            ..Default::default()
        };

        let child_parent =
            if !raw.key_name.is_empty() { Some(raw.key_name.as_str()) } else { parent_key };
        let mut object_branch = if let Some(obj_raw) = raw.inner.first() {
            convert_result(obj_raw, child_parent, method_name, None)
        } else {
            TypeDef {
                name: "object".to_string(),
                description: raw.description.clone(),
                kind: TypeKind::Object,
                protocol_type: Some("object".to_string()),
                fields: Some(vec![]),
                ..Default::default()
            }
        };

        let mut anon_1 = 0usize;
        uniquify_anonymous_types(&mut bool_branch, &method_pascal, 1, &mut anon_1);
        let mut anon_2 = 0usize;
        uniquify_anonymous_types(&mut object_branch, &method_pascal, 2, &mut anon_2);

        let mut td = TypeDef {
            name: union_name,
            description: raw.description.clone(),
            kind: TypeKind::Union,
            union_variants: Some(vec![
                UnionVariantDef {
                    name: "Bool".to_string(),
                    description: "`false` when no scan is in progress".to_string(),
                    condition: None,
                    type_def: bool_branch,
                },
                UnionVariantDef {
                    name: "Object".to_string(),
                    description: "Scanning progress object".to_string(),
                    condition: None,
                    type_def: object_branch,
                },
            ]),
            protocol_type: Some("bool-or-object".to_string()),
            condition: if raw.condition.is_empty() { None } else { Some(raw.condition.clone()) },
            ..Default::default()
        };
        annotate_type_identity(&mut td, method_name, parent_key, raw);
        return td;
    }

    if raw.r#type == "object-dynamic" {
        let key_protocol = match raw.key_name.as_str() {
            "txid" | "wtxid" | "blockhash" => "hex",
            _ => "string",
        };
        let mut value_type = if let Some(first) = raw.inner.first() {
            convert_result(first, Some("map_value"), method_name, None)
        } else {
            TypeDef {
                name: "any".to_string(),
                description: raw.description.clone(),
                kind: TypeKind::Primitive,
                protocol_type: Some("any".to_string()),
                ..Default::default()
            }
        };
        if value_type.type_identity.is_none() {
            value_type.type_identity =
                Some(format!("{}MapValue", canonical_method_pascal(method_name.unwrap_or("rpc"))));
        }
        let mut td = TypeDef {
            name: "object_dynamic".to_string(),
            description: raw.description.clone(),
            kind: TypeKind::Map,
            protocol_type: Some("object-dynamic".to_string()),
            map_key_protocol_type: Some(key_protocol.to_string()),
            map_value: Some(Box::new(value_type)),
            condition: if raw.condition.is_empty() { None } else { Some(raw.condition.clone()) },
            ..Default::default()
        };
        annotate_type_identity(&mut td, method_name, parent_key, raw);
        return td;
    }

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

    // JSON array with element template (including nested arrays, e.g. listaddressgroupings).
    if matches!(kind, TypeKind::Array) && !raw.inner.is_empty() {
        let elem = &raw.inner[0];
        let elem_parent_key: Option<&str> =
            if raw.key_name.is_empty() { Some("array_child") } else { Some(raw.key_name.as_str()) };
        let mut element_type = convert_result(elem, elem_parent_key, method_name, None);
        if matches!(element_type.kind, TypeKind::Array) && element_type.name == "array" {
            if let Some(method) = method_name {
                if parent_key.is_none() {
                    element_type.name = format!("{}Group", canonical_method_pascal(method));
                }
            }
        }
        type_def.fields = Some(vec![FieldDef {
            key: FieldKey::Anonymous(0),
            field_type: element_type,
            required: !raw.optional,
            description: raw.description.clone(),
            default_value: None,
            version_added: None,
            version_removed: None,
            emit_in_struct: None,
            force_optional: None,
        }]);
        annotate_type_identity(&mut type_def, method_name, parent_key, raw);
        return type_def;
    }

    // Handle nested structures
    if matches!(kind, TypeKind::Object) && !raw.inner.is_empty() {
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
                let child_parent = if inner.key_name.is_empty() {
                    if raw.key_name.is_empty() {
                        parent_key
                    } else {
                        Some(raw.key_name.as_str())
                    }
                } else {
                    Some(inner.key_name.as_str())
                };
                FieldDef {
                    key: FieldKey::Named(name),
                    field_type: convert_result(inner, child_parent, method_name, None),
                    required: inner.is_required(),
                    description: inner.description.clone(),
                    default_value: inner.default_value(),
                    version_added: None,
                    version_removed: None,
                    emit_in_struct: None,
                    force_optional: None,
                }
            })
            .collect();

        type_def.fields = Some(if matches!(raw.r#type.as_str(), "array" | "array-fixed") {
            build_array_of_objects_wrapper(fields)
        } else {
            fields
        });
    }

    if matches!(kind, TypeKind::Object) && !raw.inner.is_empty() {
        if let Some(p) = parent_key {
            match p {
                "array_child" if raw.r#type == "object" =>
                    if let Some(method) = method_name {
                        type_def.name = format!("{}Row", canonical_method_pascal(method));
                    },
                _ => {}
            }
        }
        // Method-scoped names for object types so codegen emits nested structs (e.g. DecodepsbtTx, DecodepsbtInput)
        if type_def.name == "object" {
            if let Some(method) = method_name {
                let method_pascal = canonical_method_pascal(method);
                if !raw.key_name.is_empty() {
                    type_def.name =
                        format!("{}{}", method_pascal, result_key_pascal_suffix(&raw.key_name));
                } else if let Some(p) = parent_key {
                    type_def.name = format!("{}{}", method_pascal, result_key_pascal_suffix(p));
                }
            }
        }
    }

    annotate_type_identity(&mut type_def, method_name, parent_key, raw);
    type_def
}

/// Bitcoin Core OpenRPC: `schema.x-bitcoin-discriminatedResult` when the result is a keyed `oneOf`.
fn parse_result_discriminator(schema: &serde_json::Value) -> Option<RpcResultDiscriminator> {
    let disc = schema.get("x-bitcoin-discriminatedResult")?;
    let (parameter, parameter_index) = if let (Some(params), Some(indices)) = (
        disc.get("parameters").and_then(|v| v.as_array()),
        disc.get("parameterIndices").and_then(|v| v.as_array()),
    ) {
        let param_joined = params.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join("|");
        let first_idx = indices.first().and_then(|v| v.as_i64())?;
        (param_joined, u32::try_from(first_idx).ok()?)
    } else {
        (
            disc.get("parameter")?.as_str()?.to_string(),
            u32::try_from(disc.get("parameterIndex")?.as_i64()?).ok()?,
        )
    };
    Some(RpcResultDiscriminator {
        parameter,
        parameter_index,
        values: disc.get("values")?.as_array()?.clone(),
        nested_parameter_key: disc
            .get("nestedParameterKey")
            .and_then(|v| v.as_str())
            .map(str::to_string),
    })
}

fn protocol_type_from_schema_type(schema_type: &str) -> Option<&'static str> {
    match schema_type {
        "string" => Some("string"),
        "number" => Some("number"),
        "integer" => Some("number"),
        "boolean" => Some("boolean"),
        "object" => Some("object"),
        "array" => Some("array"),
        "null" => Some("none"),
        _ => None,
    }
}

fn schema_description<'a>(schema: &'a serde_json::Value, fallback: &'a str) -> &'a str {
    schema.get("description").and_then(|v| v.as_str()).unwrap_or(fallback)
}

fn primary_json_schema_type(schema: &serde_json::Value) -> Option<&str> {
    match schema.get("type") {
        Some(serde_json::Value::String(s)) => Some(s.as_str()),
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.as_str())
            .find(|t| *t != "null")
            .or_else(|| arr.iter().filter_map(|v| v.as_str()).next()),
        _ => None,
    }
}

fn schema_allows_null(schema: &serde_json::Value) -> bool {
    match schema.get("type") {
        Some(serde_json::Value::String(s)) => s == "null",
        Some(serde_json::Value::Array(arr)) => arr.iter().any(|v| v.as_str() == Some("null")),
        _ => false,
    }
}

fn object_type_name(method_name: Option<&str>, field_name: Option<&str>, fallback: &str) -> String {
    match (method_name, field_name) {
        (Some(method), Some(key)) if !key.is_empty() && key != "result" => {
            format!("{}{}", canonical_method_pascal(method), result_key_pascal_suffix(key))
        }
        _ => fallback.to_string(),
    }
}

fn annotate_schema_type_identity(
    type_def: &mut TypeDef,
    method_name: Option<&str>,
    parent_key: Option<&str>,
    condition: Option<&str>,
) {
    if !matches!(
        type_def.kind,
        TypeKind::Object | TypeKind::Array | TypeKind::Union | TypeKind::Map
    ) {
        return;
    }
    let Some(method) = method_name else { return };
    let method_seg = sanitize_identity_segment(&canonical_method_pascal(method));
    let kind_seg = sanitize_identity_segment(&format!("{:?}", type_def.kind));
    let key_seg = sanitize_identity_segment(parent_key.unwrap_or("result"));
    let cond_seg = sanitize_identity_segment(condition.unwrap_or("always"));
    let identity = format!("{method_seg}__{kind_seg}__{key_seg}__{cond_seg}");
    if type_def.type_identity.is_none() {
        // Prefer a method-scoped PascalCase name for rust_emit_name(); keep the slug only when
        // the type is still anonymous (`object` / `array`).
        if type_def.name != "object"
            && type_def.name != "array"
            && type_def.name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        {
            type_def.type_identity = Some(type_def.name.clone());
        } else {
            type_def.type_identity = Some(identity);
        }
    }
}

fn infer_union_protocol_type(field_name: Option<&str>) -> &'static str {
    let Some(name) = field_name else {
        return "any";
    };
    let n: String =
        name.chars().filter(|c| *c != '_' && *c != '-').flat_map(|c| c.to_lowercase()).collect();
    match n.as_str() {
        // Core emits number|string oneOf for amounts and fee rates; keep amount domain so
        // map_parameter_type_to_rust can pick FeeRate / bitcoin::Amount from the field name.
        "amount" | "fee" | "feerate" | "maxfeerate" | "maxburnamount" => "amount",
        "range" => "range",
        _ => "any",
    }
}

fn union_from_schema_branches(
    branches: &[serde_json::Value],
    fallback_name: &str,
    description_fallback: &str,
    method_name: Option<&str>,
    field_name: Option<&str>,
) -> TypeDef {
    let method_pascal = canonical_method_pascal(method_name.unwrap_or("rpc"));
    let union_variants = branches
        .iter()
        .enumerate()
        .map(|(idx, branch)| {
            let cond = branch
                .get("description")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            let branch_fallback = format!("{fallback_name}Branch{}", idx + 1);
            let mut branch_type = type_def_from_json_schema_ctx(
                branch,
                &branch_fallback,
                schema_description(branch, description_fallback),
                method_name,
                Some(&format!("branch_{}", idx + 1)),
            );
            let mut anon_counter = 0usize;
            uniquify_anonymous_types(&mut branch_type, &method_pascal, idx + 1, &mut anon_counter);
            UnionVariantDef {
                name: format!("Branch{}", idx + 1),
                description: schema_description(branch, description_fallback).to_string(),
                condition: cond,
                type_def: branch_type,
            }
        })
        .collect();

    let protocol = infer_union_protocol_type(field_name);
    let mut name = fallback_name.to_string();
    // Stable dialect type for hash_or_height params (client import + signature).
    if field_name.is_some_and(|n| {
        n.chars()
            .filter(|c| *c != '_' && *c != '-')
            .flat_map(|c| c.to_lowercase())
            .eq("hashorheight".chars())
    }) {
        name = "HashOrHeight".to_string();
    }

    let mut td = TypeDef {
        name,
        description: "Union result preserved from OpenRPC oneOf/anyOf branches".to_string(),
        kind: TypeKind::Union,
        union_variants: Some(union_variants),
        protocol_type: Some(protocol.to_string()),
        ..Default::default()
    };
    annotate_schema_type_identity(&mut td, method_name, field_name.or(Some("result")), None);
    td
}

fn type_def_from_json_schema(
    schema: &serde_json::Value,
    fallback_name: &str,
    description_fallback: &str,
) -> TypeDef {
    type_def_from_json_schema_ctx(schema, fallback_name, description_fallback, None, None)
}

/// JSON Schema → [`TypeDef`] walker for Core schema-first OpenRPC (`params` / `result.schema`).
fn type_def_from_json_schema_ctx(
    schema: &serde_json::Value,
    fallback_name: &str,
    description_fallback: &str,
    method_name: Option<&str>,
    field_name: Option<&str>,
) -> TypeDef {
    let desc = schema_description(schema, description_fallback).to_string();

    if let Some(branches) =
        schema.get("oneOf").or_else(|| schema.get("anyOf")).and_then(|v| v.as_array())
    {
        if branches.len() >= 2 {
            return union_from_schema_branches(
                branches,
                fallback_name,
                description_fallback,
                method_name,
                field_name,
            );
        }
        if branches.len() == 1 {
            return type_def_from_json_schema_ctx(
                &branches[0],
                fallback_name,
                description_fallback,
                method_name,
                field_name,
            );
        }
    }

    if schema.get("x-bitcoin-unit").and_then(|v| v.as_str()) == Some("amount") {
        return TypeDef {
            name: "amount".to_string(),
            description: desc,
            kind: TypeKind::Primitive,
            protocol_type: Some("amount".to_string()),
            ..Default::default()
        };
    }

    if schema.get("x-bitcoin-unit").and_then(|v| v.as_str()) == Some("unix-time") {
        return TypeDef {
            name: "timestamp".to_string(),
            description: desc,
            kind: TypeKind::Primitive,
            protocol_type: Some("timestamp".to_string()),
            ..Default::default()
        };
    }

    let schema_type = primary_json_schema_type(schema);

    match schema_type {
        Some("object") => {
            let props_empty = schema
                .get("properties")
                .and_then(|v| v.as_object())
                .map(|o| o.is_empty())
                .unwrap_or(true);
            let additional_is_open = schema
                .get("additionalProperties")
                .is_some_and(|ap| ap.as_bool() == Some(true) || ap.is_object());
            let is_dynamic =
                schema.get("x-bitcoin-object-dynamic").and_then(|v| v.as_bool()).unwrap_or(false)
                    || (props_empty && additional_is_open);

            if is_dynamic {
                if let Some(additional) = schema.get("additionalProperties") {
                    if additional.as_bool() != Some(false) {
                        let value_type = if let Some(true) = additional.as_bool() {
                            TypeDef {
                                name: "any".to_string(),
                                kind: TypeKind::Primitive,
                                protocol_type: Some("any".to_string()),
                                ..Default::default()
                            }
                        } else {
                            type_def_from_json_schema_ctx(
                                additional,
                                &format!("{fallback_name}Value"),
                                &desc,
                                method_name,
                                Some("map_value"),
                            )
                        };
                        let map_name = object_type_name(
                            method_name,
                            field_name,
                            &format!("{fallback_name}Map"),
                        );
                        let mut td = TypeDef {
                            name: map_name,
                            description: desc,
                            kind: TypeKind::Map,
                            protocol_type: Some("object-dynamic".to_string()),
                            map_value: Some(Box::new(value_type)),
                            map_key_protocol_type: Some(
                                schema
                                    .get("x-ethos-map-key-protocol-type")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("string")
                                    .to_string(),
                            ),
                            ..Default::default()
                        };
                        annotate_schema_type_identity(
                            &mut td,
                            method_name,
                            field_name.or(Some("result")),
                            None,
                        );
                        return td;
                    }
                }
            }

            let required_set: std::collections::BTreeSet<String> = schema
                .get("required")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect::<std::collections::BTreeSet<_>>()
                })
                .unwrap_or_default();
            let fields = schema
                .get("properties")
                .and_then(|v| v.as_object())
                .map(|props| {
                    props
                        .iter()
                        .map(|(name, prop_schema)| {
                            let child_fallback = format!(
                                "{}{}",
                                object_type_name(method_name, field_name, fallback_name),
                                result_key_pascal_suffix(name)
                            );
                            FieldDef {
                                key: FieldKey::Named(name.clone()),
                                field_type: type_def_from_json_schema_ctx(
                                    prop_schema,
                                    &child_fallback,
                                    prop_schema
                                        .get("description")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or_default(),
                                    method_name,
                                    Some(name.as_str()),
                                ),
                                required: required_set.contains(name),
                                description: prop_schema
                                    .get("description")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                                default_value: None,
                                version_added: None,
                                version_removed: None,
                                emit_in_struct: None,
                                force_optional: None,
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let mut td = TypeDef {
                name: object_type_name(method_name, field_name, fallback_name),
                description: desc,
                kind: TypeKind::Object,
                fields: Some(fields),
                protocol_type: Some("object".to_string()),
                ..Default::default()
            };
            annotate_schema_type_identity(
                &mut td,
                method_name,
                field_name.or(Some("result")),
                None,
            );
            td
        }
        Some("array") => {
            let fields =
                if let Some(prefix_items) = schema.get("prefixItems").and_then(|v| v.as_array()) {
                    prefix_items
                        .iter()
                        .enumerate()
                        .map(|(idx, item_schema)| FieldDef {
                            key: FieldKey::Anonymous(idx),
                            field_type: type_def_from_json_schema_ctx(
                                item_schema,
                                &format!("{}Item{}", fallback_name, idx),
                                description_fallback,
                                method_name,
                                field_name,
                            ),
                            required: true,
                            description: String::new(),
                            default_value: None,
                            version_added: None,
                            version_removed: None,
                            emit_in_struct: None,
                            force_optional: None,
                        })
                        .collect::<Vec<_>>()
                } else {
                    let items = schema.get("items");
                    // Draft-07 tuple form: items is an array of schemas.
                    let element = if let Some(serde_json::Value::Array(tuple)) = items {
                        if tuple.len() > 1 {
                            return type_def_from_json_schema_ctx(
                                &serde_json::json!({
                                    "type": "array",
                                    "prefixItems": tuple,
                                    "description": desc,
                                }),
                                fallback_name,
                                description_fallback,
                                method_name,
                                field_name,
                            );
                        }
                        tuple.first().map(|item| {
                            type_def_from_json_schema_ctx(
                                item,
                                &format!("{}Item", fallback_name),
                                description_fallback,
                                method_name,
                                field_name,
                            )
                        })
                    } else {
                        items.map(|item| {
                            type_def_from_json_schema_ctx(
                                item,
                                &format!("{}Item", fallback_name),
                                description_fallback,
                                method_name,
                                field_name,
                            )
                        })
                    };
                    let element = element.unwrap_or(TypeDef {
                        name: "any".to_string(),
                        kind: TypeKind::Primitive,
                        protocol_type: Some("any".to_string()),
                        ..Default::default()
                    });
                    vec![FieldDef {
                        key: FieldKey::Anonymous(0),
                        field_type: element,
                        required: true,
                        description: String::new(),
                        default_value: None,
                        version_added: None,
                        version_removed: None,
                        emit_in_struct: None,
                        force_optional: None,
                    }]
                };
            let mut td = TypeDef {
                name: "array".to_string(),
                description: desc,
                kind: TypeKind::Array,
                fields: Some(fields),
                protocol_type: Some("array".to_string()),
                ..Default::default()
            };
            annotate_schema_type_identity(
                &mut td,
                method_name,
                field_name.or(Some("result")),
                None,
            );
            td
        }
        Some("integer") => TypeDef {
            name: "number".to_string(),
            description: desc,
            kind: TypeKind::Primitive,
            protocol_type: Some("number".to_string()),
            ..Default::default()
        },
        Some("number") => TypeDef {
            name: "number".to_string(),
            description: desc,
            kind: TypeKind::Primitive,
            protocol_type: Some("number".to_string()),
            // Integer-looking names stay protocol `number`; BitcoinCoreTypeRegistry maps via field name.
            // `is_integerish_name` / INTEGER_PARAM_NAMES document the Core `number`→integer gap.
            ..Default::default()
        },
        Some("string") => {
            let protocol = if schema
                .get("pattern")
                .and_then(|v| v.as_str())
                .is_some_and(|p| p.contains("0-9a-fA-F") || p.contains("0-9a-f"))
                && field_name.is_some_and(|n| {
                    let n = n.to_ascii_lowercase();
                    n.contains("hash")
                        || n.contains("txid")
                        || n.contains("wtxid")
                        || n == "hex"
                        || n.ends_with("hex")
                }) {
                "hex"
            } else {
                "string"
            };
            TypeDef {
                name: protocol.to_string(),
                description: desc,
                kind: TypeKind::Primitive,
                protocol_type: Some(protocol.to_string()),
                ..Default::default()
            }
        }
        Some("boolean") => TypeDef {
            name: "boolean".to_string(),
            description: desc,
            kind: TypeKind::Primitive,
            protocol_type: Some("boolean".to_string()),
            ..Default::default()
        },
        Some("null") => TypeDef {
            name: "none".to_string(),
            description: desc,
            kind: TypeKind::Primitive,
            protocol_type: Some("none".to_string()),
            ..Default::default()
        },
        Some(other) => {
            let protocol = protocol_type_from_schema_type(other).unwrap_or("any");
            TypeDef {
                name: protocol.to_string(),
                description: desc,
                kind: TypeKind::Primitive,
                protocol_type: Some(protocol.to_string()),
                ..Default::default()
            }
        }
        None => {
            if schema_allows_null(schema) {
                return TypeDef {
                    name: "none".to_string(),
                    description: desc,
                    kind: TypeKind::Primitive,
                    protocol_type: Some("none".to_string()),
                    ..Default::default()
                };
            }
            TypeDef {
                name: "any".to_string(),
                description: desc,
                kind: TypeKind::Primitive,
                protocol_type: Some("any".to_string()),
                ..Default::default()
            }
        }
    }
}

fn convert_param_from_openrpc_value(param: &serde_json::Value) -> ParamDef {
    let name = param.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let required = param.get("required").and_then(|v| v.as_bool()).unwrap_or(false);
    let mut description =
        param.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
    if let Some(aliases) = param.get("x-bitcoin-aliases").and_then(|v| v.as_array()) {
        let alias_strs: Vec<&str> = aliases.iter().filter_map(|v| v.as_str()).collect();
        if !alias_strs.is_empty() {
            description = format!("{description} (aliases: {})", alias_strs.join(", "));
        }
    }
    if param.get("x-bitcoin-placeholder").and_then(|v| v.as_bool()).unwrap_or(false) {
        description = format!("{description} [x-bitcoin-placeholder]");
    }
    let schema = param.get("schema").cloned().unwrap_or(serde_json::Value::Null);
    let default_value = schema
        .get("default")
        .map(|v| match v {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .or_else(|| {
            schema.get("x-bitcoin-default-hint").and_then(|v| v.as_str()).map(str::to_string)
        });
    let mut param_type = type_def_from_json_schema_ctx(
        &schema,
        if name.is_empty() { "param" } else { &name },
        &description,
        None,
        Some(name.as_str()),
    );
    if param_type.protocol_type.is_none() {
        param_type.protocol_type = Some(
            match param_type.kind {
                TypeKind::Object => "object",
                TypeKind::Array => "array",
                TypeKind::Map => "object-dynamic",
                TypeKind::Union => infer_union_protocol_type(Some(name.as_str())),
                TypeKind::Primitive => param_type.name.as_str(),
                _ => "any",
            }
            .to_string(),
        );
    }
    if name
        .chars()
        .filter(|c| *c != '_' && *c != '-')
        .flat_map(|c| c.to_lowercase())
        .eq("hashorheight".chars())
    {
        param_type.name = "HashOrHeight".to_string();
    }
    ParamDef {
        name,
        param_type,
        required,
        description,
        default_value,
        version_added: None,
        version_removed: None,
    }
}

fn convert_result_from_schema(schema: &serde_json::Value, method_name: &str) -> TypeDef {
    let fallback = format!("{}Result", canonical_method_pascal(method_name));
    let desc = schema_description(schema, "");
    type_def_from_json_schema_ctx(schema, &fallback, desc, Some(method_name), Some("result"))
}

fn maybe_map_from_dynamic_object_schema(
    method_name: &str,
    raw: &RawResult,
    schema: Option<&serde_json::Value>,
) -> Option<TypeDef> {
    if raw.r#type != "object-dynamic"
        && !schema
            .and_then(|s| s.get("type").and_then(|v| v.as_str()))
            .is_some_and(|t| t == "object")
    {
        return None;
    }
    let schema = schema?;
    let additional = schema.get("additionalProperties")?;
    let map_name = format!("{}ResultMap", canonical_method_pascal(method_name));
    let value_name = format!("{}Value", map_name);
    let value_type = type_def_from_json_schema(additional, &value_name, &raw.description);
    let mut td = TypeDef {
        name: map_name,
        description: raw.description.clone(),
        kind: TypeKind::Map,
        protocol_type: Some("object-dynamic".to_string()),
        map_value: Some(Box::new(value_type)),
        map_key_protocol_type: Some("string".to_string()),
        ..Default::default()
    };
    annotate_type_identity(&mut td, Some(method_name), Some("result"), raw);
    Some(td)
}

/// Picks the longer description when RPC help repeats the same key with more detail later.
fn prefer_richer_description(a: &str, b: &str) -> String {
    if b.len() > a.len() {
        b.to_string()
    } else {
        a.to_string()
    }
}

/// Merges two `FieldDef` slices for the same JSON object: duplicate key -> recurse; new keys -> append.
fn merge_duplicate_field_def_lists(a: &[FieldDef], b: &[FieldDef]) -> Vec<FieldDef> {
    let mut out: Vec<FieldDef> = a.to_vec();
    for fb in b {
        let id = fb.key.as_ident();
        if let Some(existing) = out.iter_mut().find(|f| f.key.as_ident() == id) {
            existing.field_type =
                merge_duplicate_rpc_result_types(&existing.field_type, &fb.field_type);
            existing.required = existing.required && fb.required;
            if existing.description.is_empty() && !fb.description.is_empty() {
                existing.description = fb.description.clone();
            } else if !fb.description.is_empty() {
                existing.description =
                    prefer_richer_description(&existing.description, &fb.description);
            }
        } else {
            out.push(fb.clone());
        }
    }
    out
}

/// When OpenRPC lists the same JSON key twice, merge into one field while keeping a superset
/// of nested members.
fn merge_duplicate_rpc_result_types(a: &TypeDef, b: &TypeDef) -> TypeDef {
    match (&a.kind, &b.kind) {
        (TypeKind::Object, TypeKind::Object) => {
            let fields_a = a.fields.as_deref().unwrap_or(&[]);
            let fields_b = b.fields.as_deref().unwrap_or(&[]);
            if fields_a.is_empty() {
                return b.clone();
            }
            if fields_b.is_empty() {
                return a.clone();
            }

            let is_array_object_shell = |t: &TypeDef| {
                t.protocol_type.as_deref() == Some("array")
                    && t.fields.as_ref().is_some_and(|f| f.len() == 1)
            };
            if is_array_object_shell(a) && is_array_object_shell(b) {
                let fa = &fields_a[0];
                let fb = &fields_b[0];
                if fa.key.as_ident() == fb.key.as_ident() {
                    let merged_inner =
                        merge_duplicate_rpc_result_types(&fa.field_type, &fb.field_type);
                    return TypeDef {
                        name: a.name.clone(),
                        description: prefer_richer_description(&a.description, &b.description),
                        kind: TypeKind::Object,
                        fields: Some(vec![FieldDef {
                            key: fa.key.clone(),
                            field_type: merged_inner,
                            required: fa.required && fb.required,
                            description: prefer_richer_description(
                                &fa.description,
                                &fb.description,
                            ),
                            default_value: None,
                            version_added: None,
                            version_removed: None,
                            emit_in_struct: None,
                            force_optional: None,
                        }]),
                        condition: a.condition.clone().or_else(|| b.condition.clone()),
                        ..a.clone()
                    };
                }
            }

            let merged_fields = merge_duplicate_field_def_lists(fields_a, fields_b);
            TypeDef {
                name: a.name.clone(),
                description: prefer_richer_description(&a.description, &b.description),
                kind: TypeKind::Object,
                fields: Some(merged_fields),
                condition: a.condition.clone().or_else(|| b.condition.clone()),
                ..a.clone()
            }
        }
        (TypeKind::Array, TypeKind::Array) => {
            let ea = a.array_element_type();
            let eb = b.array_element_type();
            match (ea, eb) {
                (Some(x), Some(y)) => {
                    let merged_elem = merge_duplicate_rpc_result_types(x, y);
                    let mut out = a.clone();
                    if let Some(ref mut fields) = out.fields {
                        for f in fields.iter_mut() {
                            if f.key.is_positional_zero()
                                || matches!(&f.key, FieldKey::Named(s) if s == "field_0")
                            {
                                f.field_type = merged_elem;
                                break;
                            }
                        }
                    }
                    out.description = prefer_richer_description(&a.description, &b.description);
                    out
                }
                (Some(_), None) => a.clone(),
                (None, Some(_)) => b.clone(),
                (None, None) => a.clone(),
            }
        }
        _ => {
            if matches!(b.kind, TypeKind::Object | TypeKind::Array)
                && matches!(a.kind, TypeKind::Primitive)
            {
                b.clone()
            } else if matches!(a.kind, TypeKind::Object | TypeKind::Array)
                && matches!(b.kind, TypeKind::Primitive)
            {
                a.clone()
            } else if a.protocol_type == b.protocol_type {
                a.clone()
            } else {
                b.clone()
            }
        }
    }
}

/// Allocates a JSON key name that is unique among `field_names`, appending `_1`, `_2`, ... when needed.
fn ensure_unique_merged_field_name(
    field_names: &mut std::collections::HashSet<String>,
    base_name: String,
) -> String {
    let base_name_clone = base_name.clone();
    let mut name = base_name;
    let mut counter = 0;
    while field_names.contains(&name) {
        counter += 1;
        name = format!("{}_{}", base_name_clone, counter);
    }
    field_names.insert(name.clone());
    name
}

fn stable_merge_field_key(base_name: &str, fallback_index: usize) -> FieldKey {
    if base_name.is_empty() || base_name.starts_with("field_") {
        FieldKey::Anonymous(fallback_index)
    } else {
        FieldKey::Named(base_name.to_string())
    }
}

/// Merges multiple results into a single object `TypeDef`.
/// `method_name`: RPC method name so nested object types get stable names (e.g. DecodepsbtTx).
fn merge_results_to_object(results: &[RawResult], method_name: &str) -> TypeDef {
    let mut fields: Vec<FieldDef> = Vec::new();
    let mut field_names: std::collections::HashSet<String> = std::collections::HashSet::new();

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
                // If we have conditional results (simple type + object), make all fields optional
                // because the response type depends on the condition (e.g., verbose parameter)
                let is_required = if has_conditional_results {
                    false // Make optional when we have conditional results
                } else {
                    !inner.optional
                };

                let parent =
                    if result.key_name.is_empty() { None } else { Some(result.key_name.as_str()) };
                let new_field_type = convert_result(inner, parent, Some(method_name), None);

                if !inner.key_name.is_empty() && field_names.contains(&inner.key_name) {
                    if let Some(existing) = fields
                        .iter_mut()
                        .find(|f| f.key.json_key() == Some(inner.key_name.as_str()))
                    {
                        existing.field_type =
                            merge_duplicate_rpc_result_types(&existing.field_type, &new_field_type);
                        existing.required = existing.required && is_required;
                        if existing.description.is_empty() && !inner.description.is_empty() {
                            existing.description = inner.description.clone();
                        } else if !inner.description.is_empty() {
                            existing.description = prefer_richer_description(
                                &existing.description,
                                &inner.description,
                            );
                        }
                    }
                    continue;
                }

                let key = if !inner.key_name.is_empty() {
                    let name =
                        ensure_unique_merged_field_name(&mut field_names, inner.key_name.clone());
                    FieldKey::Named(name)
                } else {
                    stable_merge_field_key("", idx)
                };
                fields.push(FieldDef {
                    key,
                    field_type: new_field_type,
                    required: is_required,
                    description: inner.description.clone(),
                    default_value: None,
                    version_added: None,
                    version_removed: None,
                    emit_in_struct: None,
                    force_optional: None,
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
                    format!("field_{}", idx)
                }
            };

            let new_field_type = convert_result(result, None, Some(method_name), None);
            let is_required_leaf = !result.optional;

            if !result.key_name.is_empty() && field_names.contains(&result.key_name) {
                if let Some(existing) =
                    fields.iter_mut().find(|f| f.key.json_key() == Some(result.key_name.as_str()))
                {
                    existing.field_type =
                        merge_duplicate_rpc_result_types(&existing.field_type, &new_field_type);
                    existing.required = existing.required && is_required_leaf;
                    if existing.description.is_empty() && !result.description.is_empty() {
                        existing.description = result.description.clone();
                    } else if !result.description.is_empty() {
                        existing.description =
                            prefer_richer_description(&existing.description, &result.description);
                    }
                }
                continue;
            }

            let key = if result.key_name.is_empty() {
                stable_merge_field_key(&base_field_name, idx)
            } else {
                FieldKey::Named(ensure_unique_merged_field_name(&mut field_names, base_field_name))
            };
            fields.push(FieldDef {
                key,
                field_type: new_field_type,
                required: is_required_leaf,
                description: result.description.clone(),
                default_value: None,
                version_added: None,
                version_removed: None,
                emit_in_struct: None,
                force_optional: None,
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
/// Primary path (Bitcoin Core OpenRPC 1.4.1+): `params[]` + `result.schema`.
/// Legacy fallback: `x-bitcoin-arguments` / `x-bitcoin-results` when present.
///
/// version_added should be determined by the caller to avoid redundant lookups.
fn convert_openrpc_method(method: OpenRpcMethod, version_added: Option<String>) -> RpcDef {
    let openrpc_result = method.result.as_ref();
    let legacy_results = openrpc_result.map(|r| r.x_bitcoin_results.clone()).unwrap_or_default();
    let result_discriminator =
        openrpc_result.and_then(|r| r.schema.as_ref()).and_then(parse_result_discriminator);

    let params: Vec<ParamDef> = if !method.x_bitcoin_arguments.is_empty() {
        method.x_bitcoin_arguments.into_iter().map(convert_argument).collect()
    } else {
        method.params.iter().map(convert_param_from_openrpc_value).collect()
    };

    let result = if !legacy_results.is_empty() {
        if legacy_results.len() == 1 {
            let schema = openrpc_result.and_then(|r| r.schema.as_ref());
            maybe_map_from_dynamic_object_schema(&method.name, &legacy_results[0], schema).or_else(
                || Some(convert_result(&legacy_results[0], None, Some(&method.name), schema)),
            )
        } else if openrpc_result
            .and_then(|r| r.schema.as_ref())
            .is_some_and(has_schema_discriminated_oneof)
        {
            Some(build_union_result_type(
                &legacy_results,
                &method.name,
                openrpc_result.and_then(|r| r.schema.as_ref()),
            ))
        } else {
            Some(merge_results_to_object(&legacy_results, &method.name))
        }
    } else if let Some(schema) = openrpc_result.and_then(|r| r.schema.as_ref()) {
        Some(convert_result_from_schema(schema, &method.name))
    } else {
        None
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
        result_discriminator,
    }
}

fn has_schema_oneof_branch_metadata(schema: &serde_json::Value) -> bool {
    schema
        .get("x-bitcoin-oneof-branches")
        .or_else(|| schema.get("x-bitcoin-oneOfBranchConditions"))
        .and_then(|v| v.as_array())
        .is_some_and(|branches| branches.len() >= 2)
}

fn has_schema_discriminated_oneof(schema: &serde_json::Value) -> bool {
    has_schema_oneof_branch_metadata(schema)
        && schema.get("x-bitcoin-discriminatedResult").is_some()
}

/// Builds a [`TypeKind::Union`] from parallel `x-bitcoin-results` or `object-one-of` branches.
fn build_union_from_raw_results(
    results: &[RawResult],
    method_name: &str,
    schema: Option<&serde_json::Value>,
    union_type_name: &str,
) -> TypeDef {
    let method_pascal = canonical_method_pascal(method_name);
    let branch_conditions: Vec<String> = schema
        .and_then(|s| {
            s.get("x-bitcoin-oneof-branches").or_else(|| s.get("x-bitcoin-oneOfBranchConditions"))
        })
        .and_then(|v| v.as_array())
        .map(|branches| {
            branches
                .iter()
                .map(|b| {
                    if let Some(s) = b.as_str() {
                        s.to_string()
                    } else if let Some(arr) = b.as_array() {
                        arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(" | ")
                    } else {
                        b.get("condition").and_then(|v| v.as_str()).unwrap_or("").to_string()
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let union_variants = results
        .iter()
        .enumerate()
        .map(|(idx, raw)| {
            let mut branch_type = convert_result(raw, None, Some(method_name), None);
            // Preserve oneOf branches while giving every anonymous object/array in the branch
            // a stable unique name. Without this, many methods collapse to repeated `Object`.
            let mut anon_counter = 0usize;
            uniquify_anonymous_types(&mut branch_type, &method_pascal, idx + 1, &mut anon_counter);

            UnionVariantDef {
                name: format!("Branch{}", idx + 1),
                description: raw.description.clone(),
                condition: branch_conditions.get(idx).cloned().filter(|s| !s.is_empty()).or_else(
                    || {
                        if raw.condition.is_empty() {
                            None
                        } else {
                            Some(raw.condition.clone())
                        }
                    },
                ),
                type_def: branch_type,
            }
        })
        .collect();

    TypeDef {
        name: union_type_name.to_string(),
        description: "Union result preserved from OpenRPC oneOf branches".to_string(),
        kind: TypeKind::Union,
        union_variants: Some(union_variants),
        protocol_type: Some("any".to_string()),
        ..Default::default()
    }
}

fn build_union_result_type(
    results: &[RawResult],
    method_name: &str,
    schema: Option<&serde_json::Value>,
) -> TypeDef {
    let name = format!("{}ResultUnion", canonical_method_pascal(method_name));
    build_union_from_raw_results(results, method_name, schema, &name)
}

fn uniquify_anonymous_types(
    td: &mut TypeDef,
    method_pascal: &str,
    branch_idx: usize,
    anon_counter: &mut usize,
) {
    let is_anon_object = td.name == "object" || td.name == "Object";
    let is_anon_array = td.name == "array" || td.name == "Array";
    if is_anon_object || is_anon_array {
        *anon_counter += 1;
        let kind = if is_anon_object { "Object" } else { "Array" };
        td.name = format!("{method_pascal}Branch{branch_idx}{kind}{anon_counter}");
    }

    if let Some(fields) = td.fields.as_mut() {
        for field in fields {
            uniquify_anonymous_types(
                &mut field.field_type,
                method_pascal,
                branch_idx,
                anon_counter,
            );
        }
    }
    if let Some(value) = td.map_value.as_mut() {
        uniquify_anonymous_types(value, method_pascal, branch_idx, anon_counter);
    }
    if let Some(uvs) = td.union_variants.as_mut() {
        for uv in uvs {
            uniquify_anonymous_types(&mut uv.type_def, method_pascal, branch_idx, anon_counter);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn getrawtransaction_schema_oneof_yields_union_not_merged_scaffold_keys() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../resources/ir/openrpc.json");
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let doc: OpenRpcDoc = serde_json::from_str(&content).expect("openrpc json");
        let method = doc
            .methods
            .iter()
            .find(|m| m.name == "getrawtransaction")
            .expect("getrawtransaction in openrpc");
        let rpc = convert_openrpc_method(method.clone(), Some("30".to_string()));
        assert!(!rpc.params.is_empty(), "schema-first params must be non-empty");
        let result = rpc.result.expect("getrawtransaction result");
        assert_eq!(result.kind, TypeKind::Union, "schema-first oneOf must lower to Union");
        let variants = result.union_variants.as_ref().expect("union variants");
        assert!(variants.len() >= 2);
        for bad in ["in_active_chain_1", "blockhash_1", "vin_1", "vout_1"] {
            for uv in variants {
                if let Some(fields) = uv.type_def.fields.as_ref() {
                    assert!(
                        !fields.iter().any(|f| f.key.as_ident() == bad),
                        "unexpected scaffold key {bad}"
                    );
                }
            }
        }
    }

    #[test]
    fn getblock_openrpc_yields_wire_plus_verbose_union_variants_without_merged_scaffold_keys() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../resources/ir/openrpc.json");
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let doc: OpenRpcDoc = serde_json::from_str(&content).expect("openrpc json");
        let method =
            doc.methods.iter().find(|m| m.name == "getblock").expect("getblock in openrpc");
        let rpc = convert_openrpc_method(method.clone(), Some("31".to_string()));
        assert_eq!(rpc.params.len(), 2);
        let result = rpc.result.expect("getblock result");
        assert_eq!(result.kind, TypeKind::Union);
        assert!(rpc.result_discriminator.is_some(), "getblock has x-bitcoin-discriminatedResult");
        let variants = result.union_variants.as_ref().expect("union variants");
        assert!(
            !variants.iter().any(|uv| {
                uv.type_def
                    .fields
                    .as_ref()
                    .is_some_and(|fields| fields.iter().any(|f| f.key.as_ident() == "tx_1"))
            }),
            "merged scaffold key tx_1 must not appear"
        );
    }

    #[test]
    fn top_level_array_methods_use_array_typedef_with_element_type() {
        let raw_elem = RawResult {
            r#type: "string".to_string(),
            optional: false,
            description: "the derived addresses".to_string(),
            skip_type_check: false,
            key_name: String::new(),
            condition: String::new(),
            inner: Vec::new(),
        };

        let raw_array = RawResult {
            r#type: "array".to_string(),
            optional: false,
            description: "list of derived addresses".to_string(),
            skip_type_check: false,
            key_name: String::new(),
            condition: String::new(),
            inner: vec![raw_elem],
        };

        let method = OpenRpcMethod {
            name: "deriveaddresses".to_string(),
            description: String::new(),
            params: Vec::new(),
            result: Some(OpenRpcResult {
                name: Some("result".to_string()),
                schema: None,
                x_bitcoin_results: vec![raw_array],
            }),
            x_bitcoin_category: "wallet".to_string(),
            x_bitcoin_examples: None,
            x_bitcoin_argument_names: Vec::new(),
            x_bitcoin_arguments: Vec::new(),
        };

        let rpc = convert_openrpc_method(method, Some("30".to_string()));
        let result_ty = rpc.result.expect("result type should be present");

        assert_eq!(result_ty.kind, TypeKind::Array);

        let elem_ty = result_ty
            .array_element_type()
            .expect("array element type should be discoverable via helper");

        assert_eq!(elem_ty.protocol_type.as_deref(), Some("string"));
    }

    #[test]
    fn distinct_top_level_array_shapes_are_mechanically_merged() {
        let str_elem = RawResult {
            r#type: "string".to_string(),
            optional: false,
            description: "address".to_string(),
            skip_type_check: false,
            key_name: "address".to_string(),
            condition: String::new(),
            inner: vec![],
        };
        let array_of_strings = RawResult {
            r#type: "array".to_string(),
            optional: false,
            description: "single-path".to_string(),
            skip_type_check: false,
            key_name: String::new(),
            condition: "for single-path".to_string(),
            inner: vec![str_elem.clone()],
        };
        let array_of_string_arrays = RawResult {
            r#type: "array".to_string(),
            optional: false,
            description: "multipath".to_string(),
            skip_type_check: false,
            key_name: String::new(),
            condition: "for multipath".to_string(),
            inner: vec![RawResult {
                r#type: "array".to_string(),
                optional: false,
                description: String::new(),
                skip_type_check: false,
                key_name: String::new(),
                condition: String::new(),
                inner: vec![str_elem],
            }],
        };

        let method = OpenRpcMethod {
            name: "deriveaddresses".to_string(),
            description: String::new(),
            params: Vec::new(),
            result: Some(OpenRpcResult {
                name: Some("result".to_string()),
                schema: None,
                x_bitcoin_results: vec![array_of_strings, array_of_string_arrays],
            }),
            x_bitcoin_category: "wallet".to_string(),
            x_bitcoin_examples: None,
            x_bitcoin_argument_names: Vec::new(),
            x_bitcoin_arguments: Vec::new(),
        };

        let rpc = convert_openrpc_method(method, Some("30".to_string()));
        let result_ty = rpc.result.expect("result type should be present");
        assert_eq!(result_ty.kind, TypeKind::Object);
    }

    #[test]
    fn getnetworkinfo_networks_and_localaddresses_use_distinct_element_types() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let openrpc_path = manifest.join("../resources/ir/openrpc.json");
        let file = std::fs::File::open(&openrpc_path)
            .unwrap_or_else(|e| panic!("open {}: {e}", openrpc_path.display()));
        let doc: OpenRpcDoc = serde_json::from_reader(file).expect("parse openrpc.json");
        let method = doc
            .methods
            .iter()
            .find(|m| m.name == "getnetworkinfo")
            .expect("getnetworkinfo present in openrpc.json");
        let rpc = convert_openrpc_method(method.clone(), Some("30".to_string()));
        let result_ty = rpc.result.expect("getnetworkinfo result");
        let fields = result_ty.fields.as_ref().expect("object result");
        let networks =
            fields.iter().find(|f| f.key.as_ident() == "networks").expect("networks field");
        let locals = fields
            .iter()
            .find(|f| f.key.as_ident() == "localaddresses")
            .expect("localaddresses field");
        let net_el = networks.field_type.array_element_type().expect("networks is array");
        let loc_el = locals.field_type.array_element_type().expect("localaddresses is array");
        assert_ne!(
            net_el.name, loc_el.name,
            "network row vs local address row must not share one IR type name"
        );
        assert_eq!(net_el.name, "GetNetworkInfoNetworks");
        assert_eq!(loc_el.name, "GetNetworkInfoLocaladdresses");
        assert!(net_el.type_identity.is_some());
        assert!(loc_el.type_identity.is_some());
    }

    #[test]
    fn getrawmempool_schema_oneof_yields_union_with_sequence_branch() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let openrpc_path = manifest.join("../resources/ir/openrpc.json");
        let file = std::fs::File::open(&openrpc_path)
            .unwrap_or_else(|e| panic!("open {}: {e}", openrpc_path.display()));
        let doc: OpenRpcDoc = serde_json::from_reader(file).expect("parse openrpc.json");
        let method = doc
            .methods
            .iter()
            .find(|m| m.name == "getrawmempool")
            .expect("getrawmempool present in openrpc.json");
        let rpc = convert_openrpc_method(method.clone(), Some("30".to_string()));
        let result_ty = rpc.result.expect("getrawmempool result");
        assert_eq!(result_ty.kind, TypeKind::Union);
        let variants = result_ty.union_variants.as_ref().expect("union variants");
        assert!(
            variants.iter().any(|uv| {
                uv.type_def.fields.as_ref().is_some_and(|fields| {
                    fields.iter().any(|f| f.key.as_ident() == "mempool_sequence")
                })
            }),
            "oneOf branches should preserve mempool_sequence key"
        );
    }

    #[test]
    fn getbalance_amount_unit_maps_to_protocol_amount() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../resources/ir/openrpc.json");
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let doc: OpenRpcDoc = serde_json::from_str(&content).expect("openrpc json");
        let method =
            doc.methods.iter().find(|m| m.name == "getbalance").expect("getbalance in openrpc");
        let rpc = convert_openrpc_method(method.clone(), Some("31".to_string()));
        let ty = rpc.result.expect("getbalance result");
        assert_eq!(ty.protocol_type.as_deref(), Some("amount"));
        assert!(!rpc.params.is_empty());
    }

    #[test]
    fn rpc_discover_and_getopenrpcinfo_present_with_schema_results() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../resources/ir/openrpc.json");
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let doc: OpenRpcDoc = serde_json::from_str(&content).expect("openrpc json");
        for name in ["rpc.discover", "getopenrpcinfo"] {
            let method = doc
                .methods
                .iter()
                .find(|m| m.name == name)
                .unwrap_or_else(|| panic!("{name} missing from openrpc.json"));
            let rpc = convert_openrpc_method(method.clone(), Some("31".to_string()));
            let ty = rpc.result.expect("discover result");
            assert_eq!(ty.kind, TypeKind::Object);
            assert!(
                ty.fields.as_ref().is_some_and(|f| !f.is_empty()),
                "{name} result object must have fields"
            );
        }
        let info = doc.methods.iter().find(|m| m.name == "getopenrpcinfo").expect("getopenrpcinfo");
        let rpc = convert_openrpc_method(info.clone(), Some("31".to_string()));
        assert!(rpc.params.iter().any(|p| p.name == "show_hidden"));
    }

    #[test]
    fn schema_oneof_branch_preservation_avoids_anonymous_object_names() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../resources/ir/openrpc.json");
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let doc: OpenRpcDoc = serde_json::from_str(&content).expect("openrpc json");

        for method in &doc.methods {
            let Some(result) = &method.result else { continue };
            let Some(schema) = result.schema.as_ref() else {
                continue;
            };
            if !has_schema_oneof_branch_metadata(schema) {
                continue;
            }
            let rpc = convert_openrpc_method(method.clone(), Some("30".to_string()));
            let result_ty = rpc.result.expect("result type");
            if result_ty.kind != TypeKind::Union {
                continue;
            }
            let variants = result_ty.union_variants.as_ref().expect("union variants");
            for uv in variants {
                let label = uv.type_def.rust_emit_name();
                assert!(
                    label != "object" && label != "Object" && label != "array" && label != "Array",
                    "oneOf-preserved method {} still has anonymous branch type label {}",
                    method.name,
                    label
                );
            }
        }
    }

    #[test]
    fn logging_dynamic_object_result_is_lowered_to_map_type() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../resources/ir/openrpc.json");
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let doc: OpenRpcDoc = serde_json::from_str(&content).expect("openrpc json");
        let method = doc.methods.iter().find(|m| m.name == "logging").expect("logging method");
        let rpc = convert_openrpc_method(method.clone(), Some("30".to_string()));
        let ty = rpc.result.expect("logging result");
        assert_eq!(ty.kind, TypeKind::Map, "logging should produce a typed dynamic map");
        assert_eq!(ty.protocol_type.as_deref(), Some("object-dynamic"));
        let mv = ty.map_value_type().expect("map value type");
        assert_eq!(mv.protocol_type.as_deref(), Some("boolean"));
    }

    #[test]
    fn getaddressesbylabel_dynamic_object_value_is_typed_object() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../resources/ir/openrpc.json");
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let doc: OpenRpcDoc = serde_json::from_str(&content).expect("openrpc json");
        let method = doc
            .methods
            .iter()
            .find(|m| m.name == "getaddressesbylabel")
            .expect("getaddressesbylabel method");
        let rpc = convert_openrpc_method(method.clone(), Some("30".to_string()));
        let ty = rpc.result.expect("getaddressesbylabel result");
        assert_eq!(ty.kind, TypeKind::Map, "must lower to map");
        let mv = ty.map_value_type().expect("map value");
        assert_eq!(mv.kind, TypeKind::Object, "value must be typed object");
        let fields = mv.fields.as_ref().expect("value object fields");
        assert!(
            fields.iter().any(|f| f.key.as_ident() == "purpose"),
            "typed map value should include purpose field"
        );
    }

    #[test]
    fn discriminated_mempool_methods_stay_union_typed() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../resources/ir/openrpc.json");
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let doc: OpenRpcDoc = serde_json::from_str(&content).expect("openrpc json");
        for method_name in ["getrawmempool", "getmempoolancestors", "getmempooldescendants"] {
            let method =
                doc.methods.iter().find(|m| m.name == method_name).expect("method in openrpc");
            let rpc = convert_openrpc_method(method.clone(), Some("30".to_string()));
            let ty = rpc.result.expect("result type");
            assert_eq!(ty.kind, TypeKind::Union, "{method_name} must remain a union");
            assert!(
                ty.union_variants.as_ref().is_some_and(|v| !v.is_empty()),
                "{method_name} union must have at least one variant"
            );
        }
    }

    #[test]
    fn scan_action_methods_stay_union_typed() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../resources/ir/openrpc.json");
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let doc: OpenRpcDoc = serde_json::from_str(&content).expect("openrpc json");
        for method_name in ["scanblocks", "scantxoutset"] {
            let method =
                doc.methods.iter().find(|m| m.name == method_name).expect("method in openrpc");
            let rpc = convert_openrpc_method(method.clone(), Some("30".to_string()));
            let ty = rpc.result.expect("result type");
            assert_eq!(ty.kind, TypeKind::Union, "{method_name} must remain a union");
        }
    }

    #[test]
    fn integer_param_names_cover_core_integer_domains() {
        assert!(is_integerish_name("height"));
        assert!(is_integerish_name("verbosity"));
        assert!(is_integerish_name("conf_target"));
        assert!(is_integerish_name("start_height"));
        assert!(!is_integerish_name("fee_rate"));
        assert!(INTEGER_PARAM_NAMES.contains(&"vout"));
    }

    #[test]
    fn adapter_assigns_type_identity_for_non_primitives() {
        let raw = RawResult {
            r#type: "object".to_string(),
            optional: false,
            description: "example".to_string(),
            skip_type_check: false,
            key_name: String::new(),
            condition: "for test".to_string(),
            inner: vec![RawResult {
                r#type: "string".to_string(),
                optional: false,
                description: "name".to_string(),
                skip_type_check: false,
                key_name: "name".to_string(),
                condition: String::new(),
                inner: vec![],
            }],
        };
        let td = convert_result(&raw, Some("result"), Some("examplemethod"), None);
        assert!(
            td.type_identity.as_ref().is_some_and(|s| !s.is_empty()),
            "object/array/union/map typedefs should carry a stable type_identity"
        );
    }

    #[test]
    fn no_rpc_name_specific_conditionals_in_openrpc_adapter() {
        let src = include_str!("openrpc.rs");
        let forbidden = [
            format!("if {} == ", "method.name"),
            format!("match {} {{", "method.name.as_str()"),
            format!("if {} == ", "rpc.name"),
        ];
        for needle in forbidden {
            assert!(
                !src.contains(&needle),
                "openrpc adapter must stay mechanical; found forbidden conditional pattern: {needle}"
            );
        }
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

    let mut ir = ProtocolIR::new(vec![module]);
    super::openrpc_type_disambiguation::disambiguate_conflated_type_names(&mut ir);
    ir
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
