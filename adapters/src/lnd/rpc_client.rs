//! LND RPC Client
//!
//! This module provides a client for making RPC calls to a running LND node.
//! It handles connection management, timeouts, retries, and response parsing.

use std::time::Duration;

use ir::ProtocolIR;
use serde_json::{json, Value};
use thiserror::Error;

use crate::rpc_adapter::ProtocolBackend;
use crate::ProtocolAdapterResult;

#[async_trait::async_trait]
impl ProtocolBackend for LndRpcClient {
    fn name(&self) -> &'static str { "lnd" }
    fn version(&self) -> String { "dynamic".to_string() }
    fn capabilities(&self) -> Vec<&'static str> { vec![crate::CAP_RPC] }
    fn normalize_output(&self, value: &serde_json::Value) -> serde_json::Value {
        let registry = crate::normalization_registry::NormalizationRegistry::for_adapter(
            crate::normalization_registry::AdapterKind::Lnd,
        )
        .unwrap_or_default();
        let (normalized, _) = registry.normalize_value(value);
        normalized
    }
    fn extract_protocol_ir(&self, path: &std::path::Path) -> ProtocolAdapterResult<ProtocolIR> {
        ProtocolIR::from_file(path).map_err(|e| crate::ProtocolAdapterError::Message(e.to_string()))
    }
    async fn call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let v = LndRpcClient::call_http(self, method, params).await?;
        Ok(v)
    }
}

impl crate::rpc_adapter::BackendProvider for LndRpcClient {
    fn implementation() -> types::Implementation { types::Implementation::Lnd }

    fn build(
    ) -> crate::ProtocolAdapterResult<Box<dyn crate::rpc_adapter::ProtocolBackend + Send + Sync>>
    {
        let client =
            Self::from_env().map_err(|e| crate::ProtocolAdapterError::Message(e.to_string()))?;
        Ok(Box::new(client))
    }
}

/// Errors that can occur during RPC communication
#[derive(Debug, Error)]
pub enum LndRpcError {
    /// HTTP request failed
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    /// JSON serialization/deserialization failed
    #[error("JSON serialization failed: {0}")]
    JsonError(#[from] serde_json::Error),

    /// RPC call returned an error
    #[error("RPC call failed: {0}")]
    RpcError(String),

    /// Request timed out
    #[error("Timeout after {0}ms")]
    Timeout(u64),

    /// Connection failed after retries
    #[error("Connection failed after {0} retries")]
    ConnectionFailed(u32),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Macaroon authentication error
    #[error("Macaroon authentication failed: {0}")]
    MacaroonError(String),

    /// TLS certificate error
    #[error("TLS certificate error: {0}")]
    TlsError(String),
}

/// Configuration for the LND RPC client
#[derive(Debug, Clone)]
pub struct LndRpcConfig {
    /// Base URL for the LND REST API
    pub base_url: String,
    /// Macaroon for authentication
    pub macaroon: String,
    /// Path to TLS certificate (optional)
    pub cert_path: Option<String>,
    /// Timeout for individual requests
    pub timeout_ms: u64,
    /// Maximum number of retries for failed requests
    pub max_retries: u32,
    /// Delay between retries in milliseconds
    pub retry_delay_ms: u64,
}

impl Default for LndRpcConfig {
    fn default() -> Self {
        Self {
            base_url: "https://localhost:8080".to_string(),
            macaroon: String::new(),
            cert_path: None,
            timeout_ms: 30000,
            max_retries: 3,
            retry_delay_ms: 1000,
        }
    }
}

impl LndRpcConfig {
    /// Create config from environment variables
    pub fn from_env() -> Self {
        fn parse_env<T: std::str::FromStr>(key: &str, default: T) -> T {
            std::env::var(key).ok().and_then(|s| s.parse().ok()).unwrap_or(default)
        }

        let mut cfg = Self::default();
        if let Ok(url) = std::env::var("LND_RPC_URL") {
            cfg.base_url = url;
        }
        if let Ok(mac) = std::env::var("LND_MACAROON") {
            cfg.macaroon = mac;
        }
        cfg.cert_path = std::env::var("LND_CERT_PATH").ok();
        cfg.timeout_ms = parse_env("LND_RPC_TIMEOUT_MS", cfg.timeout_ms);
        cfg.max_retries = parse_env("LND_RPC_MAX_RETRIES", cfg.max_retries);
        cfg.retry_delay_ms = parse_env("LND_RPC_RETRY_DELAY_MS", cfg.retry_delay_ms);
        cfg
    }
}

/// LND RPC client
#[derive(Clone)]
pub struct LndRpcClient {
    client: reqwest::Client,
    config: LndRpcConfig,
}

impl LndRpcClient {
    /// Create a new RPC client with the given configuration
    pub fn new(config: LndRpcConfig) -> Result<Self, LndRpcError> {
        // Validate configuration
        if config.base_url.is_empty() {
            return Err(LndRpcError::InvalidConfig("Base URL cannot be empty".to_string()));
        }

        if config.macaroon.is_empty() {
            return Err(LndRpcError::InvalidConfig("Macaroon cannot be empty".to_string()));
        }

        // Build HTTP client with TLS configuration
        let mut client_builder =
            reqwest::Client::builder().timeout(Duration::from_millis(config.timeout_ms));

        // Configure TLS if certificate path is provided
        if let Some(ref _cert_path) = config.cert_path {
            // For now, we'll use dangerous_accept_invalid_certs for development
            // In production, you'd want to properly validate the certificate
            client_builder = client_builder.danger_accept_invalid_certs(true);
        }

        let client = client_builder.build()?;

        Ok(Self { client, config })
    }

    /// Create a new RPC client with configuration from environment variables
    pub fn from_env() -> Result<Self, LndRpcError> { Self::new(LndRpcConfig::from_env()) }

    /// Make an HTTP RPC call to LND
    pub async fn call_http(&self, method: &str, params: Value) -> Result<Value, LndRpcError> {
        let url = format!("{}/v1/{}", self.config.base_url, method);

        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            let result = self.try_call_http(&url, &params).await;

            match result {
                Ok(response) => return Ok(response),
                Err(e) => {
                    last_error = Some(e);

                    if attempt < self.config.max_retries {
                        tokio::time::sleep(Duration::from_millis(self.config.retry_delay_ms)).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or(LndRpcError::ConnectionFailed(self.config.max_retries)))
    }

    /// Make a gRPC RPC call to LND (placeholder for future implementation)
    pub async fn call_grpc(&self, _method: &str, _params: Value) -> Result<Value, LndRpcError> {
        // Placeholder for gRPC implementation
        Err(LndRpcError::RpcError("gRPC calls not yet implemented".to_string()))
    }

    /// Attempt a single HTTP RPC call
    async fn try_call_http(&self, url: &str, params: &Value) -> Result<Value, LndRpcError> {
        // Create headers with macaroon authentication
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Grpc-Metadata-macaroon",
            self.config.macaroon.parse().map_err(|e| {
                LndRpcError::MacaroonError(format!("Invalid macaroon format: {}", e))
            })?,
        );

        let response = self.client.post(url).headers(headers).json(params).send().await?;

        if !response.status().is_success() {
            return Err(LndRpcError::RpcError(format!("HTTP error: {}", response.status())));
        }

        let response_json: Value = response.json().await?;

        // LND REST API returns the result directly, not wrapped in a JSON-RPC envelope
        Ok(response_json)
    }

    /// Test the connection to LND
    pub async fn test_connection(&self) -> Result<(), LndRpcError> {
        self.call_http("getinfo", json!({})).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lnd_rpc_config_from_env() {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async {
            std::env::set_var("LND_RPC_URL", "https://test:8080");
            std::env::set_var("LND_MACAROON", "test_macaroon");
            std::env::set_var("LND_CERT_PATH", "/path/to/cert");
            std::env::set_var("LND_RPC_TIMEOUT_MS", "5000");

            let config = LndRpcConfig::from_env();
            assert_eq!(config.base_url, "https://test:8080");
            assert_eq!(config.macaroon, "test_macaroon");
            assert_eq!(config.cert_path, Some("/path/to/cert".to_string()));
            assert_eq!(config.timeout_ms, 5000);

            // Clean up
            std::env::remove_var("LND_RPC_URL");
            std::env::remove_var("LND_MACAROON");
            std::env::remove_var("LND_CERT_PATH");
            std::env::remove_var("LND_RPC_TIMEOUT_MS");
        });
    }

    #[test]
    fn test_lnd_rpc_client_creation() {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async {
            let config = LndRpcConfig {
                base_url: "https://localhost:8080".to_string(),
                macaroon: "test_macaroon".to_string(),
                cert_path: None,
                timeout_ms: 5000,
                max_retries: 3,
                retry_delay_ms: 1000,
            };

            let client = LndRpcClient::new(config);
            assert!(client.is_ok());
        });
    }
}
