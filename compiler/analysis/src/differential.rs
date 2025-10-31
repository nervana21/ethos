//! Differential Analysis
//!
//! Executes identical fuzz inputs through multiple protocol adapters
//! and analyzes their outputs to detect semantic divergences. This provides the
//! architectural foundation for plugging in different protocol implementations
//! (Bitcoin Core, Core Lightning, LND, Rust-Lightning, etc.) without changing the core logic.

use std::sync::Arc;

use fuzz_types::{FuzzCase, FuzzResult, ProtocolAdapter};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Result of comparing outputs from multiple adapters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DifferentialResult {
    /// The fuzz case that was tested
    pub fuzz_case: FuzzCase,
    /// Results from each adapter
    pub adapter_results: Vec<FuzzResult>,
    /// Whether all adapters produced equivalent results
    pub equivalent: bool,
    /// Differences found between adapters
    pub differences: Vec<Difference>,
    /// Summary of the comparison
    pub summary: String,
}

/// A specific difference found between adapter outputs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Difference {
    /// The field path where the difference was found
    pub field_path: String,
    /// The value from the first adapter
    pub value_a: Value,
    /// The value from the second adapter
    pub value_b: Value,
    /// The adapters that produced these values
    /// The first adapter that produced the value
    pub adapter_a: String,
    /// The second adapter that produced the value
    pub adapter_b: String,
}

/// Differential analyzer that compares multiple protocol implementations
pub struct DifferentialAnalyzer {
    /// The adapters to compare
    implementations: Vec<Box<dyn ProtocolAdapter>>,
    /// Shared tokio runtime for async operations
    runtime: Arc<tokio::runtime::Runtime>,
}

impl DifferentialAnalyzer {
    /// Create a new differential analyzer with the given adapters
    pub fn new(adapters: Vec<Box<dyn ProtocolAdapter>>) -> Self {
        let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

        Self { implementations: adapters, runtime: Arc::new(runtime) }
    }

    /// Run a fuzz case through all adapters and compare results
    pub fn run_fuzz_case(&self, fuzz_case: &FuzzCase) -> DifferentialResult {
        // Apply the fuzz case to all adapters in parallel
        let rt = self.runtime.handle();

        // Create futures for all adapters
        let futures: Vec<_> = self
            .implementations
            .iter()
            .map(|adapter| {
                let case = fuzz_case.clone();
                let adapter_name = adapter.name().to_string();
                async move {
                    let case_start = std::time::Instant::now();
                    match adapter.apply_fuzz_case(&case).await {
                        Ok(mut result) => {
                            result.execution_time_ms = case_start.elapsed().as_millis() as u64;
                            // Normalize error if present
                            if let Some(ref error) = result.error {
                                result.normalized_error =
                                    Some(fuzz_types::NormalizedError::from_error_string(error));
                            }
                            result
                        }
                        Err(e) => {
                            let error_str = e.to_string();
                            FuzzResult {
                                adapter_name,
                                raw_response: Value::Null,
                                success: false,
                                error: Some(error_str.clone()),
                                normalized_error: Some(
                                    fuzz_types::NormalizedError::from_error_string(&error_str),
                                ),
                                execution_time_ms: case_start.elapsed().as_millis() as u64,
                            }
                        }
                    }
                }
            })
            .collect();

        // Wait for all futures to complete
        let adapter_results = rt.block_on(async { futures::future::join_all(futures).await });

        // Compare the results
        let differences = self.compare_outputs(&adapter_results);
        let equivalent = differences.is_empty();

        let summary = if equivalent {
            format!("All {} adapters produced equivalent results", adapter_results.len())
        } else {
            format!("Found {} semantic differences between adapters", differences.len())
        };

        DifferentialResult {
            fuzz_case: fuzz_case.clone(),
            adapter_results,
            equivalent,
            differences,
            summary,
        }
    }

    /// Get the number of adapters configured
    pub fn adapter_count(&self) -> usize { self.implementations.len() }

    /// Compare outputs from multiple adapters and return differences
    fn compare_outputs(&self, results: &[FuzzResult]) -> Vec<Difference> {
        let mut all_differences = Vec::new();

        if results.len() < 2 {
            return all_differences;
        }

        // Compare each pair of results
        for i in 0..results.len() {
            for j in (i + 1)..results.len() {
                let result_a = &results[i];
                let result_b = &results[j];

                // Check if both succeeded or both failed
                if result_a.success != result_b.success {
                    all_differences.push(Difference {
                        field_path: "success".to_string(),
                        value_a: Value::Bool(result_a.success),
                        value_b: Value::Bool(result_b.success),
                        adapter_a: result_a.adapter_name.clone(),
                        adapter_b: result_b.adapter_name.clone(),
                    });
                    continue;
                }

                // If both failed, compare error messages
                if !result_a.success && !result_b.success {
                    // First try normalized comparison
                    if let (Some(norm_a), Some(norm_b)) =
                        (&result_a.normalized_error, &result_b.normalized_error)
                    {
                        // Skip if semantically equivalent
                        if norm_a.is_equivalent(norm_b) {
                            continue; // No difference to report
                        }
                    }

                    // Report difference only if not equivalent
                    if result_a.error != result_b.error {
                        all_differences.push(Difference {
                            field_path: "error".to_string(),
                            value_a: result_a
                                .error
                                .as_ref()
                                .map(|e| Value::String(e.clone()))
                                .unwrap_or(Value::Null),
                            value_b: result_b
                                .error
                                .as_ref()
                                .map(|e| Value::String(e.clone()))
                                .unwrap_or(Value::Null),
                            adapter_a: result_a.adapter_name.clone(),
                            adapter_b: result_b.adapter_name.clone(),
                        });
                    }
                    continue;
                }

                // If both succeeded, compare the normalized responses
                if result_a.success && result_b.success {
                    let normalized_a =
                        self.implementations[i].normalize_output(&result_a.raw_response);
                    let normalized_b =
                        self.implementations[j].normalize_output(&result_b.raw_response);

                    let field_differences = self.compare_values(&normalized_a, &normalized_b, "");
                    for mut diff in field_differences {
                        diff.adapter_a = result_a.adapter_name.clone();
                        diff.adapter_b = result_b.adapter_name.clone();
                        all_differences.push(diff);
                    }
                }
            }
        }

        // Filter out cosmetic differences completely
        all_differences
            .into_iter()
            .filter(|diff| !self.is_cosmetic_field(&diff.field_path))
            .collect()
    }

    /// Recursively compare two JSON values and return differences
    #[allow(clippy::only_used_in_recursion)]
    fn compare_values(&self, a: &Value, b: &Value, path: &str) -> Vec<Difference> {
        let mut differences = Vec::new();

        match (a, b) {
            (Value::Object(map_a), Value::Object(map_b)) => {
                // Compare all keys in both objects
                let all_keys: std::collections::HashSet<&String> =
                    map_a.keys().chain(map_b.keys()).collect();

                for key in all_keys {
                    let new_path =
                        if path.is_empty() { key.clone() } else { format!("{}.{}", path, key) };

                    match (map_a.get(key), map_b.get(key)) {
                        (Some(val_a), Some(val_b)) => {
                            differences.extend(self.compare_values(val_a, val_b, &new_path));
                        }
                        (Some(val_a), None) => {
                            differences.push(Difference {
                                field_path: new_path.clone(),
                                value_a: val_a.clone(),
                                value_b: Value::Null,
                                adapter_a: "unknown".to_string(),
                                adapter_b: "unknown".to_string(),
                            });
                        }
                        (None, Some(val_b)) => {
                            differences.push(Difference {
                                field_path: new_path.clone(),
                                value_a: Value::Null,
                                value_b: val_b.clone(),
                                adapter_a: "unknown".to_string(),
                                adapter_b: "unknown".to_string(),
                            });
                        }
                        (None, None) => {} // Both missing, no difference
                    }
                }
            }
            (Value::Array(arr_a), Value::Array(arr_b)) =>
                if arr_a.len() != arr_b.len() {
                    differences.push(Difference {
                        field_path: path.to_string(),
                        value_a: Value::Number(arr_a.len().into()),
                        value_b: Value::Number(arr_b.len().into()),
                        adapter_a: "unknown".to_string(),
                        adapter_b: "unknown".to_string(),
                    });
                } else {
                    for (i, (val_a, val_b)) in arr_a.iter().zip(arr_b.iter()).enumerate() {
                        let new_path = format!("{}[{}]", path, i);
                        differences.extend(self.compare_values(val_a, val_b, &new_path));
                    }
                },
            (val_a, val_b) =>
                if val_a != val_b {
                    differences.push(Difference {
                        field_path: path.to_string(),
                        value_a: val_a.clone(),
                        value_b: val_b.clone(),
                        adapter_a: "unknown".to_string(),
                        adapter_b: "unknown".to_string(),
                    });
                },
        }

        differences
    }

    /// Check if a field path represents a cosmetic difference
    fn is_cosmetic_field(&self, path: &str) -> bool {
        // Fields that are typically cosmetic differences
        let cosmetic_fields = ["timestamp", "id", "signature", "nonce", "created_at", "updated_at"];
        cosmetic_fields.iter().any(|field| path.contains(field))
    }
}

/// Errors that can occur during differential fuzzing
#[derive(Debug, Error)]
pub enum DifferentialError {
    /// Error during fuzz case execution
    #[error("Fuzz case execution failed: {0}")]
    ExecutionFailed(String),

    /// Error during result comparison
    #[error("Result comparison failed: {0}")]
    ComparisonFailed(String),

    /// Error during normalization
    #[error("Output normalization failed: {0}")]
    NormalizationFailed(String),
}
