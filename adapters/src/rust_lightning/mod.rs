//! Rust-Lightning RPC Adapter
//!
//! This adapter provides integration with rust-lightning for differential fuzzing.
//! It implements the LightningAdapter trait to enable comparison with Core Lightning.

use std::time::Duration;

use fuzz_types::ProtocolAdapter as FuzzProtocolAdapter;
use ir::ProtocolIR;
use serde_json::Value;
use thiserror::Error;

use crate::normalization_registry::{AdapterKind, NormalizationRegistry};
use crate::rpc_adapter::ProtocolBackend;
use crate::{
    FuzzCase, FuzzResult, LightningProtocolAdapter, ProtocolAdapterError, ProtocolAdapterResult,
    CAP_RPC,
};

/// Errors that can occur during Rust-Lightning operations
#[derive(Debug, Error)]
pub enum RustLightningError {
    /// Rust-Lightning is not available in the current environment
    #[error("Rust-Lightning not available: {0}")]
    NotAvailable(String),

    /// RPC call to Rust-Lightning failed
    #[error("RPC call failed: {0}")]
    RpcError(String),

    /// Error during serialization/deserialization
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// HTTP request error
    #[error("HTTP request failed: {0}")]
    HttpError(String),

    /// Timeout error
    #[error("Request timeout")]
    Timeout,
}

/// Rust-Lightning RPC adapter for differential fuzzing
pub struct RustLightningAdapter {
    /// Normalization registry for output processing
    normalization_registry: NormalizationRegistry,
    /// Whether the adapter is available
    available: bool,
    /// HTTP client for making requests to the harness
    http_client: Option<reqwest::Client>,
    /// Base URL for the rust-lightning harness
    base_url: String,
}

impl Default for RustLightningAdapter {
    fn default() -> Self { Self::new() }
}

impl RustLightningAdapter {
    /// Create a new Rust-Lightning adapter
    pub fn new() -> Self {
        let base_url = std::env::var("RUST_LIGHTNING_URL")
            .unwrap_or_else(|_| "http://localhost:9836".to_string());

        let http_client = if Self::check_availability() {
            Some(
                reqwest::Client::builder()
                    .timeout(Duration::from_secs(30))
                    .build()
                    .unwrap_or_else(|_| panic!("Failed to create HTTP client")),
            )
        } else {
            None
        };

        Self {
            normalization_registry: NormalizationRegistry::for_adapter(AdapterKind::RustLightning)
                .unwrap_or_default(),
            available: Self::check_availability(),
            http_client,
            base_url,
        }
    }

    /// Check if Rust-Lightning is available
    fn check_availability() -> bool {
        // Check if the harness URL is available
        std::env::var("RUST_LIGHTNING_URL").is_ok()
            || std::env::var("RUST_LIGHTNING_AVAILABLE").map(|v| v == "true").unwrap_or(false)
    }

    /// Make an RPC call to Rust-Lightning
    async fn make_rpc_call(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, RustLightningError> {
        if !self.available {
            return Err(RustLightningError::NotAvailable(
                "Rust-Lightning is not available in this environment".to_string(),
            ));
        }

        // Use HTTP client to call the harness
        if let Some(ref client) = self.http_client {
            self.make_http_rpc_call(client, method, params).await
        } else {
            // No HTTP client available - return error
            Err(RustLightningError::NotAvailable(
                "Rust-Lightning harness not available".to_string(),
            ))
        }
    }

    /// Make an HTTP RPC call to the rust-lightning harness
    async fn make_http_rpc_call(
        &self,
        client: &reqwest::Client,
        method: &str,
        params: Value,
    ) -> Result<Value, RustLightningError> {
        let request_body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params
        });

        let url = format!("{}/rpc", self.base_url);

        let response = client
            .post(&url)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| RustLightningError::HttpError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(RustLightningError::HttpError(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        let response_json: Value = response
            .json()
            .await
            .map_err(|e| RustLightningError::SerializationError(e.to_string()))?;

        // Check for RPC error in response
        if let Some(error) = response_json.get("error") {
            return Err(RustLightningError::RpcError(format!("RPC error: {}", error)));
        }

        // Extract result from response
        response_json
            .get("result")
            .cloned()
            .ok_or_else(|| RustLightningError::RpcError("No result in response".to_string()))
    }
}

#[async_trait::async_trait]
impl FuzzProtocolAdapter for RustLightningAdapter {
    fn name(&self) -> &'static str { "rust_lightning" }

    async fn apply_fuzz_case(
        &self,
        case: &FuzzCase,
    ) -> Result<FuzzResult, Box<dyn std::error::Error + Send + Sync>> {
        let start_time = std::time::Instant::now();

        if !self.available {
            return Ok(FuzzResult {
                adapter_name: <Self as FuzzProtocolAdapter>::name(self).to_string(),
                raw_response: Value::Null,
                success: false,
                error: Some("Rust-Lightning not available".to_string()),
                normalized_error: None,
                execution_time_ms: start_time.elapsed().as_millis() as u64,
            });
        }

        // Convert parameters to the format expected by Rust-Lightning
        let params = serde_json::to_value(&case.parameters)
            .map_err(|e| RustLightningError::SerializationError(e.to_string()))?;

        // Make the async RPC call directly
        let result = self.make_rpc_call(&case.method_name, params).await;

        let execution_time = start_time.elapsed().as_millis() as u64;

        match result {
            Ok(response) => Ok(FuzzResult {
                adapter_name: <Self as FuzzProtocolAdapter>::name(self).to_string(),
                raw_response: response,
                success: true,
                error: None,
                normalized_error: None,
                execution_time_ms: execution_time,
            }),
            Err(e) => Ok(FuzzResult {
                adapter_name: <Self as FuzzProtocolAdapter>::name(self).to_string(),
                raw_response: Value::Null,
                success: false,
                error: Some(e.to_string()),
                normalized_error: None,
                execution_time_ms: execution_time,
            }),
        }
    }

    fn normalize_output(&self, value: &Value) -> Value {
        // Use unified normalization registry
        let (normalized, _metadata) = self.normalization_registry.normalize_value(value);
        normalized
    }
}

// Schema ProtocolAdapter implementation
impl crate::protocol_adapter::ProtocolAdapter for RustLightningAdapter {
    fn name(&self) -> &'static str { "rust_lightning" }

    fn version(&self) -> String { "0.0.1".to_string() }

    fn extract_protocol_ir(
        &self,
        _path: &std::path::Path,
    ) -> ProtocolAdapterResult<ir::ProtocolIR> {
        // Rust-Lightning doesn't have a schema file like Core Lightning
        // Instead, we would need to generate the IR from the rust-lightning API
        Err(ProtocolAdapterError::Message(
            "Rust-Lightning IR extraction not implemented".to_string(),
        ))
    }

    fn capabilities(&self) -> Vec<&'static str> { vec![CAP_RPC] }
}

// LightningProtocolAdapter implementation that delegates to FuzzProtocolAdapter
impl LightningProtocolAdapter for RustLightningAdapter {}

// Implement LightningAdapter for RustLightningAdapter
impl crate::LightningAdapter for RustLightningAdapter {}

#[async_trait::async_trait]
impl ProtocolBackend for RustLightningAdapter {
    fn name(&self) -> &'static str { "rust_lightning" }
    fn version(&self) -> String { "dynamic".to_string() }
    fn capabilities(&self) -> Vec<&'static str> { vec![crate::CAP_RPC] }
    fn normalize_output(&self, value: &serde_json::Value) -> serde_json::Value {
        let (normalized, _) = self.normalization_registry.normalize_value(value);
        normalized
    }
    fn extract_protocol_ir(
        &self,
        _path: &std::path::Path,
    ) -> crate::ProtocolAdapterResult<ProtocolIR> {
        Err(crate::ProtocolAdapterError::Message(
            "Rust-Lightning IR extraction not implemented".to_string(),
        ))
    }
    async fn call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let v = self.make_rpc_call(method, params).await.map_err(|e| {
            Box::<dyn std::error::Error + Send + Sync>::from(std::io::Error::other(e.to_string()))
        })?;
        Ok(v)
    }
}

impl crate::rpc_adapter::BackendProvider for RustLightningAdapter {
    fn implementation() -> types::Implementation { types::Implementation::RustLightning }

    fn build(
    ) -> crate::ProtocolAdapterResult<Box<dyn crate::rpc_adapter::ProtocolBackend + Send + Sync>>
    {
        Ok(Box::new(Self::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_lightning_adapter_creation() {
        let adapter = RustLightningAdapter::new();
        assert_eq!(<RustLightningAdapter as FuzzProtocolAdapter>::name(&adapter), "rust_lightning");
    }

    #[test]
    fn test_simulate_rpc_call() {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async {
            let adapter = RustLightningAdapter::new();
            let params = serde_json::Value::Object(serde_json::Map::new());

            let result = adapter.make_rpc_call("getinfo", params).await;

            // The result should be Ok if the adapter is available, or Err if not available
            // In test environment, the adapter is typically not available
            if adapter.available {
                assert!(result.is_ok());
                if let Ok(response) = result {
                    assert!(response.get("id").is_some());
                    assert!(response.get("version").is_some());
                }
            } else {
                // If adapter is not available, we expect an error
                assert!(result.is_err());
            }
        });
    }

    #[test]
    fn test_unknown_method() {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async {
            let adapter = RustLightningAdapter::new();
            let params = serde_json::Value::Object(serde_json::Map::new());

            let result = adapter.make_rpc_call("unknown_method", params).await;
            assert!(result.is_err());
        });
    }
}
