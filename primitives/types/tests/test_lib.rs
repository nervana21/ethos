use types::*;

#[test]
fn test_method_result_new() {
    let default_result = MethodResult::default();
    assert_eq!(default_result.type_, "");
    assert!(!default_result.optional);
    assert_eq!(default_result.description, "");
    assert_eq!(default_result.key_name, "");
    assert_eq!(default_result.condition, "");
    assert_eq!(default_result.inner.len(), 0);

    let inner_results = vec![MethodResult {
        type_: "string".to_string(),
        optional: true,
        description: "inner result".to_string(),
        key_name: "inner_key".to_string(),
        condition: "when_available".to_string(),
        inner: Vec::new(),
    }];

    let result = MethodResult::new(
        "object".to_string(),
        true,
        "Test result description".to_string(),
        "test_key".to_string(),
        "when_condition".to_string(),
        inner_results.clone(),
    );

    assert_eq!(result.type_, "object");
    assert!(result.optional);
    assert_eq!(result.description, "Test result description");
    assert_eq!(result.key_name, "test_key");
    assert_eq!(result.condition, "when_condition");
    assert_eq!(result.inner.len(), inner_results.len());
    assert_eq!(result.inner[0].type_, inner_results[0].type_);
}

#[test]
fn test_method_result_required() {
    let optional_result = MethodResult {
        type_: "string".to_string(),
        optional: true,
        description: "".to_string(),
        key_name: "".to_string(),
        condition: "".to_string(),
        inner: Vec::new(),
    };

    let required_result = MethodResult {
        type_: "string".to_string(),
        optional: false,
        description: "".to_string(),
        key_name: "".to_string(),
        condition: "".to_string(),
        inner: Vec::new(),
    };

    assert!(!optional_result.required());
    assert!(required_result.required());
}

#[test]
fn test_type_registry_map_argument_type() {
    let string_arg = Argument {
        names: vec!["name".to_string()],
        description: "Test argument".to_string(),
        oneline_description: "Test".to_string(),
        also_positional: false,
        type_str: None,
        required: true,
        hidden: false,
        type_: "string".to_string(),
    };
    let (type_name, is_optional) = TypeRegistry::map_argument_type(&string_arg);
    assert_eq!(type_name, "String");
    assert!(!is_optional);

    let number_arg = Argument {
        names: vec!["count".to_string()],
        description: "Count argument".to_string(),
        oneline_description: "Count".to_string(),
        also_positional: false,
        type_str: None,
        required: false,
        hidden: false,
        type_: "number".to_string(),
    };
    let (type_name, is_optional) = TypeRegistry::map_argument_type(&number_arg);
    assert_eq!(type_name, "i64");
    assert!(is_optional);

    let bool_arg = Argument {
        names: vec!["flag".to_string()],
        description: "Flag argument".to_string(),
        oneline_description: "Flag".to_string(),
        also_positional: false,
        type_str: None,
        required: true,
        hidden: false,
        type_: "boolean".to_string(),
    };
    let (type_name, is_optional) = TypeRegistry::map_argument_type(&bool_arg);
    assert_eq!(type_name, "bool");
    assert!(!is_optional);

    let object_arg = Argument {
        names: vec!["data".to_string()],
        description: "Data argument".to_string(),
        oneline_description: "Data".to_string(),
        also_positional: false,
        type_str: None,
        required: true,
        hidden: false,
        type_: "object".to_string(),
    };
    let (type_name, is_optional) = TypeRegistry::map_argument_type(&object_arg);
    assert_eq!(type_name, "serde_json::Value");
    assert!(!is_optional);

    let array_arg = Argument {
        names: vec!["items".to_string()],
        description: "Items argument".to_string(),
        oneline_description: "Items".to_string(),
        also_positional: false,
        type_str: None,
        required: false,
        hidden: false,
        type_: "array".to_string(),
    };
    let (type_name, is_optional) = TypeRegistry::map_argument_type(&array_arg);
    assert_eq!(type_name, "Vec<serde_json::Value>");
    assert!(is_optional);

    let unknown_arg = Argument {
        names: vec!["unknown".to_string()],
        description: "Unknown argument".to_string(),
        oneline_description: "Unknown".to_string(),
        also_positional: false,
        type_str: None,
        required: false,
        hidden: false,
        type_: "unknown_type".to_string(),
    };
    // Unknown types should panic rather than silently falling back
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        TypeRegistry::map_argument_type(&unknown_arg)
    }));
    assert!(result.is_err(), "Expected panic for unknown type 'unknown_type'");
}

#[test]
fn test_type_registry_map_result_type() {
    use crate::adapters::BitcoinCoreAdapter;
    let adapter = BitcoinCoreAdapter;

    let result = MethodResult {
        type_: "string".to_string(),
        optional: true,
        description: "Test result".to_string(),
        key_name: "test_key".to_string(),
        condition: "".to_string(),
        inner: Vec::new(),
    };
    let (type_name, is_optional) = TypeRegistry::map_result_type(&result, &adapter);
    assert_eq!(type_name, "String");
    assert!(is_optional);

    let required_result = MethodResult {
        type_: "number".to_string(),
        optional: false,
        description: "Required result".to_string(),
        key_name: "required_key".to_string(),
        condition: "".to_string(),
        inner: Vec::new(),
    };
    let (type_name, is_optional) = TypeRegistry::map_result_type(&required_result, &adapter);
    assert_eq!(type_name, "i64");
    assert!(!is_optional);
}
