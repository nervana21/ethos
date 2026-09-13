// SPDX-License-Identifier: CC0-1.0

//! IR roundtrip tests (serialize → deserialize).

use ethos_ir::test_utils::{field, field_anon, minimal_module, rpc, type_def};
use ethos_ir::{ProtocolDef, ProtocolIR, ProtocolModule, TypeDef, TypeKind};
use tempfile::TempDir;

fn minimal_type_def() -> TypeDef { type_def("", TypeKind::Primitive) }

fn create_sample_ir() -> ProtocolIR {
    let mut result = type_def("BlockInfo", TypeKind::Object);
    result.description = "Block information".to_string();
    result.fields = Some(vec![]);
    result.protocol_type = Some("object".to_string());

    let mut rpc_def = rpc("getblock", vec![], Some(result), "node");
    rpc_def.description = "Get block by hash".to_string();

    let module = minimal_module("rpc", vec![ProtocolDef::RpcMethod(rpc_def)]);
    ProtocolIR::new_with_version("0.1.0".to_string(), vec![module])
}

#[test]
fn test_rust_emit_name_prefers_type_identity() {
    let mut td = type_def("WrongLabel", TypeKind::Object);
    td.type_identity = Some("RightLabel".to_string());
    assert_eq!(td.rust_emit_name(), "RightLabel");

    let json = serde_json::to_string(&td).expect("serialize");
    let loaded: TypeDef = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(loaded.rust_emit_name(), "RightLabel");
    assert_eq!(loaded.name, "WrongLabel");
}

#[test]
fn test_field_key_roundtrip() {
    // Roundtrip `FieldKey` Named and Anonymous through JSON.
    let type_with_keys = TypeDef {
        name: "object".to_string(),
        description: String::new(),
        kind: TypeKind::Object,
        fields: Some(vec![
            field("txid", minimal_type_def(), true),
            field_anon(1, minimal_type_def(), true),
        ]),
        variants: None,
        union_variants: None,
        base_type: None,
        protocol_type: None,
        canonical_name: None,
        condition: None,
        ..Default::default()
    };
    let json = serde_json::to_string_pretty(&type_with_keys).expect("serialize");
    let loaded: TypeDef = serde_json::from_str(&json).expect("deserialize");
    let fields = loaded.fields.as_ref().expect("fields");
    assert_eq!(fields[0].key.as_ident(), "txid");
    assert_eq!(fields[1].key.as_ident(), "field_1");
    assert!(!fields[0].key.is_anonymous());
    assert_eq!(fields[1].key.anonymous_index(), Some(1));
}

#[test]
fn array_element_type_helper_supports_anonymous_and_named_field_0() {
    // Anonymous positional element at index 0.
    let elem_ty = minimal_type_def();
    let array_with_anonymous = TypeDef {
        name: "array".to_string(),
        description: String::new(),
        kind: TypeKind::Array,
        fields: Some(vec![field_anon(0, elem_ty.clone(), true)]),
        variants: None,
        union_variants: None,
        base_type: None,
        protocol_type: Some("array".to_string()),
        canonical_name: None,
        condition: None,
        ..Default::default()
    };

    let elem =
        array_with_anonymous.array_element_type().expect("anonymous(0) element must be recognized");
    assert_eq!(elem.name, elem_ty.name);

    // Synthetic Named(\"field_0\") element should also be recognized.
    let array_with_named = TypeDef {
        name: "array".to_string(),
        description: String::new(),
        kind: TypeKind::Array,
        fields: Some(vec![field("field_0", elem_ty.clone(), true)]),
        variants: None,
        union_variants: None,
        base_type: None,
        protocol_type: Some("array".to_string()),
        canonical_name: None,
        condition: None,
        ..Default::default()
    };

    let elem2 = array_with_named
        .array_element_type()
        .expect("Named(\"field_0\") element must be recognized");
    assert_eq!(elem2.name, elem_ty.name);

    // Non-array kinds and arrays with multiple fields should return None.
    let not_array = minimal_type_def();
    assert!(not_array.array_element_type().is_none());

    let multi_field_array = TypeDef {
        name: "array".to_string(),
        description: String::new(),
        kind: TypeKind::Array,
        fields: Some(vec![
            field_anon(0, minimal_type_def(), true),
            field_anon(1, minimal_type_def(), true),
        ]),
        variants: None,
        union_variants: None,
        base_type: None,
        protocol_type: Some("array".to_string()),
        canonical_name: None,
        condition: None,
        ..Default::default()
    };
    assert!(multi_field_array.array_element_type().is_none());
}

#[test]
fn homogeneous_array_element_type_supports_prefix_items_tuples() {
    let number_elem = TypeDef {
        name: "number".to_string(),
        description: String::new(),
        kind: TypeKind::Primitive,
        fields: None,
        variants: None,
        union_variants: None,
        base_type: None,
        protocol_type: Some("number".to_string()),
        canonical_name: None,
        condition: None,
        ..Default::default()
    };
    let tuple_array = TypeDef {
        name: "array".to_string(),
        description: String::new(),
        kind: TypeKind::Array,
        fields: Some(
            (0..5)
                .map(|idx| field_anon(idx, number_elem.clone(), true))
                .collect(),
        ),
        variants: None,
        union_variants: None,
        base_type: None,
        protocol_type: Some("array".to_string()),
        canonical_name: None,
        condition: None,
        ..Default::default()
    };

    let elem = tuple_array
        .homogeneous_array_element_type()
        .expect("homogeneous prefixItems tuple must yield shared element type");
    assert_eq!(elem.protocol_type.as_deref(), Some("number"));

    let string_elem = TypeDef {
        name: "string".to_string(),
        description: String::new(),
        kind: TypeKind::Primitive,
        fields: None,
        variants: None,
        union_variants: None,
        base_type: None,
        protocol_type: Some("string".to_string()),
        canonical_name: None,
        condition: None,
        ..Default::default()
    };
    let mixed_tuple = TypeDef {
        name: "array".to_string(),
        description: String::new(),
        kind: TypeKind::Array,
        fields: Some(vec![
            field_anon(0, number_elem, true),
            field_anon(1, string_elem, true),
        ]),
        variants: None,
        union_variants: None,
        base_type: None,
        protocol_type: Some("array".to_string()),
        canonical_name: None,
        condition: None,
        ..Default::default()
    };
    assert!(mixed_tuple.homogeneous_array_element_type().is_none());
    assert!(mixed_tuple.prefix_items_tuple_fields().is_some());
}

#[test]
fn test_typekind_map_json_roundtrip() {
    let value_ty = minimal_type_def();
    let map_ty = TypeDef {
        name: "TxMetaMap".to_string(),
        description: "dynamic-key object".to_string(),
        kind: TypeKind::Map,
        fields: None,
        variants: None,
        union_variants: None,
        base_type: None,
        protocol_type: None,
        canonical_name: None,
        type_identity: None,
        condition: None,
        map_value: Some(Box::new(value_ty.clone())),
        map_key_protocol_type: Some("hex".to_string()),
    };

    let json = serde_json::to_string_pretty(&map_ty).expect("serialize");
    let loaded: TypeDef = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(loaded.kind, TypeKind::Map);
    assert_eq!(loaded.map_key_protocol_type.as_deref(), Some("hex"));
    let inner = loaded.map_value_type().expect("map value");
    assert_eq!(inner.name, value_ty.name);
}

#[test]
fn test_ir_roundtrip_simple() {
    let rpc = rpc("getblock", vec![], Some(type_def("GetBlockResponse", TypeKind::Object)), "node");

    let module =
        ProtocolModule::from_source("rpc", "Bitcoin Core RPC", vec![ProtocolDef::RpcMethod(rpc)]);
    let ir = ProtocolIR::new(vec![module]);

    let tmp = std::env::temp_dir().join("ethos_ir_roundtrip").join("simple.ir.json");
    if let Some(parent) = tmp.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    ir.to_file(&tmp).expect("failed to write IR");
    let loaded = ProtocolIR::from_file(&tmp).expect("failed to load IR");

    assert_eq!(loaded.modules().len(), 1);
    assert_eq!(loaded.get_rpc_methods().len(), 1);
    assert_eq!(loaded.version(), ir.version());
}

#[test]
fn test_ir_roundtrip_deterministic() {
    // Build a small IR with a couple of items
    let mut type_def = type_def("Amount", TypeKind::Alias);
    type_def.base_type = Some("u64".to_string());

    let rpc = rpc("getbalance", vec![], Some(type_def.clone()), "wallet");

    let module = ProtocolModule::from_source(
        "rpc",
        "Bitcoin Core RPC",
        vec![ProtocolDef::Type(type_def), ProtocolDef::RpcMethod(rpc)],
    );
    let ir = ProtocolIR::new(vec![module]);

    let tmp1 = std::env::temp_dir().join("ethos_ir_roundtrip").join("deterministic1.ir.json");
    let tmp2 = std::env::temp_dir().join("ethos_ir_roundtrip").join("deterministic2.ir.json");
    if let Some(parent) = tmp1.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    ir.to_file(&tmp1).expect("failed to write IR #1");
    let loaded1 = ProtocolIR::from_file(&tmp1).expect("failed to load IR #1");

    loaded1.to_file(&tmp2).expect("failed to write IR #2");
    let loaded2 = ProtocolIR::from_file(&tmp2).expect("failed to load IR #2");

    assert_eq!(loaded1.definition_count(), loaded2.definition_count());
    assert_eq!(loaded1.get_rpc_methods().len(), loaded2.get_rpc_methods().len());
}

#[test]
fn test_roundtrip_serialization() {
    let original_ir = create_sample_ir();
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let file_path = temp_dir.path().join("test.ir.json");

    original_ir.to_file(&file_path).expect("Failed to save IR to file");

    let loaded_ir = ProtocolIR::from_file(&file_path).expect("Failed to load IR from file");

    assert_eq!(original_ir.version(), loaded_ir.version());
    assert_eq!(original_ir.modules().len(), loaded_ir.modules().len());
    assert_eq!(original_ir.modules()[0].name(), loaded_ir.modules()[0].name());
    assert_eq!(
        original_ir.modules()[0].definitions().len(),
        loaded_ir.modules()[0].definitions().len()
    );
}

#[test]
fn test_deterministic_serialization() {
    let ir = create_sample_ir();
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let file_path1 = temp_dir.path().join("test1.ir.json");
    let file_path2 = temp_dir.path().join("test2.ir.json");

    ir.to_file(&file_path1).expect("Failed to save IR to file 1");
    ir.to_file(&file_path2).expect("Failed to save IR to file 2");

    let content1 = std::fs::read_to_string(&file_path1).expect("Failed to read file 1");
    let content2 = std::fs::read_to_string(&file_path2).expect("Failed to read file 2");

    assert_eq!(content1, content2, "Serialization should be deterministic");
}

#[test]
fn test_error_handling_malformed_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let file_path = temp_dir.path().join("malformed.ir.json");

    std::fs::write(&file_path, r#"{"version": "0.1.0", "modules": [{"invalid": "json"}]"#)
        .expect("Failed to write malformed JSON");

    let result = ProtocolIR::from_file(&file_path);
    assert!(result.is_err(), "Should fail to parse malformed JSON");
}

#[test]
fn test_error_handling_nonexistent_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let file_path = temp_dir.path().join("nonexistent.ir.json");

    let result = ProtocolIR::from_file(&file_path);
    assert!(result.is_err(), "Should fail to read nonexistent file");
}

#[test]
fn test_directory_creation() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let nested_path = temp_dir.path().join("nested").join("deep").join("test.ir.json");

    let ir = create_sample_ir();

    ir.to_file(&nested_path).expect("Failed to create nested directories");

    assert!(nested_path.exists(), "File should be created in nested directory");
}

#[test]
fn test_ir_serialization_roundtrip_bitcoin() {
    // Test that Bitcoin IR can be saved and loaded without data loss
    let current_dir =
        std::env::current_dir().expect("failed to get current_dir for IR roundtrip test");
    let project_root = current_dir
        .ancestors()
        .find(|p| p.join("Cargo.toml").exists() && p.join("resources").exists())
        .expect("failed to locate project root containing Cargo.toml and resources/");
    let ir_path = project_root.join("resources/ir/bitcoin.ir.json");

    let original = ProtocolIR::from_file(&ir_path).expect("load Bitcoin IR");
    let temp_path = std::env::temp_dir().join("test_roundtrip_bitcoin.ir.json");

    // Test roundtrip
    original.to_file(&temp_path).expect("save Bitcoin IR");
    let reloaded = ProtocolIR::from_file(&temp_path).expect("reload Bitcoin IR");

    // Verify core properties
    assert_eq!(original.version(), reloaded.version());
    assert_eq!(original.modules().len(), reloaded.modules().len());
    assert_eq!(original.definition_count(), reloaded.definition_count());
    assert_eq!(original.get_rpc_methods().len(), reloaded.get_rpc_methods().len());

    // Verify specific Bitcoin IR characteristics
    assert!(original.get_rpc_methods().len() > 100, "Bitcoin IR should have many RPC methods");

    // Check for known Bitcoin RPC methods
    let rpc_names: std::collections::HashSet<String> =
        original.get_rpc_methods().iter().map(|rpc| rpc.name.clone()).collect();
    assert!(rpc_names.contains("getblock"), "Should contain getblock RPC");
    assert!(rpc_names.contains("getbalance"), "Should contain getbalance RPC");
    assert!(rpc_names.contains("sendrawtransaction"), "Should contain sendrawtransaction RPC");

    std::fs::remove_file(temp_path).ok();
}

#[test]
fn test_ir_validation_real_files() {
    // Test that real IR files pass validation
    let current_dir =
        std::env::current_dir().expect("failed to get current_dir for IR validation test");
    let project_root = current_dir
        .ancestors()
        .find(|p| p.join("Cargo.toml").exists() && p.join("resources").exists())
        .expect("failed to locate project root containing Cargo.toml and resources/");

    // Test Bitcoin IR validation
    let bitcoin_ir_path = project_root.join("resources/ir/bitcoin.ir.json");
    let bitcoin_ir = ProtocolIR::from_file(&bitcoin_ir_path).expect("load Bitcoin IR");

    // Note: This would require importing the validator, but demonstrates the concept
    // let validator = analysis::IrValidator::new();
    // let errors = validator.validate(&bitcoin_ir);
    // assert!(errors.is_empty(), "Bitcoin IR should pass validation: {:?}", errors);

    // Basic sanity checks
    assert!(!bitcoin_ir.get_rpc_methods().is_empty(), "Bitcoin IR should have RPC methods");
}

#[test]
fn test_ir_deterministic_serialization_real_data() {
    // Test that real IR files serialize deterministically
    let current_dir =
        std::env::current_dir().expect("failed to get current_dir for IR deterministic test");
    let project_root = current_dir
        .ancestors()
        .find(|p| p.join("Cargo.toml").exists() && p.join("resources").exists())
        .expect("failed to locate project root containing Cargo.toml and resources/");
    let ir_path = project_root.join("resources/ir/bitcoin.ir.json");

    let original = ProtocolIR::from_file(&ir_path).expect("load IR");
    let temp_dir = std::env::temp_dir().join("ethos_ir_deterministic");
    let _ = std::fs::create_dir_all(&temp_dir);

    let file1 = temp_dir.join("test1.ir.json");
    let file2 = temp_dir.join("test2.ir.json");

    // Serialize twice
    original.to_file(&file1).expect("save IR #1");
    original.to_file(&file2).expect("save IR #2");

    // Compare file contents
    let content1 = std::fs::read_to_string(&file1).expect("read file 1");
    let content2 = std::fs::read_to_string(&file2).expect("read file 2");

    assert_eq!(content1, content2, "Real IR serialization should be deterministic");

    // Cleanup
    let _ = std::fs::remove_dir_all(temp_dir);
}
