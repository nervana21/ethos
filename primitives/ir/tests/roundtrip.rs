//! Round-trip tests for ProtocolIR serialization
//!
//! Tests that IR can be serialized and deserialized without data loss,
//! and that serialization is deterministic.

use ethos_ir::{AccessLevel, ProtocolDef, ProtocolIR, ProtocolModule, RpcDef, TypeDef, TypeKind};
use tempfile::TempDir;

/// Create a sample ProtocolIR for testing
fn create_sample_ir() -> ProtocolIR {
    let rpc_def = RpcDef {
        name: "getblock".to_string(),
        description: "Get block by hash".to_string(),
        params: vec![],
        result: Some(TypeDef {
            name: "BlockInfo".to_string(),
            description: "Block information".to_string(),
            kind: TypeKind::Object,
            fields: Some(vec![]),
            variants: None,
            base_type: None,
            protocol_type: Some("object".to_string()),
            canonical_name: None,
            condition: None,
        }),
        category: "node".to_string(),
        access_level: AccessLevel::default(),
        requires_private_keys: false,
        examples: None,
        hidden: None,
        version_added: None,
        version_removed: None,
    };

    let module = ProtocolModule::from_source(
        "rpc",
        "Bitcoin RPC API",
        vec![ProtocolDef::RpcMethod(rpc_def)],
        "bitcoin_core",
    );

    ProtocolIR::new_with_version("0.1.0".to_string(), vec![module])
}

#[test]
fn test_roundtrip_serialization() {
    let original_ir = create_sample_ir();
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let file_path = temp_dir.path().join("test.ir.json");

    // Save to file
    original_ir.to_file(&file_path).expect("Failed to save IR to file");

    // Load from file
    let loaded_ir = ProtocolIR::from_file(&file_path).expect("Failed to load IR from file");

    // Verify they are equal
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

    // Save twice
    ir.to_file(&file_path1).expect("Failed to save IR to file 1");
    ir.to_file(&file_path2).expect("Failed to save IR to file 2");

    // Read both files and compare content
    let content1 = std::fs::read_to_string(&file_path1).expect("Failed to read file 1");
    let content2 = std::fs::read_to_string(&file_path2).expect("Failed to read file 2");

    assert_eq!(content1, content2, "Serialization should be deterministic");
}

#[test]
fn test_error_handling_malformed_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let file_path = temp_dir.path().join("malformed.ir.json");

    // Write malformed JSON
    std::fs::write(&file_path, r#"{"version": "0.1.0", "modules": [{"invalid": "json"}]"#)
        .expect("Failed to write malformed JSON");

    // Should return an error
    let result = ProtocolIR::from_file(&file_path);
    assert!(result.is_err(), "Should fail to parse malformed JSON");
}

#[test]
fn test_error_handling_nonexistent_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let file_path = temp_dir.path().join("nonexistent.ir.json");

    // Should return an error
    let result = ProtocolIR::from_file(&file_path);
    assert!(result.is_err(), "Should fail to read nonexistent file");
}

#[test]
fn test_directory_creation() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let nested_path = temp_dir.path().join("nested").join("deep").join("test.ir.json");

    let ir = create_sample_ir();

    // Should create directories automatically
    ir.to_file(&nested_path).expect("Failed to create nested directories");

    assert!(nested_path.exists(), "File should be created in nested directory");
}
