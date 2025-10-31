//! Comprehensive unit tests for the DifferentialAnalyzer module

use std::collections::HashMap;

use async_trait::async_trait;
use ethos_analysis::DifferentialAnalyzer;
use fuzz_types::{FuzzCase, FuzzResult, ProtocolAdapter};
use serde_json::{json, Value};

// Mock adapter for testing
struct MockAdapter {
    name: &'static str,
    should_succeed: bool,
    response: Value,
    execution_delay_ms: u64,
    error_message: Option<String>,
}

#[async_trait]
impl ProtocolAdapter for MockAdapter {
    fn name(&self) -> &'static str { self.name }

    async fn apply_fuzz_case(
        &self,
        _case: &FuzzCase,
    ) -> Result<FuzzResult, Box<dyn std::error::Error + Send + Sync>> {
        // Simulate execution delay using std::thread::sleep for simplicity
        if self.execution_delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(self.execution_delay_ms));
        }

        Ok(FuzzResult {
            adapter_name: self.name.to_string(),
            raw_response: self.response.clone(),
            success: self.should_succeed,
            error: if self.should_succeed {
                None
            } else {
                Some(self.error_message.clone().unwrap_or_else(|| "Mock error".to_string()))
            },
            normalized_error: None,
            execution_time_ms: self.execution_delay_ms,
        })
    }

    fn normalize_output(&self, value: &Value) -> Value { value.clone() }
}

// Helper functions for creating test data
fn create_test_fuzz_case() -> FuzzCase {
    let mut parameters = HashMap::new();
    parameters.insert("test_param".to_string(), Value::String("test_value".to_string()));

    FuzzCase {
        method_name: "test_method".to_string(),
        parameters,
        expected_result_type: Some("object".to_string()),
    }
}

/// Create a test adapter for differential analysis
fn create_test_adapter(
    name: &'static str,
    should_succeed: bool,
    response: Value,
) -> Box<dyn ProtocolAdapter> {
    Box::new(MockAdapter {
        name,
        should_succeed,
        response,
        execution_delay_ms: 0,
        error_message: None,
    })
}

fn create_test_adapter_with_error(
    name: &'static str,
    should_succeed: bool,
    response: Value,
    error_message: Option<String>,
) -> Box<dyn ProtocolAdapter> {
    Box::new(MockAdapter { name, should_succeed, response, execution_delay_ms: 0, error_message })
}

#[test]
fn test_new() {
    let empty_adapters: Vec<Box<dyn ProtocolAdapter>> = vec![];
    let empty_analyzer = DifferentialAnalyzer::new(empty_adapters);
    assert_eq!(empty_analyzer.adapter_count(), 0);

    let single_adapters: Vec<Box<dyn ProtocolAdapter>> =
        vec![create_test_adapter("single_adapter", true, Value::Null)];
    let single_analyzer = DifferentialAnalyzer::new(single_adapters);
    assert_eq!(single_analyzer.adapter_count(), 1);

    let adapters: Vec<Box<dyn ProtocolAdapter>> = vec![
        create_test_adapter("adapter1", true, Value::Null),
        create_test_adapter("adapter2", true, Value::Null),
    ];
    let analyzer = DifferentialAnalyzer::new(adapters);
    assert_eq!(analyzer.adapter_count(), 2);
}

#[test]
fn test_run_fuzz_case() {
    let fuzz_case = create_test_fuzz_case();

    let adapters: Vec<Box<dyn ProtocolAdapter>> = vec![
        create_test_adapter("adapter1", true, json!({"result": "success"})),
        create_test_adapter("adapter2", true, json!({"result": "success"})),
    ];
    let analyzer = DifferentialAnalyzer::new(adapters);
    let result = analyzer.run_fuzz_case(&fuzz_case);
    assert_eq!(result.fuzz_case.method_name, "test_method");
    assert_eq!(result.adapter_results.len(), 2);
    assert!(result.equivalent);
    assert!(result.differences.is_empty());
    assert!(result.summary.contains("equivalent"));

    let error_adapters: Vec<Box<dyn ProtocolAdapter>> = vec![
        create_test_adapter("error_adapter1", false, Value::Null),
        create_test_adapter("error_adapter2", false, Value::Null),
    ];
    let error_analyzer = DifferentialAnalyzer::new(error_adapters);
    let error_result = error_analyzer.run_fuzz_case(&fuzz_case);
    assert_eq!(error_result.adapter_results.len(), 2);
    assert!(error_result.equivalent);
    assert!(error_result.differences.is_empty());
    assert!(error_result.summary.contains("equivalent"));

    let different_error_adapters: Vec<Box<dyn ProtocolAdapter>> = vec![
        create_test_adapter_with_error(
            "error_adapter1",
            false,
            Value::Null,
            Some("Error: method not found".to_string()),
        ),
        create_test_adapter_with_error(
            "error_adapter2",
            false,
            Value::Null,
            Some("Error: invalid parameters".to_string()),
        ),
    ];
    let different_error_analyzer = DifferentialAnalyzer::new(different_error_adapters);
    let different_error_result = different_error_analyzer.run_fuzz_case(&fuzz_case);
    assert_eq!(different_error_result.adapter_results.len(), 2);
    assert!(!different_error_result.equivalent);
    assert!(!different_error_result.differences.is_empty());
    assert!(
        different_error_result.summary.contains("differences")
            || different_error_result.summary.contains("semantic")
    );

    let mixed_adapters: Vec<Box<dyn ProtocolAdapter>> = vec![
        create_test_adapter("success_adapter", true, json!({"result": "success"})),
        create_test_adapter("error_adapter", false, Value::Null),
    ];
    let mixed_analyzer = DifferentialAnalyzer::new(mixed_adapters);
    let mixed_result = mixed_analyzer.run_fuzz_case(&fuzz_case);
    assert_eq!(mixed_result.adapter_results.len(), 2);
    assert!(!mixed_result.equivalent);
    assert!(!mixed_result.differences.is_empty());
    assert!(mixed_result.summary.contains("differences"));
    assert!(mixed_result.differences.iter().any(|diff| diff.field_path == "success"));

    // Test both succeed but return different responses (should show field differences)
    let different_response_adapters: Vec<Box<dyn ProtocolAdapter>> = vec![
        create_test_adapter("adapter1", true, json!({"result": "success", "data": "value1"})),
        create_test_adapter("adapter2", true, json!({"result": "success", "data": "value2"})),
    ];
    let different_response_analyzer = DifferentialAnalyzer::new(different_response_adapters);
    let different_response_result = different_response_analyzer.run_fuzz_case(&fuzz_case);
    assert_eq!(different_response_result.adapter_results.len(), 2);
    assert!(!different_response_result.equivalent);
    assert!(!different_response_result.differences.is_empty());
    assert!(different_response_result.summary.contains("differences"));
    assert!(different_response_result.differences.iter().any(|diff| diff.field_path == "data"));
}

#[test]
fn test_adapter_count() {
    let empty_adapters: Vec<Box<dyn ProtocolAdapter>> = vec![];
    let empty_analyzer = DifferentialAnalyzer::new(empty_adapters);
    assert_eq!(empty_analyzer.adapter_count(), 0);

    let single_adapters: Vec<Box<dyn ProtocolAdapter>> =
        vec![create_test_adapter("single_adapter", true, Value::Null)];
    let single_analyzer = DifferentialAnalyzer::new(single_adapters);
    assert_eq!(single_analyzer.adapter_count(), 1);

    let adapters: Vec<Box<dyn ProtocolAdapter>> = vec![
        create_test_adapter("adapter1", true, Value::Null),
        create_test_adapter("adapter2", true, Value::Null),
    ];
    let analyzer = DifferentialAnalyzer::new(adapters);
    assert_eq!(analyzer.adapter_count(), 2);
}
