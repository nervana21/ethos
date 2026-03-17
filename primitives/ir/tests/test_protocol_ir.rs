//! Comprehensive unit tests for the Ethos IR module

use ethos_ir::test_utils::{minimal_module, rpc, type_def};
use ethos_ir::*;

#[test]
fn test_protocol_ir_new() {
    let modules = vec![minimal_module("rpc", vec![])];
    let version = "0.1.0".to_string();

    let protocol_ir = ProtocolIR::new(modules);

    assert_eq!(protocol_ir.version(), &version);
    assert_eq!(protocol_ir.modules().len(), 1);
    assert_eq!(protocol_ir.name(), "Ethos Protocol");
    assert_eq!(protocol_ir.description(), "The canonical Ethos protocol specification");
    assert_eq!(protocol_ir.definition_count(), 0);
}

#[test]
fn test_protocol_ir_get_module() {
    let modules = vec![minimal_module("rpc", vec![]), minimal_module("p2p", vec![])];
    let protocol_ir = ProtocolIR::new(modules);

    let rpc_module = protocol_ir.get_module("rpc");
    assert!(rpc_module.is_some());
    assert_eq!(rpc_module.expect("rpc module should exist").name(), "rpc");

    let p2p_module = protocol_ir.get_module("p2p");
    assert!(p2p_module.is_some());
    assert_eq!(p2p_module.expect("p2p module should exist").name(), "p2p");

    let non_existent = protocol_ir.get_module("nonexistent");
    assert!(non_existent.is_none());
}

#[test]
fn test_protocol_ir_get_rpc_methods() {
    let rpc_def = rpc("getblock", vec![], None, "blockchain");

    let modules = vec![minimal_module("rpc", vec![ProtocolDef::RpcMethod(rpc_def.clone())])];
    let protocol_ir = ProtocolIR::new(modules);

    let rpc_methods = protocol_ir.get_rpc_methods();
    assert_eq!(rpc_methods.len(), 1);
    assert_eq!(rpc_methods[0].name, "getblock");
}

/// Test for ProtocolIR::get_type_definitions function
#[test]
fn test_protocol_ir_get_type_definitions() {
    let type_def = type_def("Block", TypeKind::Object);

    let modules = vec![minimal_module("types", vec![ProtocolDef::Type(type_def.clone())])];
    let protocol_ir = ProtocolIR::new(modules);

    let type_definitions = protocol_ir.get_type_definitions();
    assert_eq!(type_definitions.len(), 1);
    assert_eq!(type_definitions[0].name, "Block");
}

/// Test for ProtocolIR::version function
#[test]
fn test_protocol_ir_version() {
    let modules = vec![];
    let protocol_ir = ProtocolIR::new(modules);

    assert_eq!(protocol_ir.version(), "0.1.0");
}

/// Test for ProtocolIR::modules function
#[test]
fn test_protocol_ir_modules() {
    let modules = vec![minimal_module("rpc", vec![]), minimal_module("p2p", vec![])];
    let protocol_ir = ProtocolIR::new(modules.clone());

    let retrieved_modules = protocol_ir.modules();
    assert_eq!(retrieved_modules.len(), 2);
    assert_eq!(retrieved_modules[0].name(), "rpc");
    assert_eq!(retrieved_modules[1].name(), "p2p");
}

/// Test for ProtocolIR::modules_mut function
#[test]
fn test_protocol_ir_modules_mut() {
    let modules = vec![minimal_module("rpc", vec![])];
    let mut protocol_ir = ProtocolIR::new(modules);

    let modules_mut = protocol_ir.modules_mut();
    assert_eq!(modules_mut.len(), 1);

    // Test that we can modify the modules
    modules_mut.push(minimal_module("p2p", vec![]));
    assert_eq!(protocol_ir.modules().len(), 2);
}

/// Test for ProtocolIR::name function
#[test]
fn test_protocol_ir_name() {
    let modules = vec![];
    let protocol_ir = ProtocolIR::new(modules);

    assert_eq!(protocol_ir.name(), "Ethos Protocol");
}

/// Test for ProtocolIR::description function
#[test]
fn test_protocol_ir_description() {
    let modules = vec![];
    let protocol_ir = ProtocolIR::new(modules);

    assert_eq!(protocol_ir.description(), "The canonical Ethos protocol specification");
}

/// Test for ProtocolIR::definition_count function
#[test]
fn test_protocol_ir_definition_count() {
    let rpc_def = rpc("getblock", vec![], None, "blockchain");
    let type_def = type_def("Block", TypeKind::Object);

    let modules = vec![
        minimal_module("rpc", vec![ProtocolDef::RpcMethod(rpc_def)]),
        minimal_module("types", vec![ProtocolDef::Type(type_def)]),
    ];
    let protocol_ir = ProtocolIR::new(modules);

    assert_eq!(protocol_ir.definition_count(), 2);
}

/// Test for ProtocolIR::source_implementations function
#[test]
fn test_protocol_ir_source_implementations() {
    let rpc_def = rpc("getblock", vec![], None, "blockchain");

    let modules = vec![minimal_module("rpc", vec![ProtocolDef::RpcMethod(rpc_def)])];
    let _protocol_ir = ProtocolIR::new(modules);
}

#[test]
fn test_strip_hidden_rpcs() {
    let mut rpc_hidden = rpc("hidden_rpc", vec![], None, "test");
    rpc_hidden.hidden = Some(true);

    let rpc_public = rpc("public_rpc", vec![], None, "test"); // hidden: None

    let mut rpc_explicit_visible = rpc("visible_rpc", vec![], None, "test");
    rpc_explicit_visible.hidden = Some(false);

    let type_def = type_def("SomeType", TypeKind::Object);

    let definitions = vec![
        ProtocolDef::RpcMethod(rpc_hidden),
        ProtocolDef::RpcMethod(rpc_public),
        ProtocolDef::RpcMethod(rpc_explicit_visible),
        ProtocolDef::Type(type_def),
    ];
    let mut ir = ProtocolIR::new(vec![minimal_module("rpc", definitions)]);

    assert_eq!(ir.definition_count(), 4);
    assert_eq!(ir.get_rpc_methods().len(), 3);

    ir.strip_hidden_rpcs();

    assert_eq!(ir.definition_count(), 3, "one hidden RPC should be removed");
    let rpcs = ir.get_rpc_methods();
    assert_eq!(rpcs.len(), 2);
    let names: Vec<&str> = rpcs.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"public_rpc"));
    assert!(names.contains(&"visible_rpc"));
    assert!(!names.contains(&"hidden_rpc"));

    let types = ir.get_type_definitions();
    assert_eq!(types.len(), 1);
    assert_eq!(types[0].name, "SomeType");
}

/// Test for ProtocolIR::merge function
#[test]
fn test_protocol_ir_merge() {
    let rpc_def1 = rpc("getblock", vec![], None, "blockchain");
    let mut rpc_def2 = rpc("getblock", vec![], None, "blockchain");
    rpc_def2.description = "Get block (duplicate)".to_string();

    let ir1 = ProtocolIR::new(vec![minimal_module("rpc", vec![ProtocolDef::RpcMethod(rpc_def1)])]);
    let ir2 = ProtocolIR::new(vec![minimal_module("rpc", vec![ProtocolDef::RpcMethod(rpc_def2)])]);

    let merged = ProtocolIR::merge(vec![ir1, ir2]);

    assert_eq!(merged.version(), "merged");
    assert_eq!(merged.modules().len(), 1);
    assert_eq!(merged.get_rpc_methods().len(), 1); // Should deduplicate

    let empty_merged = ProtocolIR::merge(vec![]);
    assert_eq!(empty_merged.version(), "empty");
    assert_eq!(empty_merged.modules().len(), 0);

    let single_ir = ProtocolIR::new(vec![]);
    let single_merged = ProtocolIR::merge(vec![single_ir]);
    assert_eq!(single_merged.version(), "0.1.0");
}

#[test]
fn test_protocol_module_new() {
    let definitions = vec![ProtocolDef::RpcMethod(rpc("test", vec![], None, "test"))];

    let module = ProtocolModule::new(
        "test_module".to_string(),
        "Test module description".to_string(),
        definitions,
    );

    assert_eq!(module.name(), "test_module");
    assert_eq!(module.description(), "Test module description");
    assert_eq!(module.definitions().len(), 1);
    // metadata() method was removed as part of metadata cleanup
}

#[test]
fn test_protocol_module_get_rpc_methods() {
    let rpc_def = rpc("getblock", vec![], None, "blockchain");

    let definitions = vec![
        ProtocolDef::RpcMethod(rpc_def.clone()),
        ProtocolDef::Type(type_def("Block", TypeKind::Object)),
    ];

    let module = minimal_module("rpc", definitions);

    let rpc_methods = module.get_rpc_methods();
    assert_eq!(rpc_methods.len(), 1);
    assert_eq!(rpc_methods[0].name, "getblock");
}

#[test]
fn test_protocol_module_get_type_definitions() {
    let type_def = type_def("Transaction", TypeKind::Object);

    let definitions = vec![
        ProtocolDef::Type(type_def.clone()),
        ProtocolDef::RpcMethod(rpc("sendrawtransaction", vec![], None, "transactions")),
    ];

    let module = minimal_module("types", definitions);

    let type_definitions = module.get_type_definitions();
    assert_eq!(type_definitions.len(), 1);
    assert_eq!(type_definitions[0].name, "Transaction");
}

#[test]
fn test_protocol_module_metadata() {
    let _module = minimal_module("test", vec![]);

    // metadata() method was removed as part of metadata cleanup
    // This test is no longer relevant since we removed metadata entirely
}

#[test]
fn test_protocol_module_name() {
    let module = minimal_module("wallet", vec![]);

    assert_eq!(module.name(), "wallet");
}

#[test]
fn test_protocol_module_description() {
    let description = "Advanced wallet operations module";
    let module = ProtocolModule::new("wallet".to_string(), description.to_string(), vec![]);

    assert_eq!(module.description(), description);
}

#[test]
fn test_protocol_module_definitions() {
    let definitions = vec![
        ProtocolDef::RpcMethod(rpc("getbalance", vec![], None, "wallet")),
        ProtocolDef::Type(type_def("Balance", TypeKind::Object)),
    ];

    let module = minimal_module("wallet", definitions.clone());

    let retrieved_definitions = module.definitions();
    assert_eq!(retrieved_definitions.len(), 2);

    match &retrieved_definitions[0] {
        ProtocolDef::RpcMethod(rpc) => assert_eq!(rpc.name, "getbalance"),
        _ => panic!("Expected RpcMethod as first definition"),
    }
    match &retrieved_definitions[1] {
        ProtocolDef::Type(ty) => assert_eq!(ty.name, "Balance"),
        _ => panic!("Expected Type as second definition"),
    }
}

#[test]
fn test_protocol_module_definitions_mut() {
    let mut module = minimal_module("test", vec![]);

    let definitions_mut = module.definitions_mut();
    assert_eq!(definitions_mut.len(), 0);

    definitions_mut.push(ProtocolDef::RpcMethod(rpc("test_method", vec![], None, "test")));

    assert_eq!(module.definitions().len(), 1);
}

#[test]
fn test_protocol_module_from_source() {
    let definitions = vec![ProtocolDef::RpcMethod(rpc("getinfo", vec![], None, "info"))];

    let module = ProtocolModule::new(
        "bitcoin_core".to_string(),
        "Bitcoin Core RPC module".to_string(),
        definitions,
    );

    assert_eq!(module.name(), "bitcoin_core");
    assert_eq!(module.description(), "Bitcoin Core RPC module");
    assert_eq!(module.definitions().len(), 1);
    // metadata() method was removed as part of metadata cleanup
}
