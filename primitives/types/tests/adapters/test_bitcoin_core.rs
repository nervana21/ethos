use ir::{AccessLevel, RpcDef};
use types::adapters::bitcoin_core::BitcoinCoreAdapter;
use types::type_adapter::TypeAdapter;
use types::MethodResult;

/// Helper function to create test RpcDef with specified results
fn create_test_rpc_def(results: Vec<MethodResult>) -> RpcDef {
    RpcDef {
        name: "test_method".to_string(),
        description: "A test method".to_string(),
        params: vec![],
        result: if results.is_empty() {
            None
        } else {
            Some(convert_method_result_to_type_def(&results[0]))
        },
        category: "test".to_string(),
        access_level: AccessLevel::default(),
        requires_private_keys: false,
        examples: None,
        hidden: None,
        version_added: None,
        version_removed: None,
    }
}

/// Helper function to convert MethodResult to TypeDef
fn convert_method_result_to_type_def(result: &MethodResult) -> ir::TypeDef {
    // Convert inner fields to FieldDef format
    let fields = if !result.inner.is_empty() {
        Some(
            result
                .inner
                .iter()
                .map(|inner| ir::FieldDef {
                    name: inner.key_name.clone(),
                    field_type: ir::TypeDef {
                        name: inner.type_.clone(),
                        description: inner.description.clone(),
                        kind: ir::TypeKind::Primitive,
                        fields: None,
                        variants: None,
                        base_type: Some(inner.type_.clone()),
                        protocol_type: None,
                        canonical_name: None,
                        condition: None,
                    },
                    required: !inner.optional,
                    description: inner.description.clone(),
                    default_value: None,
                })
                .collect(),
        )
    } else {
        None
    };

    ir::TypeDef {
        name: result.type_.clone(),
        description: result.description.clone(),
        kind: if fields.is_some() { ir::TypeKind::Object } else { ir::TypeKind::Primitive },
        fields,
        variants: None,
        base_type: Some(result.type_.clone()),
        protocol_type: None,
        canonical_name: None,
        condition: None,
    }
}

/// Helper function to create a MethodResult with common defaults
fn create_method_result(
    type_: &str,
    key_name: &str,
    description: &str,
    optional: bool,
) -> MethodResult {
    MethodResult {
        type_: type_.to_string(),
        optional,
        description: description.to_string(),
        key_name: key_name.to_string(),
        condition: String::new(),
        inner: vec![],
    }
}

#[test]
fn test_protocol_name() {
    let adapter = BitcoinCoreAdapter;
    assert_eq!(adapter.protocol_name(), "bitcoin_core");
}

#[test]
fn test_parse_response_schema() {
    let adapter = BitcoinCoreAdapter;

    let method_empty = create_test_rpc_def(vec![]);
    assert!(adapter.parse_response_schema(&method_empty).is_none());

    let primitive_result = create_method_result("string", "result", "A primitive result", false);
    let method_primitive = create_test_rpc_def(vec![primitive_result.clone()]);
    let parsed = adapter.parse_response_schema(&method_primitive);
    let parsed_results = parsed.unwrap();
    assert_eq!(parsed_results.len(), 1);
    assert_eq!(parsed_results[0].key_name, ""); // key_name should be stripped
    assert_eq!(parsed_results[0].type_, "string");

    let inner_result = create_method_result("string", "inner_field", "Inner field", false);
    let object_result = MethodResult {
        type_: "object".to_string(),
        optional: false,
        description: "An object result".to_string(),
        key_name: "data".to_string(),
        condition: String::new(),
        inner: vec![inner_result],
    };
    let method_object = create_test_rpc_def(vec![object_result.clone()]);
    let parsed = adapter.parse_response_schema(&method_object);
    assert!(parsed.is_some());
    let parsed_results = parsed.unwrap();
    assert_eq!(parsed_results.len(), 1);
    assert_eq!(parsed_results[0].key_name, ""); // key_name is empty for top-level results
    assert_eq!(parsed_results[0].inner.len(), 1);

    // Test with multiple fields in a single object
    let inner_result1 = create_method_result("string", "field1", "First result", false);
    let inner_result2 = create_method_result("number", "field2", "Second result", false);
    let object_result = MethodResult {
        type_: "object".to_string(),
        optional: false,
        description: "An object with multiple fields".to_string(),
        key_name: "data".to_string(),
        condition: String::new(),
        inner: vec![inner_result1, inner_result2],
    };
    let method_multiple = create_test_rpc_def(vec![object_result]);
    let parsed = adapter.parse_response_schema(&method_multiple);
    assert!(parsed.is_some());
    let parsed_results = parsed.unwrap();
    assert_eq!(parsed_results.len(), 1);
    assert_eq!(parsed_results[0].inner.len(), 2);
    assert_eq!(parsed_results[0].inner[0].key_name, "field1");
    assert_eq!(parsed_results[0].inner[1].key_name, "field2");

    let no_key_result = create_method_result("string", "", "No key result", false);
    let method_no_key = create_test_rpc_def(vec![no_key_result.clone()]);
    let parsed = adapter.parse_response_schema(&method_no_key);
    assert!(parsed.is_some());
    let parsed_results = parsed.unwrap();
    assert_eq!(parsed_results.len(), 1);
    assert_eq!(parsed_results[0].key_name, ""); // Should remain empty
}

#[test]
fn test_map_type_to_rust() {
    let adapter = BitcoinCoreAdapter;

    let bitcoin_float_fields = vec![
        "difficulty",
        "verificationprogress",
        "relayfee",
        "incrementalfee",
        "incrementalrelayfee",
        "networkhashps",
    ];
    for field in bitcoin_float_fields {
        let result = create_method_result("number", field, &format!("{} field", field), false);
        assert_eq!(adapter.map_type_to_rust(&result), "f64");
    }

    let hex_result = create_method_result("hex", "txid", "A hex value", false);
    assert_eq!(adapter.map_type_to_rust(&hex_result), "String");

    // Test number with "difficulty" in description and empty key_name → f64
    let difficulty_result = create_method_result("number", "", "Current difficulty value", false);
    assert_eq!(adapter.map_type_to_rust(&difficulty_result), "f64");

    let standard_mappings = vec![
        ("string", "String"),
        ("number", "i64"),
        ("int", "i64"),
        ("integer", "i64"),
        ("boolean", "bool"),
        ("bool", "bool"),
        ("array", "Vec<serde_json::Value>"),
        ("object", "serde_json::Value"),
        ("none", "()"),
    ];
    for (input_type, expected_rust_type) in standard_mappings {
        let result = create_method_result(
            input_type,
            "test_field",
            &format!("A {} value", input_type),
            false,
        );
        assert_eq!(adapter.map_type_to_rust(&result), expected_rust_type);
    }

    let unknown_result =
        create_method_result("unknown_type", "unknown_field", "An unknown type", false);
    assert_eq!(adapter.map_type_to_rust(&unknown_result), "serde_json::Value");
}

// Removed test_has_strongly_typed_response - method no longer exists
