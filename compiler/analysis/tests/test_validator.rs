use ethos_analysis::IrValidator;
use ir::{
    AccessLevel, ParamDef, ProtocolDef, ProtocolIR, ProtocolModule, RpcDef, TypeDef, TypeKind,
};

#[test]
fn allows_hashorheight_as_primitive() {
    let bad = TypeDef {
        name: "HashOrHeight".into(),
        description: "".into(),
        kind: TypeKind::Primitive,
        fields: None,
        variants: None,
        base_type: None,
        protocol_type: None,
        canonical_name: None,
        condition: None,
    };
    let rpc = RpcDef {
        name: "getblockstats".into(),
        description: "".into(),
        params: vec![ParamDef {
            name: "hash_or_height".into(),
            param_type: bad,
            required: true,
            description: "".into(),
            default_value: None,
        }],
        result: None,
        category: "core".into(),
        access_level: AccessLevel::default(),
        requires_private_keys: false,
        examples: None,
        hidden: None,
        version_added: None,
        version_removed: None,
    };
    let ir = ProtocolIR::new(vec![ProtocolModule::new(
        "core".into(),
        "".into(),
        vec![ProtocolDef::RpcMethod(rpc)],
    )]);

    let validator = IrValidator::new();
    let errors = validator.validate(&ir);
    assert!(
        errors.is_empty(),
        "HashOrHeight as Primitive should not be rejected at validator layer, got: {:?}",
        errors
    );
}

#[test]
fn fails_on_duplicate_rpc_names() {
    let rpc1 = RpcDef {
        name: "getblock".into(),
        description: "".into(),
        params: vec![],
        result: None,
        category: "core".into(),
        access_level: AccessLevel::default(),
        requires_private_keys: false,
        examples: None,
        hidden: None,
        version_added: None,
        version_removed: None,
    };
    let rpc2 = RpcDef {
        name: "getblock".into(), // Same name
        description: "".into(),
        params: vec![],
        result: None,
        category: "core".into(),
        access_level: AccessLevel::default(),
        requires_private_keys: false,
        examples: None,
        hidden: None,
        version_added: None,
        version_removed: None,
    };
    let ir = ProtocolIR::new(vec![ProtocolModule::new(
        "core".into(),
        "".into(),
        vec![ProtocolDef::RpcMethod(rpc1), ProtocolDef::RpcMethod(rpc2)],
    )]);

    let validator = IrValidator::new();
    let errors = validator.validate(&ir);
    assert!(!errors.is_empty(), "Expected validation errors");
    assert!(
        errors.iter().any(|e| e.contains("Duplicate RPC name")),
        "Expected duplicate RPC name error, got: {:?}",
        errors
    );
}

#[test]
fn fails_on_empty_param_name() {
    let rpc = RpcDef {
        name: "test".into(),
        description: "".into(),
        params: vec![ParamDef {
            name: "".into(), // Empty name
            param_type: TypeDef {
                name: "String".into(),
                description: "".into(),
                kind: TypeKind::Primitive,
                fields: None,
                variants: None,
                base_type: None,
                protocol_type: None,
                canonical_name: None,
                condition: None,
            },
            required: true,
            description: "".into(),
            default_value: None,
        }],
        result: None,
        category: "core".into(),
        access_level: AccessLevel::default(),
        requires_private_keys: false,
        examples: None,
        hidden: None,
        version_added: None,
        version_removed: None,
    };
    let ir = ProtocolIR::new(vec![ProtocolModule::new(
        "core".into(),
        "".into(),
        vec![ProtocolDef::RpcMethod(rpc)],
    )]);

    let validator = IrValidator::new();
    let errors = validator.validate(&ir);
    assert!(!errors.is_empty(), "Expected validation errors");
    assert!(
        errors.iter().any(|e| e.contains("empty name")),
        "Expected empty param name error, got: {:?}",
        errors
    );
}

#[test]
fn fails_on_duplicate_param_names() {
    let rpc = RpcDef {
        name: "test".into(),
        description: "".into(),
        params: vec![
            ParamDef {
                name: "param1".into(),
                param_type: TypeDef {
                    name: "String".into(),
                    description: "".into(),
                    kind: TypeKind::Primitive,
                    fields: None,
                    variants: None,
                    base_type: None,
                    protocol_type: None,
                    canonical_name: None,
                    condition: None,
                },
                required: true,
                description: "".into(),
                default_value: None,
            },
            ParamDef {
                name: "param1".into(), // Same name
                param_type: TypeDef {
                    name: "String".into(),
                    description: "".into(),
                    kind: TypeKind::Primitive,
                    fields: None,
                    variants: None,
                    base_type: None,
                    protocol_type: None,
                    canonical_name: None,
                    condition: None,
                },
                required: true,
                description: "".into(),
                default_value: None,
            },
        ],
        result: None,
        category: "core".into(),
        access_level: AccessLevel::default(),
        requires_private_keys: false,
        examples: None,
        hidden: None,
        version_added: None,
        version_removed: None,
    };
    let ir = ProtocolIR::new(vec![ProtocolModule::new(
        "core".into(),
        "".into(),
        vec![ProtocolDef::RpcMethod(rpc)],
    )]);

    let validator = IrValidator::new();
    let errors = validator.validate(&ir);
    assert!(!errors.is_empty(), "Expected validation errors");
    assert!(
        errors.iter().any(|e| e.contains("duplicate param")),
        "Expected duplicate param error, got: {:?}",
        errors
    );
}

#[test]
fn fails_on_optional_kind_with_required_true() {
    let rpc = RpcDef {
        name: "test".into(),
        description: "".into(),
        params: vec![ParamDef {
            name: "param1".into(),
            param_type: TypeDef {
                name: "String".into(),
                description: "".into(),
                kind: TypeKind::Optional, // Optional kind
                fields: None,
                variants: None,
                base_type: None,
                protocol_type: None,
                canonical_name: None,
                condition: None,
            },
            required: true, // But required=true
            description: "".into(),
            default_value: None,
        }],
        result: None,
        category: "core".into(),
        access_level: AccessLevel::default(),
        requires_private_keys: false,
        examples: None,
        hidden: None,
        version_added: None,
        version_removed: None,
    };
    let ir = ProtocolIR::new(vec![ProtocolModule::new(
        "core".into(),
        "".into(),
        vec![ProtocolDef::RpcMethod(rpc)],
    )]);

    let validator = IrValidator::new();
    let errors = validator.validate(&ir);
    assert!(!errors.is_empty(), "Expected validation errors");
    assert!(
        errors.iter().any(|e| e.contains("Optional kind with required=true is invalid")),
        "Expected optional kind validation error, got: {:?}",
        errors
    );
}

#[test]
fn fails_on_custom_type_without_base_type() {
    let rpc = RpcDef {
        name: "test".into(),
        description: "".into(),
        params: vec![ParamDef {
            name: "param1".into(),
            param_type: TypeDef {
                name: "CustomType".into(),
                description: "".into(),
                kind: TypeKind::Custom, // Custom kind
                fields: None,
                variants: None,
                base_type: None, // But no base_type
                protocol_type: None,
                canonical_name: None,
                condition: None,
            },
            required: true,
            description: "".into(),
            default_value: None,
        }],
        result: None,
        category: "core".into(),
        access_level: AccessLevel::default(),
        requires_private_keys: false,
        examples: None,
        hidden: None,
        version_added: None,
        version_removed: None,
    };
    let ir = ProtocolIR::new(vec![ProtocolModule::new(
        "core".into(),
        "".into(),
        vec![ProtocolDef::RpcMethod(rpc)],
    )]);

    let validator = IrValidator::new();
    let errors = validator.validate(&ir);
    assert!(!errors.is_empty(), "Expected validation errors");
    assert!(
        errors.iter().any(|e| e.contains("Custom type must set base_type")),
        "Expected custom type validation error, got: {:?}",
        errors
    );
}

#[test]
fn passes_with_valid_ir() {
    let rpc = RpcDef {
        name: "getblock".into(),
        description: "Get block information".into(),
        params: vec![ParamDef {
            name: "hash".into(),
            param_type: TypeDef {
                name: "String".into(),
                description: "Block hash".into(),
                kind: TypeKind::Primitive,
                fields: None,
                variants: None,
                base_type: None,
                protocol_type: None,
                canonical_name: None,
                condition: None,
            },
            required: true,
            description: "Block hash".into(),
            default_value: None,
        }],
        result: Some(TypeDef {
            name: "BlockInfo".into(),
            description: "Block information".into(),
            kind: TypeKind::Object,
            fields: None,
            variants: None,
            base_type: None,
            protocol_type: None,
            canonical_name: None,
            condition: None,
        }),
        category: "core".into(),
        access_level: AccessLevel::default(),
        requires_private_keys: false,
        examples: None,
        hidden: None,
        version_added: None,
        version_removed: None,
    };
    let ir = ProtocolIR::new(vec![ProtocolModule::new(
        "core".into(),
        "Core RPC methods".into(),
        vec![ProtocolDef::RpcMethod(rpc)],
    )]);

    let validator = IrValidator::new();
    let errors = validator.validate(&ir);
    assert!(errors.is_empty(), "Expected no validation errors for valid IR, got: {:?}", errors);
}
