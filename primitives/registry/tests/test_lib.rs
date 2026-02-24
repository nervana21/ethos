use ir::test_utils::{param, primitive_type, rpc, type_def};
use ir::{RpcDef, TypeKind};
use registry::{ProtocolRegistry, ProtocolRegistryReader};

/// Helper function to create a test RPC definition with basic information
fn create_test_rpc_def(name: &str, description: &str) -> RpcDef {
    let mut r = rpc(name, vec![], None, "test");
    r.description = description.to_string();
    r
}

/// Helper function to create a test RPC definition with parameters and result
fn create_complex_rpc_def(name: &str, description: &str) -> RpcDef {
    let param = param("block_hash", primitive_type("string", None), true);
    let result = type_def("object", TypeKind::Object);
    let mut r = rpc(name, vec![param], Some(result), "test");
    r.description = description.to_string();
    r
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
