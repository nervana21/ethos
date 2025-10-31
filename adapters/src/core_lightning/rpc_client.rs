//! Core Lightning RPC Client
//!
//! This module provides a client for making RPC calls to a running Core Lightning node.
//! It handles connection management, timeouts, retries, and response parsing.

use std::path::PathBuf;
use std::time::Duration;

use ir::ProtocolIR;
use serde_json::{json, Value};
use thiserror::Error;

use crate::rpc_adapter::ProtocolBackend;
use crate::ProtocolAdapterResult;

#[async_trait::async_trait]
impl ProtocolBackend for CoreLightningRpcClient {
    fn name(&self) -> &'static str { "core_lightning" }
    fn version(&self) -> String { "dynamic".to_string() }
    fn capabilities(&self) -> Vec<&'static str> { vec![crate::CAP_RPC] }
    fn normalize_output(&self, value: &serde_json::Value) -> serde_json::Value {
        // Use lightning normalization by default (can inject proper registry if needed)
        let registry = crate::normalization_registry::NormalizationRegistry::for_adapter(
            crate::normalization_registry::AdapterKind::CoreLightning,
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
        let v = CoreLightningRpcClient::call(self, method, params).await?;
        Ok(v)
    }
}

impl crate::rpc_adapter::BackendProvider for CoreLightningRpcClient {
    fn implementation() -> types::Implementation { types::Implementation::CoreLightning }

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
pub enum RpcClientError {
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

    /// Unix socket communication error
    #[error("Unix socket error: {0}")]
    UnixSocketError(String),

    /// Invalid socket path configuration
    #[error("Invalid socket path: {0}")]
    InvalidSocketPath(String),
}

/// Configuration for the RPC client
#[derive(Debug, Clone)]
pub struct RpcClientConfig {
    /// Base URL for the Core Lightning RPC endpoint (HTTP mode)
    pub base_url: Option<String>,
    /// Unix socket path for Core Lightning RPC (UNIX socket mode)
    pub socket_path: Option<PathBuf>,
    /// Timeout for individual requests
    pub timeout_ms: u64,
    /// Maximum number of retries for failed requests
    pub max_retries: u32,
    /// Delay between retries in milliseconds
    pub retry_delay_ms: u64,
}

impl Default for RpcClientConfig {
    fn default() -> Self {
        Self {
            base_url: Some("http://localhost:9835".to_string()),
            socket_path: None,
            timeout_ms: 3000,
            max_retries: 3,
            retry_delay_ms: 1000,
        }
    }
}

impl RpcClientConfig {
    /// Create config from environment variables
    pub fn from_env() -> Self {
        fn parse_env<T: std::str::FromStr>(key: &str, default: T) -> T {
            std::env::var(key).ok().and_then(|s| s.parse().ok()).unwrap_or(default)
        }

        let mut cfg = Self::default();

        if let Ok(path) = std::env::var("CLN_RPC_SOCKET") {
            cfg.socket_path = Some(PathBuf::from(path));
            cfg.base_url = None;
        } else {
            // fall back to HTTP mode
            if let Ok(url) = std::env::var("CLN_RPC_URL") {
                cfg.base_url = Some(url);
            }
        }

        cfg.timeout_ms = parse_env("CLN_RPC_TIMEOUT_MS", cfg.timeout_ms);
        cfg.max_retries = parse_env("CLN_RPC_MAX_RETRIES", cfg.max_retries);
        cfg.retry_delay_ms = parse_env("CLN_RPC_RETRY_DELAY_MS", cfg.retry_delay_ms);

        cfg
    }
}

/// Core Lightning RPC client
#[derive(Clone, Debug)]
pub struct CoreLightningRpcClient {
    client: Option<reqwest::Client>,
    config: RpcClientConfig,
}

impl CoreLightningRpcClient {
    /// Create a new RPC client with the given configuration
    pub fn new(config: RpcClientConfig) -> Result<Self, RpcClientError> {
        // Validate configuration
        if config.base_url.is_none() && config.socket_path.is_none() {
            return Err(RpcClientError::InvalidSocketPath(
                "Either base_url or socket_path must be provided".to_string(),
            ));
        }

        let client = if config.base_url.is_some() {
            Some(
                reqwest::Client::builder()
                    .timeout(Duration::from_millis(config.timeout_ms))
                    .build()?,
            )
        } else {
            None
        };

        Ok(Self { client, config })
    }

    /// Create a new RPC client with configuration from environment variables
    pub fn from_env() -> Result<Self, RpcClientError> { Self::new(RpcClientConfig::from_env()) }

    /// Make an RPC call to Core Lightning
    pub async fn call(&self, method: &str, params: Value) -> Result<Value, RpcClientError> {
        let request_body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params
        });

        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            let result = if let Some(ref client) = self.client {
                // HTTP mode
                self.try_call_http(client, &request_body).await
            } else if let Some(ref socket_path) = self.config.socket_path {
                // UNIX socket mode
                self.try_call_unix_socket(socket_path, &request_body).await
            } else {
                return Err(RpcClientError::InvalidSocketPath(
                    "No client or socket path configured".to_string(),
                ));
            };

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

        Err(last_error.unwrap_or(RpcClientError::ConnectionFailed(self.config.max_retries)))
    }

    /// Attempt a single HTTP RPC call
    async fn try_call_http(
        &self,
        client: &reqwest::Client,
        request_body: &Value,
    ) -> Result<Value, RpcClientError> {
        let base_url = self.config.base_url.as_ref().ok_or_else(|| {
            RpcClientError::InvalidSocketPath("Base URL not configured".to_string())
        })?;

        let response = client.post(base_url).json(request_body).send().await?;

        if !response.status().is_success() {
            return Err(RpcClientError::RpcError(format!("HTTP error: {}", response.status())));
        }

        let response_json: Value = response.json().await?;

        // Check for RPC error in response
        if let Some(error) = response_json.get("error") {
            return Err(RpcClientError::RpcError(format!("RPC error: {}", error)));
        }

        // Extract result from response
        response_json
            .get("result")
            .cloned()
            .ok_or_else(|| RpcClientError::RpcError("No result in response".to_string()))
    }

    /// Attempt a single UNIX socket RPC call
    async fn try_call_unix_socket(
        &self,
        socket_path: &PathBuf,
        request_body: &Value,
    ) -> Result<Value, RpcClientError> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::UnixStream;

        // Connect to the UNIX socket
        let mut stream = UnixStream::connect(socket_path).await.map_err(|e| {
            RpcClientError::UnixSocketError(format!("Failed to connect to socket: {}", e))
        })?;

        // Serialize the request with newline terminator
        let mut request_bytes =
            serde_json::to_vec(request_body).map_err(RpcClientError::JsonError)?;
        request_bytes.push(b'\n'); // Add newline terminator for Core Lightning RPC

        // Send the request
        stream.write_all(&request_bytes).await.map_err(|e| {
            RpcClientError::UnixSocketError(format!("Failed to write to socket: {}", e))
        })?;

        // Read the response line by line (not read_to_end)
        let mut response = String::new();
        let mut buffer = [0; 1024];

        // Read until we get a complete JSON response
        loop {
            let n = stream.read(&mut buffer).await.map_err(|e| {
                RpcClientError::UnixSocketError(format!("Failed to read from socket: {}", e))
            })?;
            if n == 0 {
                break;
            }
            response.push_str(&String::from_utf8_lossy(&buffer[..n]));

            // Check if we have a complete JSON response
            if serde_json::from_str::<serde_json::Value>(response.trim()).is_ok() {
                break;
            }
        }

        // Parse the response
        let response_json: Value =
            serde_json::from_str(response.trim()).map_err(RpcClientError::JsonError)?;

        // Check for RPC error in response
        if let Some(error) = response_json.get("error") {
            return Err(RpcClientError::RpcError(format!("RPC error: {}", error)));
        }

        // Extract result from response
        response_json
            .get("result")
            .cloned()
            .ok_or_else(|| RpcClientError::RpcError("No result in response".to_string()))
    }

    /// Test the connection to Core Lightning
    pub async fn test_connection(&self) -> Result<(), RpcClientError> {
        self.call("getinfo", json!({})).await?;
        Ok(())
    }
}
