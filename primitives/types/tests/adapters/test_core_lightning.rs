use ir::{AccessLevel, RpcDef};
use types::adapters::core_lightning::CoreLightningAdapter;
use types::type_adapter::TypeAdapter;
use types::MethodResult;

/// Helper function to create test RpcDef with specified results and raw fields
fn create_test_rpc_def(
    results: Vec<MethodResult>,
    _raw: serde_json::Map<String, serde_json::Value>,
) -> RpcDef {
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
    ir::TypeDef {
        name: result.type_.clone(),
        description: result.description.clone(),
        kind: ir::TypeKind::Primitive,
        fields: None,
        variants: None,
        base_type: Some(result.type_.clone()),
        protocol_type: None,
        canonical_name: None,
        condition: None,
    }
}

/// Helper function to create a MethodResult
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
    let adapter = CoreLightningAdapter;
    assert_eq!(adapter.protocol_name(), "core_lightning");
}

#[test]
fn test_parse_response_schema() {
    let adapter = CoreLightningAdapter;

    let method_empty = create_test_rpc_def(vec![], serde_json::Map::new());
    assert!(adapter.parse_response_schema(&method_empty).is_none());

    // Create a TypeDef with fields for the response schema
    let id_field = ir::FieldDef {
        name: "id".to_string(),
        field_type: ir::TypeDef {
            name: "string".to_string(),
            description: "Channel ID".to_string(),
            kind: ir::TypeKind::Primitive,
            fields: None,
            variants: None,
            base_type: None,
            protocol_type: None,
            canonical_name: None,
            condition: None,
        },
        required: true,
        description: "Channel ID".to_string(),
        default_value: None,
    };

    let amount_field = ir::FieldDef {
        name: "amount".to_string(),
        field_type: ir::TypeDef {
            name: "u64".to_string(),
            description: "Channel amount in satoshis".to_string(),
            kind: ir::TypeKind::Primitive,
            fields: None,
            variants: None,
            base_type: None,
            protocol_type: None,
            canonical_name: None,
            condition: None,
        },
        required: true,
        description: "Channel amount in satoshis".to_string(),
        default_value: None,
    };

    let optional_field = ir::FieldDef {
        name: "optional_field".to_string(),
        field_type: ir::TypeDef {
            name: "string".to_string(),
            description: "Optional field".to_string(),
            kind: ir::TypeKind::Primitive,
            fields: None,
            variants: None,
            base_type: None,
            protocol_type: None,
            canonical_name: None,
            condition: None,
        },
        required: false,
        description: "Optional field".to_string(),
        default_value: None,
    };

    let response_type = ir::TypeDef {
        name: "object".to_string(),
        description: "Response object".to_string(),
        kind: ir::TypeKind::Object,
        fields: Some(vec![id_field, amount_field, optional_field]),
        variants: None,
        base_type: None,
        protocol_type: None,
        canonical_name: None,
        condition: None,
    };

    let method_with_response = RpcDef {
        name: "test_method".to_string(),
        description: "A test method".to_string(),
        params: vec![],
        result: Some(response_type),
        category: "test".to_string(),
        access_level: AccessLevel::default(),
        requires_private_keys: false,
        examples: None,
        hidden: None,
        version_added: None,
        version_removed: None,
    };
    let parsed = adapter.parse_response_schema(&method_with_response);
    assert!(parsed.is_some());
    let parsed_results = parsed.unwrap();
    assert_eq!(parsed_results.len(), 1);
    assert_eq!(parsed_results[0].inner.len(), 3);

    let id_result = parsed_results[0].inner.iter().find(|r| r.key_name == "id").unwrap();
    assert!(!id_result.optional);
    assert_eq!(id_result.type_, "string");

    let amount_result = parsed_results[0].inner.iter().find(|r| r.key_name == "amount").unwrap();
    assert!(!amount_result.optional);
    assert_eq!(amount_result.type_, "u64");

    let optional_result =
        parsed_results[0].inner.iter().find(|r| r.key_name == "optional_field").unwrap();
    assert!(optional_result.optional);

    // Test deeply nested valid schema - simplified for current implementation
    let deep_field = ir::FieldDef {
        name: "level5".to_string(),
        field_type: ir::TypeDef {
            name: "string".to_string(),
            description: "Deep field".to_string(),
            kind: ir::TypeKind::Primitive,
            fields: None,
            variants: None,
            base_type: None,
            protocol_type: None,
            canonical_name: None,
            condition: None,
        },
        required: true,
        description: "Deep field".to_string(),
        default_value: None,
    };

    let deep_type = ir::TypeDef {
        name: "object".to_string(),
        description: "Deep object".to_string(),
        kind: ir::TypeKind::Object,
        fields: Some(vec![deep_field]),
        variants: None,
        base_type: None,
        protocol_type: None,
        canonical_name: None,
        condition: None,
    };

    let method_deep = RpcDef {
        name: "test_method".to_string(),
        description: "A test method".to_string(),
        params: vec![],
        result: Some(deep_type),
        category: "test".to_string(),
        access_level: AccessLevel::default(),
        requires_private_keys: false,
        examples: None,
        hidden: None,
        version_added: None,
        version_removed: None,
    };
    let parsed_deep = adapter.parse_response_schema(&method_deep);
    assert!(parsed_deep.is_some());
    assert_eq!(parsed_deep.unwrap().len(), 1);

    // All malformed schema tests removed - not applicable to current TypeDef-based implementation
}

#[test]
fn test_map_type_to_rust() {
    let adapter = CoreLightningAdapter;

    let lightning_amount_types = vec!["sat", "satoshi", "satoshis", "msat", "millisatoshis"];
    for amount_type in lightning_amount_types {
        let result =
            create_method_result(amount_type, "amount", &format!("{} field", amount_type), false);
        assert_eq!(adapter.map_type_to_rust(&result), "u64");
    }

    let hex_result = create_method_result("hex", "txid", "A hex value", false);
    assert_eq!(adapter.map_type_to_rust(&hex_result), "String");

    let u32_result = create_method_result("u32", "count", "A u32 value", false);
    assert_eq!(adapter.map_type_to_rust(&u32_result), "u32");

    let u64_result = create_method_result("u64", "timestamp", "A u64 value", false);
    assert_eq!(adapter.map_type_to_rust(&u64_result), "u64");

    let standard_mappings = vec![
        ("string", "String"),
        ("number", "i64"),
        ("integer", "i64"),
        ("boolean", "bool"),
        ("bool", "bool"),
        ("object", "serde_json::Value"),
        ("array", "serde_json::Value"),
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
}

#[test]
#[should_panic(expected = "Unmapped Core Lightning result type")]
fn test_map_type_to_rust_unknown_panics() {
    let adapter = CoreLightningAdapter;
    let unknown_result =
        create_method_result("unknown_type", "unknown_field", "An unknown type", false);
    // This should panic with an informative message to surface unmapped types early.
    let _ = adapter.map_type_to_rust(&unknown_result);
}

// Removed test_has_strongly_typed_response - method no longer exists
