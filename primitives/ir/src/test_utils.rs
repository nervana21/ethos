//! Test helpers for building IR values in tests.
//!
//! Available when the `test-utils` feature is enabled.

use crate::protocol_ir::{ParamDef, ProtocolDef, ProtocolModule, RpcDef, TypeDef, TypeKind};

/// Builds a type definition with the given name and kind; other fields are empty/default.
pub fn type_def(name: &str, kind: TypeKind) -> TypeDef {
    TypeDef {
        name: name.to_string(),
        description: String::new(),
        kind,
        fields: None,
        variants: None,
        union_variants: None,
        base_type: None,
        protocol_type: None,
        canonical_name: None,
        condition: None,
    }
}

/// Builds a primitive type (e.g. "string", "hex", "none"); optional protocol_type for the IR protocol primitive.
pub fn primitive_type(name: &str, protocol_type: Option<String>) -> TypeDef {
    TypeDef {
        name: name.to_string(),
        description: String::new(),
        kind: TypeKind::Primitive,
        fields: None,
        variants: None,
        union_variants: None,
        base_type: None,
        protocol_type,
        canonical_name: None,
        condition: None,
    }
}

/// Builds a parameter definition.
pub fn param(name: &str, param_type: TypeDef, required: bool) -> ParamDef {
    ParamDef {
        name: name.to_string(),
        param_type,
        required,
        description: String::new(),
        default_value: None,
        version_added: None,
        version_removed: None,
    }
}

/// Builds a minimal RPC method definition.
pub fn rpc(name: &str, params: Vec<ParamDef>, result: Option<TypeDef>, category: &str) -> RpcDef {
    RpcDef {
        name: name.to_string(),
        description: String::new(),
        params,
        result,
        category: category.to_string(),
        ..RpcDef::default()
    }
}

/// Builds a minimal protocol module (name and definitions only; description is empty).
pub fn minimal_module(name: impl Into<String>, definitions: Vec<ProtocolDef>) -> ProtocolModule {
    ProtocolModule::new(name.into(), String::new(), definitions)
}
