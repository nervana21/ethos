#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]

//! Rust-Lightning Harness
//!
//! A small HTTP server that provides a bridge between the ethos fuzzing system
//! and rust-lightning. This allows the differential fuzzing system to test
//! rust-lightning implementations alongside Core Lightning.

use std::net::SocketAddr;
use std::sync::Arc;

use serde_json::{json, Value};
use thiserror::Error;
use warp::Filter;

/// Errors that can occur in the harness
#[derive(Debug, Error)]
pub enum HarnessError {
    #[error("RPC call failed: {0}")]
    RpcError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Rust-Lightning not available: {0}")]
    NotAvailable(String),
}

/// The main harness server
pub struct RustLightningHarness {
    /// Whether rust-lightning is available
    available: bool,
}

impl RustLightningHarness {
    /// Create a new harness instance
    pub fn new() -> Self {
        Self {
            available: true, // For now, always available
        }
    }

    /// Handle an RPC call
    pub async fn handle_rpc_call(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, HarnessError> {
        if !self.available {
            return Err(HarnessError::NotAvailable(
                "Rust-Lightning is not available in this environment".to_string(),
            ));
        }

        // Handle specific Lightning RPC methods
        match method {
            "getinfo" => self.handle_getinfo().await,
            "listpeers" => self.handle_listpeers().await,
            "listchannels" => self.handle_listchannels().await,
            "invoice" => self.handle_invoice(params).await,
            "pay" => self.handle_pay(params).await,
            "connect" => self.handle_connect(params).await,
            "disconnect" => self.handle_disconnect(params).await,
            "fundchannel" => self.handle_fundchannel(params).await,
            "close" => self.handle_close(params).await,
            _ => {
                // Return error for unknown methods
                Err(HarnessError::RpcError(format!("Unknown method: {}", method)))
            }
        }
    }

    /// Handle getinfo RPC call
    async fn handle_getinfo(&self) -> Result<Value, HarnessError> {
        // In a real implementation, this would query the actual Rust-Lightning node
        Ok(json!({
            "id": "02f1a9c...",
            "alias": "rust-lightning-node",
            "color": "02f1a9c",
            "num_peers": 0,
            "num_pending_channels": 0,
            "num_active_channels": 0,
            "num_inactive_channels": 0,
            "address": [],
            "binding": [],
            "version": "0.0.1",
            "blockheight": 800000,
            "network": "bitcoin",
            "msatoshi_fees_collected": 0,
            "fees_collected_msat": "0msat",
            "lightning-dir": "/home/user/.lightning"
        }))
    }

    /// Handle listpeers RPC call
    async fn handle_listpeers(&self) -> Result<Value, HarnessError> {
        Ok(json!({
            "peers": []
        }))
    }

    /// Handle listchannels RPC call
    async fn handle_listchannels(&self) -> Result<Value, HarnessError> {
        Ok(json!({
            "channels": []
        }))
    }

    /// Handle invoice RPC call
    async fn handle_invoice(&self, params: Value) -> Result<Value, HarnessError> {
        // Extract amount_msat from params
        let amount_msat = params.get("amount_msat")
            .and_then(|v| v.as_str())
            .ok_or_else(|| HarnessError::RpcError("Missing required parameter: amount_msat".to_string()))?;

        // In a real implementation, this would create an actual invoice using Rust-Lightning
        Ok(json!({
            "payment_hash": "abc123...",
            "expires_at": 1234567890,
            "bolt11": "lnbc1...",
            "payment_secret": "def456...",
            "warning_capacity": "",
            "warning_offline": "",
            "warning_deadends": "",
            "warning_private_unused": "",
            "warning_mpp": ""
        }))
    }

    /// Handle pay RPC call
    async fn handle_pay(&self, params: Value) -> Result<Value, HarnessError> {
        let bolt11 = params.get("bolt11")
            .and_then(|v| v.as_str())
            .ok_or_else(|| HarnessError::RpcError("Missing required parameter: bolt11".to_string()))?;

        // In a real implementation, this would attempt to pay using Rust-Lightning
        Ok(json!({
            "payment_preimage": "def456...",
            "payment_hash": "abc123...",
            "created_at": 1234567890,
            "parts": 1,
            "amount_msat": 1000,
            "amount_sent_msat": 1000,
            "payment_secret": "ghi789...",
            "status": "complete"
        }))
    }

    /// Handle connect RPC call
    async fn handle_connect(&self, params: Value) -> Result<Value, HarnessError> {
        let _id = params.get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| HarnessError::RpcError("Missing required parameter: id".to_string()))?;

        // In a real implementation, this would connect to the peer using Rust-Lightning
        Ok(json!({
            "id": "02f1a9c...",
            "features": "02a2a2a2a2a2a2a2",
            "direction": "out"
        }))
    }

    /// Handle disconnect RPC call
    async fn handle_disconnect(&self, params: Value) -> Result<Value, HarnessError> {
        let _id = params.get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| HarnessError::RpcError("Missing required parameter: id".to_string()))?;

        // In a real implementation, this would disconnect the peer using Rust-Lightning
        Ok(json!({}))
    }

    /// Handle fundchannel RPC call
    async fn handle_fundchannel(&self, params: Value) -> Result<Value, HarnessError> {
        let _id = params.get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| HarnessError::RpcError("Missing required parameter: id".to_string()))?;
        let _amount = params.get("amount")
            .and_then(|v| v.as_str())
            .ok_or_else(|| HarnessError::RpcError("Missing required parameter: amount".to_string()))?;

        // In a real implementation, this would fund a channel using Rust-Lightning
        Ok(json!({
            "tx": "0200000001...",
            "txid": "abc123...",
            "channel_id": "def456...",
            "outnum": 0
        }))
    }

    /// Handle close RPC call
    async fn handle_close(&self, params: Value) -> Result<Value, HarnessError> {
        let _id = params.get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| HarnessError::RpcError("Missing required parameter: id".to_string()))?;

        // In a real implementation, this would close the channel using Rust-Lightning
        Ok(json!({
            "type": "mutual",
            "tx": "0200000001...",
            "txid": "abc123..."
        }))
    }
}

/// Create the HTTP server routes
fn create_routes(
    harness: Arc<RustLightningHarness>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let harness_filter = warp::any().map(move || harness.clone());

    // Health check endpoint
    let health =
        warp::path("health").and(warp::get()).map(|| warp::reply::json(&json!({"status": "ok"})));

    // RPC endpoint
    let rpc = warp::path("rpc")
        .and(warp::post())
        .and(harness_filter)
        .and(warp::body::json())
        .and_then(handle_rpc_request);

    health.or(rpc)
}

/// Handle RPC requests
async fn handle_rpc_request(
    harness: Arc<RustLightningHarness>,
    request: Value,
) -> Result<impl warp::Reply, warp::Rejection> {
    // Extract method and params from the request
    let method = request
        .get("method")
        .and_then(|v| v.as_str())
        .ok_or_else(|| warp::reject::custom(RpcRequestError::InvalidMethod()))?;

    let params = request.get("params").cloned().unwrap_or(json!({}));

    // Handle the RPC call
    match harness.handle_rpc_call(method, params).await {
        Ok(result) => {
            let response = json!({
                "jsonrpc": "2.0",
                "id": request.get("id").cloned().unwrap_or(json!(1)),
                "result": result
            });
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            let error_response = json!({
                "jsonrpc": "2.0",
                "id": request.get("id").cloned().unwrap_or(json!(1)),
                "error": {
                    "code": -32601,
                    "message": e.to_string()
                }
            });
            Ok(warp::reply::json(&error_response))
        }
    }
}

/// Error for invalid RPC requests
#[derive(Debug)]
struct RpcRequestError {
    message: String,
}

impl RpcRequestError {
    fn InvalidMethod() -> Self { Self { message: "Invalid method".to_string() } }
}

impl warp::reject::Reject for RpcRequestError {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Get the port from environment or use default
    let port: u16 = std::env::var("RUST_LIGHTNING_HARNESS_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9836);

    let addr: SocketAddr = ([0, 0, 0, 0], port).into();

    // Create the harness
    let harness = Arc::new(RustLightningHarness::new());

    // Create the routes
    let routes = create_routes(harness);

    println!("Rust-Lightning Harness starting on {}", addr);
    println!("Health check: http://{}:{}/health", addr.ip(), addr.port());
    println!("RPC endpoint: http://{}:{}/rpc", addr.ip(), addr.port());

    // Start the server
    warp::serve(routes).run(addr).await;

    Ok(())
}
