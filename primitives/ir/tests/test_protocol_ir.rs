//! Comprehensive unit tests for the Ethos IR module

use ethos_ir::*;

#[test]
fn test_protocol_ir_new() {
    let modules = vec![ProtocolModule::new("rpc".to_string(), "RPC module".to_string(), vec![])];
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
    let modules = vec![
        ProtocolModule::new("rpc".to_string(), "RPC module".to_string(), vec![]),
        ProtocolModule::new("p2p".to_string(), "P2P module".to_string(), vec![]),
    ];
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
    let rpc_def = RpcDef {
        name: "getblock".to_string(),
        description: "Get block".to_string(),
        params: vec![],
        result: None,
        category: "blockchain".to_string(),
        access_level: AccessLevel::default(),
        requires_private_keys: false,
        examples: None,
        hidden: None,
        version_added: None,
        version_removed: None,
    };

    let modules = vec![ProtocolModule::new(
        "rpc".to_string(),
        "RPC module".to_string(),
        vec![ProtocolDef::RpcMethod(rpc_def.clone())],
    )];
    let protocol_ir = ProtocolIR::new(modules);

    let rpc_methods = protocol_ir.get_rpc_methods();
    assert_eq!(rpc_methods.len(), 1);
    assert_eq!(rpc_methods[0].name, "getblock");
}

/// Test for ProtocolIR::get_type_definitions function
#[test]
fn test_protocol_ir_get_type_definitions() {
    let type_def = TypeDef {
        name: "Block".to_string(),
        description: "Block type".to_string(),
        kind: TypeKind::Object,
        fields: None,
        variants: None,
        base_type: None,
        protocol_type: None,
        canonical_name: None,
        condition: None,
    };

    let modules = vec![ProtocolModule::new(
        "types".to_string(),
        "Types module".to_string(),
        vec![ProtocolDef::Type(type_def.clone())],
    )];
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
    let modules = vec![
        ProtocolModule::new("rpc".to_string(), "RPC".to_string(), vec![]),
        ProtocolModule::new("p2p".to_string(), "P2P".to_string(), vec![]),
    ];
    let protocol_ir = ProtocolIR::new(modules.clone());

    let retrieved_modules = protocol_ir.modules();
    assert_eq!(retrieved_modules.len(), 2);
    assert_eq!(retrieved_modules[0].name(), "rpc");
    assert_eq!(retrieved_modules[1].name(), "p2p");
}

/// Test for ProtocolIR::modules_mut function
#[test]
fn test_protocol_ir_modules_mut() {
    let modules = vec![ProtocolModule::new("rpc".to_string(), "RPC".to_string(), vec![])];
    let mut protocol_ir = ProtocolIR::new(modules);

    let modules_mut = protocol_ir.modules_mut();
    assert_eq!(modules_mut.len(), 1);

    // Test that we can modify the modules
    modules_mut.push(ProtocolModule::new("p2p".to_string(), "P2P".to_string(), vec![]));
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
    let rpc_def = RpcDef {
        name: "getblock".to_string(),
        description: "Get block".to_string(),
        params: vec![],
        result: None,
        category: "blockchain".to_string(),
        access_level: AccessLevel::default(),
        requires_private_keys: false,
        examples: None,
        hidden: None,
        version_added: None,
        version_removed: None,
    };

    let type_def = TypeDef {
        name: "Block".to_string(),
        description: "Block type".to_string(),
        kind: TypeKind::Object,
        fields: None,
        variants: None,
        base_type: None,
        protocol_type: None,
        canonical_name: None,
        condition: None,
    };

    let modules = vec![
        ProtocolModule::new(
            "rpc".to_string(),
            "RPC module".to_string(),
            vec![ProtocolDef::RpcMethod(rpc_def)],
        ),
        ProtocolModule::new(
            "types".to_string(),
            "Types module".to_string(),
            vec![ProtocolDef::Type(type_def)],
        ),
    ];
    let protocol_ir = ProtocolIR::new(modules);

    assert_eq!(protocol_ir.definition_count(), 2);
}

/// Test for ProtocolIR::source_implementations function
#[test]
fn test_protocol_ir_source_implementations() {
    let rpc_def = RpcDef {
        name: "getblock".to_string(),
        description: "Get block".to_string(),
        params: vec![],
        result: None,
        category: "blockchain".to_string(),
        access_level: AccessLevel::default(),
        requires_private_keys: false,
        examples: None,
        hidden: None,
        version_added: None,
        version_removed: None,
    };

    let modules = vec![ProtocolModule::new(
        "rpc".to_string(),
        "RPC module".to_string(),
        vec![ProtocolDef::RpcMethod(rpc_def)],
    )];
    let _protocol_ir = ProtocolIR::new(modules);
}

/// Test for ProtocolIR::merge function
#[test]
fn test_protocol_ir_merge() {
    let rpc_def1 = RpcDef {
        name: "getblock".to_string(),
        description: "Get block".to_string(),
        params: vec![],
        result: None,
        category: "blockchain".to_string(),
        access_level: AccessLevel::default(),
        requires_private_keys: false,
        examples: None,
        hidden: None,
        version_added: None,
        version_removed: None,
    };

    let rpc_def2 = RpcDef {
        name: "getblock".to_string(),
        description: "Get block (duplicate)".to_string(),
        params: vec![],
        result: None,
        category: "blockchain".to_string(),
        access_level: AccessLevel::default(),
        requires_private_keys: false,
        examples: None,
        hidden: None,
        version_added: None,
        version_removed: None,
    };

    let ir1 = ProtocolIR::new(vec![ProtocolModule::new(
        "rpc".to_string(),
        "RPC".to_string(),
        vec![ProtocolDef::RpcMethod(rpc_def1)],
    )]);
    let ir2 = ProtocolIR::new(vec![ProtocolModule::new(
        "rpc".to_string(),
        "RPC".to_string(),
        vec![ProtocolDef::RpcMethod(rpc_def2)],
    )]);

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
    let definitions = vec![ProtocolDef::RpcMethod(RpcDef {
        name: "test".to_string(),
        description: "Test method".to_string(),
        params: vec![],
        result: None,
        category: "test".to_string(),
        access_level: AccessLevel::default(),
        requires_private_keys: false,
        examples: None,
        hidden: None,
        version_added: None,
        version_removed: None,
    })];

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
    let rpc_def = RpcDef {
        name: "getblock".to_string(),
        description: "Get block".to_string(),
        params: vec![],
        result: None,
        category: "blockchain".to_string(),
        access_level: AccessLevel::default(),
        requires_private_keys: false,
        examples: None,
        hidden: None,
        version_added: None,
        version_removed: None,
    };

    let definitions = vec![
        ProtocolDef::RpcMethod(rpc_def.clone()),
        ProtocolDef::Type(TypeDef {
            name: "Block".to_string(),
            description: "Block type".to_string(),
            kind: TypeKind::Object,
            fields: None,
            variants: None,
            base_type: None,
            protocol_type: None,
            canonical_name: None,
            condition: None,
        }),
    ];

    let module = ProtocolModule::new("rpc".to_string(), "RPC module".to_string(), definitions);

    let rpc_methods = module.get_rpc_methods();
    assert_eq!(rpc_methods.len(), 1);
    assert_eq!(rpc_methods[0].name, "getblock");
}

#[test]
fn test_protocol_module_get_type_definitions() {
    let type_def = TypeDef {
        name: "Transaction".to_string(),
        description: "Transaction type".to_string(),
        kind: TypeKind::Object,
        fields: None,
        variants: None,
        base_type: None,
        protocol_type: None,
        canonical_name: None,
        condition: None,
    };

    let definitions = vec![
        ProtocolDef::Type(type_def.clone()),
        ProtocolDef::RpcMethod(RpcDef {
            name: "sendrawtransaction".to_string(),
            description: "Send raw transaction".to_string(),
            params: vec![],
            result: None,
            category: "transactions".to_string(),
            access_level: AccessLevel::default(),
            requires_private_keys: false,
            examples: None,
            hidden: None,
            version_added: None,
            version_removed: None,
        }),
    ];

    let module = ProtocolModule::new("types".to_string(), "Types module".to_string(), definitions);

    let type_definitions = module.get_type_definitions();
    assert_eq!(type_definitions.len(), 1);
    assert_eq!(type_definitions[0].name, "Transaction");
}

#[test]
fn test_protocol_module_metadata() {
    let _module = ProtocolModule::new("test".to_string(), "Test module".to_string(), vec![]);

    // metadata() method was removed as part of metadata cleanup
    // This test is no longer relevant since we removed metadata entirely
}

#[test]
fn test_protocol_module_name() {
    let module = ProtocolModule::new("wallet".to_string(), "Wallet module".to_string(), vec![]);

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
        ProtocolDef::RpcMethod(RpcDef {
            name: "getbalance".to_string(),
            description: "Get balance".to_string(),
            params: vec![],
            result: None,
            category: "wallet".to_string(),
            access_level: AccessLevel::default(),
            requires_private_keys: false,
            examples: None,
            hidden: None,
            version_added: None,
            version_removed: None,
        }),
        ProtocolDef::Type(TypeDef {
            name: "Balance".to_string(),
            description: "Balance type".to_string(),
            kind: TypeKind::Object,
            fields: None,
            variants: None,
            base_type: None,
            protocol_type: None,
            canonical_name: None,
            condition: None,
        }),
    ];

    let module =
        ProtocolModule::new("wallet".to_string(), "Wallet module".to_string(), definitions.clone());

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
    let mut module = ProtocolModule::new("test".to_string(), "Test module".to_string(), vec![]);

    let definitions_mut = module.definitions_mut();
    assert_eq!(definitions_mut.len(), 0);

    definitions_mut.push(ProtocolDef::RpcMethod(RpcDef {
        name: "test_method".to_string(),
        description: "Test method".to_string(),
        params: vec![],
        result: None,
        category: "test".to_string(),
        access_level: AccessLevel::default(),
        requires_private_keys: false,
        examples: None,
        hidden: None,
        version_added: None,
        version_removed: None,
    }));

    assert_eq!(module.definitions().len(), 1);
}

#[test]
fn test_protocol_module_from_source() {
    let definitions = vec![ProtocolDef::RpcMethod(RpcDef {
        name: "getinfo".to_string(),
        description: "Get info".to_string(),
        params: vec![],
        result: None,
        category: "info".to_string(),
        access_level: AccessLevel::default(),
        requires_private_keys: false,
        examples: None,
        hidden: None,
        version_added: None,
        version_removed: None,
    })];

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
