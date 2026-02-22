use ethos_analysis::IrValidator;
use ir::test_utils::{minimal_module, param, primitive_type, rpc, type_def};
use ir::{ProtocolDef, ProtocolIR, ProtocolModule, TypeKind};

#[test]
fn allows_hashorheight_as_primitive() {
    let bad = primitive_type("HashOrHeight", None);
    let rpc = rpc("getblockstats", vec![param("hash_or_height", bad, true)], None, "core");
    let ir = ProtocolIR::new(vec![minimal_module("core", vec![ProtocolDef::RpcMethod(rpc)])]);

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
    let rpc1 = rpc("getblock", vec![], None, "core");
    let rpc2 = rpc("getblock", vec![], None, "core"); // Same name
    let ir = ProtocolIR::new(vec![minimal_module(
        "core",
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
    let rpc = rpc(
        "test",
        vec![param("", primitive_type("String", None), true)], // Empty name
        None,
        "core",
    );
    let ir = ProtocolIR::new(vec![minimal_module("core", vec![ProtocolDef::RpcMethod(rpc)])]);

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
    let string_type = primitive_type("String", None);
    let rpc = rpc(
        "test",
        vec![
            param("param1", string_type.clone(), true),
            param("param1", string_type, true), // Same name
        ],
        None,
        "core",
    );
    let ir = ProtocolIR::new(vec![minimal_module("core", vec![ProtocolDef::RpcMethod(rpc)])]);

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
    let rpc = rpc(
        "test",
        vec![param(
            "param1",
            type_def("String", TypeKind::Optional), // Optional kind
            true,                                   // But required=true
        )],
        None,
        "core",
    );
    let ir = ProtocolIR::new(vec![minimal_module("core", vec![ProtocolDef::RpcMethod(rpc)])]);

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
    let rpc = rpc(
        "test",
        vec![param(
            "param1",
            type_def("CustomType", TypeKind::Custom), // Custom kind, no base_type
            true,
        )],
        None,
        "core",
    );
    let ir = ProtocolIR::new(vec![minimal_module("core", vec![ProtocolDef::RpcMethod(rpc)])]);

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
    let mut rpc = rpc(
        "getblock",
        vec![param("hash", primitive_type("String", None), true)],
        Some(type_def("BlockInfo", TypeKind::Object)),
        "core",
    );
    rpc.description = "Get block information".into();
    let module = ProtocolModule::new(
        "core".into(),
        "Core RPC methods".into(),
        vec![ProtocolDef::RpcMethod(rpc)],
    );
    let ir = ProtocolIR::new(vec![module]);

    let validator = IrValidator::new();
    let errors = validator.validate(&ir);
    assert!(errors.is_empty(), "Expected no validation errors for valid IR, got: {:?}", errors);
}
