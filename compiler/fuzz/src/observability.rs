//! Observability and Monitoring for Differential Fuzzing
//!
//! This module provides structured logging, metrics collection, and monitoring
//! capabilities for the differential fuzzing system.

use std::collections::HashMap;
use std::time::Duration;
use serde_json::{json, Value};
use thiserror::Error;

use ethos_analysis::differential::DifferentialResult;
use fuzz_types::FuzzResult;

/// Errors that can occur during observability operations
#[derive(Debug, Error)]
pub enum ObservabilityError {
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Metrics collection error: {0}")]
    MetricsError(String),
}

/// Metrics collector for differential fuzzing
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FuzzingMetrics {
    /// Total number of test cases executed
    pub total_cases: u64,
    /// Number of equivalent results
    pub equivalent_cases: u64,
    /// Number of divergent results
    pub divergent_cases: u64,
    /// Number of semantic differences found
    pub total_differences: u64,
    /// Total execution time
    pub total_execution_time: Duration,
    /// Average execution time per case
    pub average_execution_time: Duration,
    /// Adapter-specific metrics
    pub adapter_metrics: HashMap<String, AdapterMetrics>,
    /// Corpus statistics
    pub corpus_stats: CorpusMetrics,
}

/// Metrics for individual adapters
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AdapterMetrics {
    /// Adapter name
    pub name: String,
    /// Number of successful calls
    pub successful_calls: u64,
    /// Number of failed calls
    pub failed_calls: u64,
    /// Average response time
    pub average_response_time: Duration,
    /// Total response time
    pub total_response_time: Duration,
    /// Error rate percentage
    pub error_rate: f64,
}

/// Corpus-related metrics
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CorpusMetrics {
    /// Number of stable corpus entries
    pub stable_entries: u64,
    /// Number of divergence entries
    pub divergence_entries: u64,
    /// Number of crash entries
    pub crash_entries: u64,
    /// Corpus growth rate (entries per hour)
    pub growth_rate: f64,
}

impl Default for FuzzingMetrics {
    fn default() -> Self {
        Self {
            total_cases: 0,
            equivalent_cases: 0,
            divergent_cases: 0,
            total_differences: 0,
            total_execution_time: Duration::ZERO,
            average_execution_time: Duration::ZERO,
            adapter_metrics: HashMap::new(),
            corpus_stats: CorpusMetrics {
                stable_entries: 0,
                divergence_entries: 0,
                crash_entries: 0,
                growth_rate: 0.0,
            },
        }
    }
}

impl FuzzingMetrics {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a differential result
    pub fn record_result(&mut self, result: &DifferentialResult, execution_time: Duration) {
        self.total_cases += 1;
        self.total_execution_time += execution_time;
        self.average_execution_time = Duration::from_millis(
            self.total_execution_time.as_millis() as u64 / self.total_cases
        );

        if result.equivalent {
            self.equivalent_cases += 1;
        } else {
            self.divergent_cases += 1;
            self.total_differences += result.differences.len() as u64;
        }

        // Update adapter metrics
        for adapter_result in &result.adapter_results {
            self.update_adapter_metrics(adapter_result);
        }
    }

    /// Update metrics for a specific adapter
    fn update_adapter_metrics(&mut self, result: &FuzzResult) {
        let adapter_name = result.adapter_name.clone();
        let metrics = self.adapter_metrics.entry(adapter_name.clone()).or_insert_with(|| {
            AdapterMetrics {
                name: adapter_name,
                successful_calls: 0,
                failed_calls: 0,
                average_response_time: Duration::ZERO,
                total_response_time: Duration::ZERO,
                error_rate: 0.0,
            }
        });

        if result.success {
            metrics.successful_calls += 1;
        } else {
            metrics.failed_calls += 1;
        }

        metrics.total_response_time += Duration::from_millis(result.execution_time_ms);
        metrics.average_response_time = Duration::from_millis(
            metrics.total_response_time.as_millis() as u64 /
            (metrics.successful_calls + metrics.failed_calls)
        );

        let total_calls = metrics.successful_calls + metrics.failed_calls;
        metrics.error_rate = if total_calls > 0 {
            (metrics.failed_calls as f64 / total_calls as f64) * 100.0
        } else {
            0.0
        };
    }

    /// Update corpus metrics
    pub fn update_corpus_metrics(&mut self, stable: u64, divergences: u64, crashes: u64) {
        self.corpus_stats.stable_entries = stable;
        self.corpus_stats.divergence_entries = divergences;
        self.corpus_stats.crash_entries = crashes;

        // Calculate growth rate (simplified)
        let total_entries = stable + divergences + crashes;
        self.corpus_stats.growth_rate = total_entries as f64 /
            (self.total_cases as f64 / 3600.0).max(1.0); // entries per hour
    }

    /// Get a summary of current metrics
    pub fn get_summary(&self) -> Value {
        json!({
            "total_cases": self.total_cases,
            "equivalent_cases": self.equivalent_cases,
            "divergent_cases": self.divergent_cases,
            "total_differences": self.total_differences,
            "total_execution_time_ms": self.total_execution_time.as_millis(),
            "average_execution_time_ms": self.average_execution_time.as_millis(),
            "adapter_metrics": self.adapter_metrics,
            "corpus_stats": {
                "stable_entries": self.corpus_stats.stable_entries,
                "divergence_entries": self.corpus_stats.divergence_entries,
                "crash_entries": self.corpus_stats.crash_entries,
                "growth_rate": self.corpus_stats.growth_rate
            }
        })
    }

    /// Print a human-readable summary
    pub fn print_summary(&self) {
        println!("=== Differential Fuzzing Metrics ===");
        println!("Total cases: {}", self.total_cases);
        println!("Equivalent cases: {} ({:.1}%)",
            self.equivalent_cases,
            if self.total_cases > 0 {
                (self.equivalent_cases as f64 / self.total_cases as f64) * 100.0
            } else { 0.0 }
        );
        println!("Divergent cases: {} ({:.1}%)",
            self.divergent_cases,
            if self.total_cases > 0 {
                (self.divergent_cases as f64 / self.total_cases as f64) * 100.0
            } else { 0.0 }
        );
        println!("Total differences: {}", self.total_differences);
        println!("Average execution time: {:.2}ms",
            self.average_execution_time.as_millis() as f64 / 1000.0
        );
        println!("=====================================");
    }
}

/// Structured logger for differential fuzzing
pub struct StructuredLogger {
    /// Log level
    level: LogLevel,
    /// Whether to output JSON logs
    json_output: bool,
}

/// Log levels for structured logging
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl StructuredLogger {
    /// Create a new structured logger
    pub fn new(level: LogLevel, json_output: bool) -> Self {
        Self { level, json_output }
    }

    /// Create logger from environment variables
    pub fn from_env() -> Self {
        let level = std::env::var("RUST_LOG")
            .unwrap_or_else(|_| "info".to_string())
            .parse()
            .unwrap_or(LogLevel::Info);

        let json_output = std::env::var("JSON_LOGS")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .unwrap_or(false);

        Self::new(level, json_output)
    }

    /// Log a differential result
    pub fn log_result(&self, result: &DifferentialResult, execution_time: Duration) {
        if self.level <= LogLevel::Info {
            let log_entry = json!({
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "level": "info",
                "event": "differential_result",
                "method_name": result.fuzz_case.method_name,
                "equivalent": result.equivalent,
                "differences_count": result.differences.len(),
                "execution_time_ms": execution_time.as_millis(),
                "adapter_results": result.adapter_results.iter().map(|r| json!({
                    "adapter": r.adapter_name,
                    "success": r.success,
                    "execution_time_ms": r.execution_time_ms,
                    "error": r.error
                })).collect::<Vec<_>>()
            });

            if self.json_output {
                println!("{}", serde_json::to_string(&log_entry).unwrap());
            } else {
                println!("[{}] {} - {} differences, {}ms",
                    chrono::Utc::now().format("%H:%M:%S"),
                    result.fuzz_case.method_name,
                    result.differences.len(),
                    execution_time.as_millis()
                );
            }
        }
    }

    /// Log a difference found between adapters
    pub fn log_difference(&self, difference: &ethos_analysis::differential::Difference) {
        if self.level <= LogLevel::Warn {
            let log_entry = json!({
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "level": "warn",
                "event": "difference_found",
                "field_path": difference.field_path,
                "adapter_a": difference.adapter_a,
                "adapter_b": difference.adapter_b,
                "value_a": difference.value_a,
                "value_b": difference.value_b,
            });

            if self.json_output {
                println!("{}", serde_json::to_string(&log_entry).unwrap());
            } else {
                println!("[{}] WARN: Difference in {}: {} vs {}",
                    chrono::Utc::now().format("%H:%M:%S"),
                    difference.field_path,
                    difference.value_a,
                    difference.value_b
                );
            }
        }
    }

    /// Log metrics summary
    pub fn log_metrics(&self, metrics: &FuzzingMetrics) {
        if self.level <= LogLevel::Info {
            let log_entry = json!({
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "level": "info",
                "event": "metrics_summary",
                "metrics": metrics.get_summary()
            });

            if self.json_output {
                println!("{}", serde_json::to_string(&log_entry).unwrap());
            } else {
                metrics.print_summary();
            }
        }
    }
}

impl std::str::FromStr for LogLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "debug" => Ok(LogLevel::Debug),
            "info" => Ok(LogLevel::Info),
            "warn" => Ok(LogLevel::Warn),
            "error" => Ok(LogLevel::Error),
            _ => Err(format!("Invalid log level: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use serde_json::Value;

    #[test]
    fn test_metrics_collection() {
        let mut metrics = FuzzingMetrics::new();
        assert_eq!(metrics.total_cases, 0);

        // Simulate recording a result
        let fuzz_case = crate::differential_pass::FuzzCase {
            method_name: "test".to_string(),
            parameters: HashMap::new(),
            expected_result_type: None,
        };

        let result = crate::differential_pass::DifferentialResult {
            fuzz_case: fuzz_case.clone(),
            adapter_results: vec![],
            equivalent: true,
            differences: vec![],
            summary: "Test".to_string(),
        };

        metrics.record_result(&result, Duration::from_millis(100));
        assert_eq!(metrics.total_cases, 1);
        assert_eq!(metrics.equivalent_cases, 1);
    }

    #[test]
    fn test_structured_logger() {
        let logger = StructuredLogger::new(LogLevel::Info, false);
        // Test that logger can be created without panicking
        assert_eq!(logger.level, LogLevel::Info);
        assert!(!logger.json_output);
    }
}
