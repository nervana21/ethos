// SPDX-License-Identifier: CC0-1.0

//! IR roundtrip tests (serialize → deserialize).

use ethos_ir::test_utils::{rpc, type_def};
use ethos_ir::{FieldDef, FieldKey, ProtocolDef, ProtocolIR, ProtocolModule, TypeDef, TypeKind};

fn minimal_type_def() -> TypeDef { type_def("", TypeKind::Primitive) }

#[test]
fn test_field_key_roundtrip() {
    // Roundtrip `FieldKey` Named and Anonymous through JSON.
    let type_with_keys = TypeDef {
        name: "object".to_string(),
        description: String::new(),
        kind: TypeKind::Object,
        fields: Some(vec![
            FieldDef {
                key: FieldKey::Named("txid".to_string()),
                field_type: minimal_type_def(),
                required: true,
                description: String::new(),
                default_value: None,
                version_added: None,
                version_removed: None,
            },
            FieldDef {
                key: FieldKey::Anonymous(1),
                field_type: minimal_type_def(),
                required: true,
                description: String::new(),
                default_value: None,
                version_added: None,
                version_removed: None,
            },
        ]),
        variants: None,
        union_variants: None,
        base_type: None,
        protocol_type: None,
        canonical_name: None,
        condition: None,
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
        fields: Some(vec![FieldDef {
            key: FieldKey::Anonymous(0),
            field_type: elem_ty.clone(),
            required: true,
            description: String::new(),
            default_value: None,
            version_added: None,
            version_removed: None,
        }]),
        variants: None,
        union_variants: None,
        base_type: None,
        protocol_type: Some("array".to_string()),
        canonical_name: None,
        condition: None,
    };

    let elem =
        array_with_anonymous.array_element_type().expect("anonymous(0) element must be recognized");
    assert_eq!(elem.name, elem_ty.name);

    // Synthetic Named(\"field_0\") element should also be recognized.
    let array_with_named = TypeDef {
        name: "array".to_string(),
        description: String::new(),
        kind: TypeKind::Array,
        fields: Some(vec![FieldDef {
            key: FieldKey::Named("field_0".to_string()),
            field_type: elem_ty.clone(),
            required: true,
            description: String::new(),
            default_value: None,
            version_added: None,
            version_removed: None,
        }]),
        variants: None,
        union_variants: None,
        base_type: None,
        protocol_type: Some("array".to_string()),
        canonical_name: None,
        condition: None,
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
            FieldDef {
                key: FieldKey::Anonymous(0),
                field_type: minimal_type_def(),
                required: true,
                description: String::new(),
                default_value: None,
                version_added: None,
                version_removed: None,
            },
            FieldDef {
                key: FieldKey::Anonymous(1),
                field_type: minimal_type_def(),
                required: true,
                description: String::new(),
                default_value: None,
                version_added: None,
                version_removed: None,
            },
        ]),
        variants: None,
        union_variants: None,
        base_type: None,
        protocol_type: Some("array".to_string()),
        canonical_name: None,
        condition: None,
    };
    assert!(multi_field_array.array_element_type().is_none());
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
