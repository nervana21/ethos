use ir::{AccessLevel, ParamDef, RpcDef, TypeDef, TypeKind};
use registry::{ProtocolRegistry, ProtocolRegistryReader};

/// Helper function to create a test RPC definition with basic information
fn create_test_rpc_def(name: &str, description: &str) -> RpcDef {
    RpcDef {
        name: name.to_string(),
        description: description.to_string(),
        params: vec![],
        result: None,
        category: "test".to_string(),
        access_level: AccessLevel::default(),
        requires_private_keys: false,
        examples: None,
        hidden: None,
        version_added: None,
        version_removed: None,
    }
}

/// Helper function to create a test RPC definition with parameters and result
fn create_complex_rpc_def(name: &str, description: &str) -> RpcDef {
    let param = ParamDef {
        name: "block_hash".to_string(),
        param_type: TypeDef {
            name: "string".to_string(),
            description: "The block hash to query".to_string(),
            kind: TypeKind::Primitive,
            fields: None,
            variants: None,
            base_type: None,
            protocol_type: None,
            canonical_name: None,
            condition: None,
        },
        required: true,
        description: "The block hash to query".to_string(),
        default_value: None,
    };

    let result = TypeDef {
        name: "object".to_string(),
        description: "Information about the block".to_string(),
        kind: TypeKind::Object,
        fields: None,
        variants: None,
        base_type: None,
        protocol_type: None,
        canonical_name: None,
        condition: None,
    };

    RpcDef {
        name: name.to_string(),
        description: description.to_string(),
        params: vec![param],
        result: Some(result),
        category: "test".to_string(),
        access_level: AccessLevel::default(),
        requires_private_keys: false,
        examples: None,
        hidden: None,
        version_added: None,
        version_removed: None,
    }
}

#[test]
fn test_protocol_registry_new() {
    let registry = ProtocolRegistry::new();

    assert_eq!(registry.method_count(), 0);
    assert!(registry.list_methods().is_empty());
    assert!(registry.get_method("nonexistent").is_none());
}

#[test]
fn test_protocol_registry_insert() {
    let mut registry = ProtocolRegistry::new();

    let method1 =
        create_test_rpc_def("getblockchaininfo", "Returns information about the blockchain");
    registry.insert(method1);

    assert_eq!(registry.method_count(), 1);
    assert!(registry.list_methods().contains(&"getblockchaininfo"));
    assert!(registry.get_method("getblockchaininfo").is_some());

    let method2 = create_test_rpc_def("getblock", "Returns block information");
    registry.insert(method2);

    assert_eq!(registry.method_count(), 2);
    assert!(registry.list_methods().contains(&"getblockchaininfo"));
    assert!(registry.list_methods().contains(&"getblock"));

    let method3 = create_test_rpc_def("getblockchaininfo", "Updated description");
    registry.insert(method3);

    assert_eq!(registry.method_count(), 2);
    let retrieved_method = registry.get_method("getblockchaininfo").unwrap();
    assert_eq!(retrieved_method.description, "Updated description");
}

#[test]
fn test_protocol_registry_reader_list_methods() {
    let mut registry = ProtocolRegistry::new();

    assert!(registry.list_methods().is_empty());

    // Test with multiple methods to verify sorted order
    let method1 =
        create_test_rpc_def("getblockchaininfo", "Returns information about the blockchain");
    let method2 = create_test_rpc_def("getblock", "Returns block information");
    let method3 = create_test_rpc_def("getrawtransaction", "Returns raw transaction data");

    registry.insert(method1);
    registry.insert(method2);
    registry.insert(method3);

    let methods = registry.list_methods();
    assert_eq!(methods.len(), 3);

    // Verify methods are returned in sorted order
    let expected_order = vec!["getblock", "getblockchaininfo", "getrawtransaction"];
    assert_eq!(methods, expected_order);
}

#[test]
fn test_protocol_registry_reader_get_method() {
    let mut registry = ProtocolRegistry::new();

    assert!(registry.get_method("nonexistent").is_none());

    let method1 =
        create_test_rpc_def("getblockchaininfo", "Returns information about the blockchain");
    registry.insert(method1);

    assert!(registry.get_method("nonexistent").is_none());
    assert!(registry.get_method("getblock").is_none());

    let retrieved_method = registry.get_method("getblockchaininfo");
    assert!(retrieved_method.is_some());

    let method = retrieved_method.unwrap();
    assert_eq!(method.name, "getblockchaininfo");
    assert_eq!(method.description, "Returns information about the blockchain");

    let complex_method = create_complex_rpc_def("getblock", "Returns block information");
    registry.insert(complex_method);

    let retrieved_complex = registry.get_method("getblock");
    assert!(retrieved_complex.is_some());

    let method = retrieved_complex.unwrap();
    assert_eq!(method.name, "getblock");
    assert_eq!(method.params.len(), 1);
    assert!(method.result.is_some());
    assert_eq!(method.params[0].name, "block_hash");
}

#[test]
fn test_protocol_registry_reader_method_count() {
    let mut registry = ProtocolRegistry::new();

    assert_eq!(registry.method_count(), 0);

    let method1 =
        create_test_rpc_def("getblockchaininfo", "Returns information about the blockchain");
    registry.insert(method1);
    assert_eq!(registry.method_count(), 1);

    let method2 = create_test_rpc_def("getblock", "Returns block information");
    let method3 = create_test_rpc_def("getrawtransaction", "Returns raw transaction data");
    registry.insert(method2);
    registry.insert(method3);
    assert_eq!(registry.method_count(), 3);

    let method4 = create_test_rpc_def("getblockchaininfo", "Updated description");
    registry.insert(method4);
    assert_eq!(registry.method_count(), 3);

    let complex_method =
        create_complex_rpc_def("sendrawtransaction", "Broadcasts a raw transaction");
    registry.insert(complex_method);
    assert_eq!(registry.method_count(), 4);
}
