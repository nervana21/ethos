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
fn test_api_definition_with_implementation_and_version() {
	// Test with empty version string - should fail to parse
	let empty_version_result = ProtocolVersion::from_string("");
	assert!(empty_version_result.is_err());

	// Test with whitespace version string - should fail to parse
	let whitespace_version_result = ProtocolVersion::from_string("   ");
	assert!(whitespace_version_result.is_err());

	// Test with valid version
	let valid_api = ApiDefinition::with_implementation_and_version(
		Implementation::BitcoinCore,
		ProtocolVersion::from_string("1.0.0").unwrap(),
	)
	.expect("Valid implementation and version should succeed");
	assert_eq!(valid_api.protocol, Protocol::Bitcoin);
	assert_eq!(valid_api.version.as_str(), "1.0.0");
	assert_eq!(valid_api.methods.len(), 0);
	assert!(valid_api.methods.is_empty());
}

#[test]
fn test_api_definition_get_method() {
	let mut api = ApiDefinition::default();
	let method_json = serde_json::json!({
		"name": "test_method",
		"description": "Test method",
		"examples": "",
		"argument_names": [],
		"arguments": [],
		"results": []
	});

	assert!(api.get_method("non_existent").is_none());

	api.add_method("test_method".to_string(), method_json);
	let retrieved_method = api.get_method("test_method");
	assert!(retrieved_method.is_some());
	assert_eq!(retrieved_method.unwrap()["name"], "test_method");
}

#[test]
fn test_api_definition_get_method_mut() {
	let mut api = ApiDefinition::default();
	let method_json = serde_json::json!({
		"name": "test_method",
		"description": "Test method description",
		"examples": "",
		"argument_names": [],
		"arguments": [],
		"results": []
	});

	api.add_method("test_method".to_string(), method_json);

	let retrieved_method = api.get_method_mut("test_method");
	assert!(retrieved_method.is_some());

	if let Some(method) = retrieved_method {
		assert_eq!(method["description"], "Test method description");
		method["description"] =
			serde_json::Value::String("Modified test method description".to_string());
		assert_eq!(method["description"], "Modified test method description");
	}
}

#[test]
fn test_api_definition_add_method() {
	let mut api = ApiDefinition::default();
	let method_json = serde_json::json!({
		"name": "test_method",
		"description": "Test method",
		"examples": "",
		"argument_names": [],
		"arguments": [],
		"results": []
	});

	assert_eq!(api.method_count(), 0);
	api.add_method("test_method".to_string(), method_json);
	assert_eq!(api.method_count(), 1);
	assert!(api.get_method("test_method").is_some());
}

#[test]
fn test_api_definition_remove_method() {
	let mut api = ApiDefinition::default();
	let method_json = serde_json::json!({
		"name": "test_method",
		"description": "Test method",
		"examples": "",
		"argument_names": [],
		"arguments": [],
		"results": []
	});

	api.add_method("test_method".to_string(), method_json);
	assert_eq!(api.method_count(), 1);

	let removed_method = api.remove_method("test_method");
	assert!(removed_method.is_some());
	assert_eq!(removed_method.unwrap()["name"], "test_method");
	assert_eq!(api.method_count(), 0);

	let non_existent = api.remove_method("non_existent");
	assert!(non_existent.is_none());
}

#[test]
fn test_api_definition_iter_methods() {
	let mut api = ApiDefinition::default();
	let method1_json = serde_json::json!({
		"name": "method1",
		"description": "First method",
		"examples": "",
		"argument_names": [],
		"arguments": [],
		"results": []
	});

	let method2_json = serde_json::json!({
		"name": "method2",
		"description": "Second method",
		"examples": "",
		"argument_names": [],
		"arguments": [],
		"results": []
	});

	api.add_method("method1".to_string(), method1_json);
	api.add_method("method2".to_string(), method2_json);

	let mut method_names: Vec<String> = api.iter_methods().map(|(name, _)| name.clone()).collect();
	method_names.sort();

	assert_eq!(method_names, vec!["method1", "method2"]);
}

#[test]
fn test_api_definition_method_count() {
	let mut api = ApiDefinition::default();
	assert_eq!(api.method_count(), 0);

	let method_json = serde_json::json!({
		"name": "test_method",
		"description": "Test method",
		"examples": "",
		"argument_names": [],
		"arguments": [],
		"results": []
	});

	api.add_method("test_method".to_string(), method_json);
	assert_eq!(api.method_count(), 1);

	let method2_json = serde_json::json!({
		"name": "test_method2",
		"description": "Test method 2",
		"examples": "",
		"argument_names": [],
		"arguments": [],
		"results": []
	});

	api.add_method("test_method2".to_string(), method2_json);
	assert_eq!(api.method_count(), 2);
}

#[test]
fn test_api_definition_is_empty() {
	let mut api = ApiDefinition::default();
	assert!(api.is_empty());

	let method_json = serde_json::json!({
		"name": "test_method",
		"description": "Test method",
		"examples": "",
		"argument_names": [],
		"arguments": [],
		"results": []
	});

	api.add_method("test_method".to_string(), method_json);
	assert!(!api.is_empty());
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
