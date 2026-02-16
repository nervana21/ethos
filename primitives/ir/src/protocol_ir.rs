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
    /// Example usage strings for the method (preserved from raw schema)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<Vec<String>>,
    /// Whether this method is hidden from documentation (preserved from raw schema)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Condition under which this type/field is present (preserved from raw schema)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
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
}

/// Field definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDef {
    /// Field name
    pub name: String,
    /// Field type
    pub field_type: TypeDef,
    /// Whether this field is required
    pub required: bool,
    /// Field description
    pub description: String,
    /// Default value (if any)
    pub default_value: Option<String>,
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

/// Type kinds
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TypeKind {
    /// Primitive type (string, number, boolean, etc.)
    Primitive,
    /// Object type (struct)
    Object,
    /// Enum type
    Enum,
    /// Array type
    Array,
    /// Optional type
    Optional,
    /// Dialect-specific or adapter-defined type with concrete implementation (e.g. HashOrHeight enum)
    Custom,
    /// Type alias
    Alias,
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
            let module = ProtocolModule::from_source(&name, &desc, defs, "merged");
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
    pub fn from_source(
        name: &str,
        desc: &str,
        defs: Vec<ProtocolDef>,
        _source: &'static str,
    ) -> Self {
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
