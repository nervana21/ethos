//! Ethos Intermediate Representation
//!
//! This module defines the core IR structures that represent
//! Bitcoin protocol dialects (e.g. Bitcoin Core).
//! LND, etc. are some of these dialects.

use serde::{Deserialize, Serialize};

/// The Ethos IR - represents the canonical protocol meaning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolIR {
    /// Ethos protocol specification version (e.g., "0.1.0")
    version: String,
    /// Protocol modules (RPC, P2P, PSBT, etc.)
    modules: Vec<ProtocolModule>,
}

/// A module within the protocol (e.g., RPC, P2P, PSBT)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolModule {
    /// Module name (e.g., "rpc", "p2p", "psbt")
    name: String,
    /// Module description
    description: String,
    /// Protocol definitions in this module
    definitions: Vec<ProtocolDef>,
}

/// A definition within the protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProtocolDef {
    /// RPC method definition
    RpcMethod(RpcDef),
    /// Network message definition
    Message(MessageDef),
    /// Type definition
    Type(TypeDef),
    /// Constant definition
    Constant(ConstantDef),
}

/// When an RPC result is a `oneOf` keyed by a parameter (e.g. `verbosity`), as in Bitcoin Core OpenRPC
/// `x-bitcoin-discriminated-result`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RpcResultDiscriminator {
    /// Primary parameter name (may be a pipe-joined alias in Core docs, e.g. `verbosity|verbose`).
    pub parameter: String,
    /// Zero-based index into JSON-RPC `params`.
    #[serde(rename = "parameterIndex")]
    pub parameter_index: u32,
    /// Discriminant values; parallel to `oneOf` arms where Core provides them (int, string, or JSON null).
    pub values: Vec<serde_json::Value>,
    /// When the discriminant is nested inside an object parameter (e.g. `template_request.mode`).
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "nestedParameterKey")]
    pub nested_parameter_key: Option<String>,
}

/// RPC method definition
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RpcDef {
    /// Method name
    pub name: String,
    /// Method description
    pub description: String,
    /// Method parameters
    pub params: Vec<ParamDef>,
    /// Method return type
    pub result: Option<TypeDef>,
    /// Method category - protocol-specific domain classification (e.g., "wallet", "mining", "channel")
    /// This field is populated by protocol adapters from their schema definitions.
    pub category: String,
    /// Method access level / visibility tier
    #[serde(default)]
    pub access_level: AccessLevel,
    /// Whether this method requires private key access
    pub requires_private_keys: bool,
    /// Version when this method was first added/supported
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_added: Option<String>,
    /// Version when this method was last supported (None if still supported)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_removed: Option<String>,
    /// Example usage strings for the method (preserved from schema)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<Vec<String>>,
    /// Whether this method is hidden from documentation (preserved from schema)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    /// Structured linkage when the result is a discriminated `oneOf` (OpenRPC extension).
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "result_discriminator")]
    pub result_discriminator: Option<RpcResultDiscriminator>,
}

/// Network message definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDef {
    /// Message name
    pub name: String,
    /// Message description
    pub description: String,
    /// Message fields
    pub fields: Vec<FieldDef>,
    /// Message type (e.g., "request", "response", "notification")
    pub message_type: MessageType,
    /// Message version
    pub version: Option<String>,
}

/// Type definition
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TypeDef {
    /// Rust-mapped type name (e.g., "String", "i64", "u64")
    ///
    /// This is the final Rust type name after protocol-specific type mapping.
    /// When accessed via `param.param_type.name`, this represents the Rust type,
    /// not the parameter name or protocol primitive type.
    pub name: String,
    /// Type description
    pub description: String,
    /// Type kind (struct, enum, primitive, etc.)
    pub kind: TypeKind,
    /// Type fields (for structs)
    pub fields: Option<Vec<FieldDef>>,
    /// Type variants (for enums)
    pub variants: Option<Vec<VariantDef>>,
    /// Union/one-of variants (for TypeKind::Union)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub union_variants: Option<Vec<UnionVariantDef>>,
    /// Base type (for type aliases)
    pub base_type: Option<String>,
    /// Protocol primitive type identifier (e.g., "string", "number", "boolean", "hex")
    ///
    /// This preserves the protocol's native primitive type identifier before Rust mapping.
    /// Used by adapters to map protocol types to Rust types (e.g., "number" → "i64", "hex" → "String").
    /// This is the IR-based representation of the protocol's type system.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol_type: Option<String>,
    /// Canonical name for this type if it is an alias or duplicate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_name: Option<String>,
    /// When set, overrides [`Self::name`] for Rust struct emission and type-registry keys in codegen.
    ///
    /// Used when OpenRPC reuses one component title for different JSON shapes (e.g. multiple
    /// `DecodePsbtRow` objects). Optional for backward compatibility with older IR files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_identity: Option<String>,
    /// Condition under which this type/field is present (preserved from schema)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    /// Homogeneous JSON object with dynamic keys mapping to `map_value` (for `TypeKind::Map`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub map_value: Option<Box<TypeDef>>,
    /// Protocol type for map keys before Rust mapping (e.g. `"hex"` for transaction ids).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub map_key_protocol_type: Option<String>,
}

impl TypeDef {
    /// If this type represents a homogeneous JSON array, return the element type.
    ///
    /// Convention:
    /// - `kind == TypeKind::Array`
    /// - `fields` contains a single entry that serves as the element prototype
    /// - The field key may be either:
    ///   - `FieldKey::Anonymous(0)` (preferred positional form), or
    ///   - `FieldKey::Named("field_0")` (legacy/synthetic name).
    ///
    /// Callers should use this helper instead of matching on `FieldKey` directly
    /// so IR producers can evolve without breaking consumers.
    pub fn array_element_type(&self) -> Option<&TypeDef> {
        if !matches!(self.kind, TypeKind::Array) {
            return None;
        }
        let fields = self.fields.as_ref()?;
        if fields.len() != 1 {
            return None;
        }
        let field = &fields[0];

        let is_positional_zero = field.key.is_positional_zero()
            || matches!(field.key, FieldKey::Named(ref s) if s == "field_0");

        if !is_positional_zero {
            return None;
        }

        Some(&field.field_type)
    }

    /// When an array encodes OpenRPC `prefixItems` as multiple anonymous fields
    /// that all share the same element type (e.g. fixed-size numeric tuples like
    /// `feerate_percentiles`), returns that shared element type. Heterogeneous
    /// tuples return `None`.
    pub fn homogeneous_array_element_type(&self) -> Option<&TypeDef> {
        if !matches!(self.kind, TypeKind::Array) {
            return None;
        }
        if let Some(elem) = self.array_element_type() {
            return Some(elem);
        }
        let fields = self.fields.as_ref()?;
        if fields.is_empty() || !fields.iter().all(|f| f.key.is_anonymous()) {
            return None;
        }
        let first = &fields[0].field_type;
        if fields.iter().skip(1).all(|f| Self::array_element_types_equivalent(&f.field_type, first))
        {
            Some(first)
        } else {
            None
        }
    }

    /// OpenRPC `prefixItems` encoded as multiple anonymous fields with differing
    /// element types (e.g. `listaddressgroupings` address/amount/account rows).
    pub fn prefix_items_tuple_fields(&self) -> Option<&[FieldDef]> {
        if !matches!(self.kind, TypeKind::Array) {
            return None;
        }
        if self.array_element_type().is_some() {
            return None;
        }
        let fields = self.fields.as_ref()?;
        if fields.len() < 2 || !fields.iter().all(|f| f.key.is_anonymous()) {
            return None;
        }
        let first = &fields[0].field_type;
        if fields.iter().skip(1).all(|f| Self::array_element_types_equivalent(&f.field_type, first))
        {
            return None;
        }
        Some(fields.as_slice())
    }

    fn array_element_types_equivalent(a: &TypeDef, b: &TypeDef) -> bool {
        if a.kind != b.kind {
            return false;
        }
        match a.kind {
            TypeKind::Primitive => a.protocol_type == b.protocol_type,
            TypeKind::Object
            | TypeKind::Map
            | TypeKind::Union
            | TypeKind::Enum
            | TypeKind::Alias
            | TypeKind::Custom => a.rust_emit_name() == b.rust_emit_name(),
            TypeKind::Array =>
                match (a.homogeneous_array_element_type(), b.homogeneous_array_element_type()) {
                    (Some(ae), Some(be)) => Self::array_element_types_equivalent(ae, be),
                    _ => false,
                },
            TypeKind::Optional => false,
        }
    }

    /// Value type for a `TypeKind::Map` (dynamic JSON object keys).
    pub fn map_value_type(&self) -> Option<&TypeDef> {
        if !matches!(self.kind, TypeKind::Map) {
            return None;
        }
        self.map_value.as_deref()
    }

    /// Name used for Rust struct emission and codegen type-registry keys.
    ///
    /// Returns [`Self::type_identity`] when set, otherwise [`Self::name`].
    pub fn rust_emit_name(&self) -> &str {
        self.type_identity.as_deref().unwrap_or(self.name.as_str())
    }
}

/// Constant definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstantDef {
    /// Constant name
    pub name: String,
    /// Constant value
    pub value: String,
    /// Constant type
    pub const_type: String,
    /// Constant description
    pub description: String,
}

/// Parameter definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamDef {
    /// Parameter name
    pub name: String,
    /// Parameter type
    pub param_type: TypeDef,
    /// Whether this parameter is required
    pub required: bool,
    /// Parameter description
    pub description: String,
    /// Default value (if any)
    pub default_value: Option<String>,
    /// Version when this parameter was added (None = available in all versions / unknown)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_added: Option<String>,
    /// Version when this parameter was removed (None = still present)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_removed: Option<String>,
}

/// Identity of a field: either a named key (e.g. JSON key) or an anonymous
/// ordinal (position in the parent's `fields` array).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FieldKey {
    /// Named field (e.g. JSON object key).
    Named(String),
    /// Anonymous field identified by position (index in the parent's `fields`).
    Anonymous(usize),
}

impl FieldKey {
    /// Returns the string used for Rust identifiers and display (for example,
    /// codegen and diagnostics). `Named(s) => s`, `Anonymous(i) =>
    /// "field_{i}"`.
    pub fn as_ident(&self) -> String {
        match self {
            FieldKey::Named(s) => s.clone(),
            FieldKey::Anonymous(i) => format!("field_{i}"),
        }
    }

    /// Returns the JSON key for this field when present. Named fields have a
    /// key; anonymous fields do not.
    pub fn json_key(&self) -> Option<&str> {
        match self {
            FieldKey::Named(s) => Some(s.as_str()),
            FieldKey::Anonymous(_) => None,
        }
    }

    // Helpers below: prefer these over matching on the enum so the abstraction
    // can be replaced later (e.g. with a plain string) without changing call sites.

    /// Returns true for anonymous (positional) fields, false for named.
    pub fn is_anonymous(&self) -> bool { matches!(self, FieldKey::Anonymous(_)) }

    /// Returns the ordinal for anonymous fields; `None` for named.
    pub fn anonymous_index(&self) -> Option<usize> {
        match self {
            FieldKey::Anonymous(i) => Some(*i),
            FieldKey::Named(_) => None,
        }
    }

    /// Returns true iff this key represents the first positional element (index 0).
    ///
    /// This is a convenience for code that needs to reason about array element
    /// prototypes without matching on the enum directly.
    pub fn is_positional_zero(&self) -> bool { self.anonymous_index() == Some(0) }

    /// Trims the name in place for named fields; no-op for anonymous.
    pub fn trim_named_in_place(&mut self) {
        if let FieldKey::Named(s) = self {
            *s = s.trim().to_string();
        }
    }

    /// Resolves this key against a slice of fields: by index for anonymous, by
    /// name (with index fallback) for named.
    pub fn find_in<'a>(
        &self,
        fields: &'a [FieldDef],
        fallback_index: usize,
    ) -> Option<&'a FieldDef> {
        match self {
            FieldKey::Anonymous(i) => fields.get(*i),
            FieldKey::Named(s) => fields
                .iter()
                .find(|e| e.key.json_key() == Some(s.as_str()))
                .or_else(|| fields.get(fallback_index)),
        }
    }
}

/// Field definition
#[derive(Debug, Clone, Serialize)]
pub struct FieldDef {
    /// Field identity (named or anonymous by ordinal).
    pub key: FieldKey,
    /// Field type
    pub field_type: TypeDef,
    /// Whether this field is required
    pub required: bool,
    /// Field description
    pub description: String,
    /// Default value (if any)
    pub default_value: Option<String>,
    /// Version when this field was added (None = available in all versions / unknown)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_added: Option<String>,
    /// Version when this field was removed (None = still present)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_removed: Option<String>,
    /// When `Some(false)`, this field is omitted from generated response structs (OpenRPC verbosity
    /// scaffolding that is not a separate JSON key on the wire). `None` means emit (default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emit_in_struct: Option<bool>,
    /// When `true`, generated Rust uses `Option<T>` even if `required` is true (Core may omit).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force_optional: Option<bool>,
}

// Custom deserializer to support both legacy IR field shape (with `key`) and
// the newer shape (with `name`).
impl<'de> Deserialize<'de> for FieldDef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct FieldDefHelper {
            #[serde(default)]
            name: Option<String>,
            #[serde(default)]
            key: Option<FieldKeyHelper>,
            field_type: TypeDef,
            required: bool,
            description: String,
            #[serde(default)]
            default_value: Option<String>,
            #[serde(default)]
            version_added: Option<String>,
            #[serde(default)]
            version_removed: Option<String>,
            #[serde(default)]
            emit_in_struct: Option<bool>,
            #[serde(default)]
            force_optional: Option<bool>,
        }

        #[derive(Deserialize)]
        #[serde(untagged)]
        enum FieldKeyHelper {
            /// Legacy IR encoded the field name under a `Named` variant.
            NamedVariant {
                #[serde(rename = "Named")]
                named: String,
            },
            /// IR encodes anonymous fields under an `Anonymous` variant.
            AnonymousVariant {
                #[serde(rename = "Anonymous")]
                anonymous: usize,
            },
            /// Fallback: accept a plain string or any other representation.
            Other(serde_json::Value),
        }

        let helper = FieldDefHelper::deserialize(deserializer)?;

        // Prefer explicit `key` when present; fall back to legacy `name` field.
        let mut key = if let Some(key) = helper.key {
            match key {
                FieldKeyHelper::NamedVariant { named } => FieldKey::Named(named),
                FieldKeyHelper::AnonymousVariant { anonymous } => FieldKey::Anonymous(anonymous),
                FieldKeyHelper::Other(v) => v
                    .as_str()
                    .map(|s| FieldKey::Named(s.to_string()))
                    // Fallback: accept any non-string key as an unnamed field.
                    .unwrap_or(FieldKey::Named("<unnamed>".to_string())),
            }
        } else if let Some(name) = helper.name {
            FieldKey::Named(name)
        } else {
            // Legacy IR may omit both `name` and `key` in rare cases; synthesize a placeholder.
            FieldKey::Named("<unnamed>".to_string())
        };

        // Normalize named keys to trimmed form.
        key.trim_named_in_place();

        Ok(FieldDef {
            key,
            field_type: helper.field_type,
            required: helper.required,
            description: helper.description,
            default_value: helper.default_value,
            version_added: helper.version_added,
            version_removed: helper.version_removed,
            emit_in_struct: helper.emit_in_struct,
            force_optional: helper.force_optional,
        })
    }
}

/// Variant definition for enums
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantDef {
    /// Variant name
    pub name: String,
    /// Variant description
    pub description: String,
    /// Variant value (if any)
    pub value: Option<String>,
    /// Associated data (if any)
    pub associated_data: Option<Vec<FieldDef>>,
}

/// Variant definition for union/one-of types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnionVariantDef {
    /// Variant name (e.g. "array", "object", or a semantic label).
    pub name: String,
    /// Variant description.
    pub description: String,
    /// Condition under which this variant is present (e.g. "for verbose = false").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    /// Variant type definition.
    pub type_def: TypeDef,
}

/// Type kinds
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum TypeKind {
    /// Primitive type (string, number, boolean, etc.)
    #[default]
    Primitive,
    /// Object type (struct)
    Object,
    /// Array type
    Array,
    /// Enum type
    Enum,
    /// Optional type
    Optional,
    /// Type alias
    Alias,
    /// Dialect-specific or adapter-defined type with concrete implementation (e.g. HashOrHeight enum)
    Custom,
    /// Union / one-of type composed of multiple variants.
    Union,
    /// JSON object with dynamic keys and homogeneous values (e.g. mempool txid → entry).
    Map,
}

/// Message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    /// Request message
    Request,
    /// Response message
    Response,
    /// Notification message
    Notification,
    /// Error message
    Error,
}

/// Method access level - indicates intended use and operational risk
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum AccessLevel {
    /// Standard public API methods for general use
    #[default]
    Public,
    /// Testing and development methods (regtest/testnet)
    Testing,
    /// Internal debugging and diagnostic methods
    Internal,
    /// Advanced operations that require caution to use
    Advanced,
}

impl ProtocolIR {
    /// Create a new Protocol IR with the default Ethos protocol version
    pub fn new(modules: Vec<ProtocolModule>) -> Self {
        Self::new_with_version("0.1.0".to_string(), modules)
    }

    /// Load ProtocolIR from a JSON file
    pub fn from_file(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let ir: Self = serde_json::from_str(&content)?;
        Ok(ir)
    }

    /// Save ProtocolIR to a JSON file with pretty formatting
    pub fn to_file(&self, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::File::create(path)?;
        serde_json::to_writer_pretty(&mut file, self)?;
        // Ensure file ends with a newline (POSIX standard)
        use std::io::Write;
        writeln!(file)?;
        Ok(())
    }

    /// Create a new Protocol IR with a specific Ethos protocol version
    pub fn new_with_version(version: String, modules: Vec<ProtocolModule>) -> Self {
        Self { version, modules }
    }

    /// Get a specific module by name
    pub fn get_module(&self, name: &str) -> Option<&ProtocolModule> {
        self.modules.iter().find(|m| m.name == name)
    }

    /// Get all RPC methods across all modules
    pub fn get_rpc_methods(&self) -> Vec<&RpcDef> {
        self.modules
            .iter()
            .flat_map(|m| m.definitions.iter())
            .filter_map(|def| match def {
                ProtocolDef::RpcMethod(rpc) => Some(rpc),
                _ => None,
            })
            .collect()
    }

    /// Get all type definitions across all modules
    pub fn get_type_definitions(&self) -> Vec<&TypeDef> {
        self.modules
            .iter()
            .flat_map(|m| m.definitions.iter())
            .filter_map(|def| match def {
                ProtocolDef::Type(ty) => Some(ty),
                _ => None,
            })
            .collect()
    }

    /// Get the Ethos protocol specification version
    pub fn version(&self) -> &str { &self.version }

    /// Get all modules in this protocol
    pub fn modules(&self) -> &[ProtocolModule] { &self.modules }

    /// Get mutable reference to all modules in this protocol
    pub fn modules_mut(&mut self) -> &mut Vec<ProtocolModule> { &mut self.modules }

    /// Get the protocol name
    pub fn name(&self) -> &'static str { "Ethos Protocol" }

    /// Get the protocol description
    pub fn description(&self) -> &'static str { "The canonical Ethos protocol specification" }

    /// Get the total number of definitions across all modules
    pub fn definition_count(&self) -> usize {
        self.modules.iter().map(|m| m.definitions.len()).sum()
    }

    /// Removes RPC method definitions that are marked `hidden == Some(true)`.
    /// Non-RPC definitions and RPCs with `hidden != Some(true)` are left unchanged.
    pub fn strip_hidden_rpcs(&mut self) {
        for module in self.modules_mut() {
            module.definitions_mut().retain(|def| match def {
                ProtocolDef::RpcMethod(rpc) => rpc.hidden != Some(true),
                _ => true,
            });
        }
    }

    /// Merge multiple ProtocolIRs into a single canonical IR
    ///
    /// This method performs merging by:
    /// - Deduplicating definitions with the same name
    /// - Preserving source attribution
    /// - Maintaining deterministic ordering
    ///
    /// # Arguments
    ///
    /// * `irs` - Vector of protocol IRs to merge
    ///
    /// # Returns
    ///
    /// Returns the merged protocol IR
    pub fn merge(irs: Vec<Self>) -> Self {
        use std::collections::BTreeMap;

        if irs.is_empty() {
            return Self::new_with_version("empty".to_string(), vec![]);
        }

        if irs.len() == 1 {
            return irs.into_iter().next().expect("IRS should not be empty");
        }

        // Group modules by name
        let mut by_module: BTreeMap<String, Vec<ProtocolModule>> = BTreeMap::new();
        for ir in irs {
            for m in ir.modules() {
                by_module.entry(m.name().to_string()).or_default().push(m.clone());
            }
        }

        let mut merged_modules = Vec::new();
        for (name, group) in by_module {
            let mut rpc_by_name: BTreeMap<String, RpcDef> = BTreeMap::new();
            let mut types_by_name: BTreeMap<String, TypeDef> = BTreeMap::new();
            let mut desc = String::new();

            for m in group {
                desc = if desc.is_empty() { m.description().to_string() } else { desc };
                for def in m.definitions() {
                    match def {
                        ProtocolDef::RpcMethod(r) => {
                            rpc_by_name.entry(r.name.clone()).or_insert(r.clone());
                        }
                        ProtocolDef::Type(t) => {
                            types_by_name.entry(t.name.clone()).or_insert(t.clone());
                        }
                        _other => { /* keep or bucket as-is if needed */ }
                    }
                }
            }

            let mut defs = Vec::with_capacity(rpc_by_name.len() + types_by_name.len());
            defs.extend(rpc_by_name.into_values().map(ProtocolDef::RpcMethod));
            defs.extend(types_by_name.into_values().map(ProtocolDef::Type));

            // Create merged module
            let module = ProtocolModule::from_source(&name, &desc, defs);
            merged_modules.push(module);
        }

        Self::new_with_version("merged".into(), merged_modules)
    }
}

impl ProtocolModule {
    /// Create a new protocol module
    pub fn new(name: String, description: String, definitions: Vec<ProtocolDef>) -> Self {
        Self { name, description, definitions }
    }

    /// Get RPC methods in this module
    pub fn get_rpc_methods(&self) -> Vec<&RpcDef> {
        self.definitions
            .iter()
            .filter_map(|def| match def {
                ProtocolDef::RpcMethod(rpc) => Some(rpc),
                _ => None,
            })
            .collect()
    }

    /// Get type definitions in this module
    pub fn get_type_definitions(&self) -> Vec<&TypeDef> {
        self.definitions
            .iter()
            .filter_map(|def| match def {
                ProtocolDef::Type(ty) => Some(ty),
                _ => None,
            })
            .collect()
    }

    /// Get the module name
    pub fn name(&self) -> &str { &self.name }

    /// Get the module description
    pub fn description(&self) -> &str { &self.description }

    /// Get all definitions in this module
    pub fn definitions(&self) -> &[ProtocolDef] { &self.definitions }

    /// Get mutable reference to all definitions in this module
    pub fn definitions_mut(&mut self) -> &mut Vec<ProtocolDef> { &mut self.definitions }

    /// Create a new protocol module
    pub fn from_source(name: &str, desc: &str, defs: Vec<ProtocolDef>) -> Self {
        ProtocolModule::new(name.to_string(), desc.to_string(), defs)
    }
}

impl RpcDef {
    /// Check if this RPC method has a structured response
    pub fn has_structured_response(&self) -> bool {
        self.result.is_some() && self.result.as_ref().map(|r| r.name.as_str()) != Some("none")
    }

    /// Get the result type name if available
    pub fn result_type_name(&self) -> Option<&str> { self.result.as_ref().map(|r| r.name.as_str()) }
}
