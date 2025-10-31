use std::result::Result;
use std::sync::Arc;
use std::fmt;
use crate::transport::core::{TransportTrait, TransportError};
use crate::transport::DefaultTransport;

/// Thin wrapper around a transport for making RPC calls
pub struct RpcClient {
    transport: Arc<dyn TransportTrait>,
}

impl fmt::Debug for RpcClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RpcClient")
            .field("transport", &"<dyn TransportTrait>")
            .finish()
    }
}

impl RpcClient {
    /// Wrap an existing transport (no URL+auth dance)
    pub fn from_transport(inner: Arc<dyn TransportTrait>) -> Self {
        Self { transport: inner }
    }

    /// Create a new RPC client with socket path
    pub fn new(socket_path: &str) -> Self {
        let transport = {{TRANSPORT_CONSTRUCTOR}};
        Self { transport: Arc::new(transport) }
    }

    /// Call a JSON-RPC method
    pub async fn call_method(&self, method: &str, params: &[serde_json::Value]) -> Result<serde_json::Value, TransportError> {
        self.transport.send_request(method, params).await
    }
}
