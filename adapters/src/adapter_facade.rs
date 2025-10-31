//! Transport-agnostic Adapter Facade and Invocation Strategies
//!
//! This module exposes a single facade for loading `ProtocolIR` and invoking
//! backend methods across multiple transports (RPC and non-RPC), as well as an
//! offline strategy for deterministic tooling.

use std::path::Path;

use futures::stream::BoxStream;
use ir::ProtocolIR;
use serde_json::Value;

use crate::{ProtocolAdapterError, ProtocolAdapterResult};

/// Strategy selection for the facade
#[derive(Clone, Debug)]
pub enum StrategyKind {
    /// Offline file-based Protocol IR loading; no I/O invocation
    OfflineIr,
    /// Runtime RPC via existing backends/transports
    RpcRuntime,
    /// Placeholder for non-RPC transports (e.g., in-proc, pub/sub)
    NonRpc,
}

/// Configuration passed to the facade
#[derive(Clone, Debug)]
pub struct AdapterConfig {
    /// Which strategy to use
    pub strategy: StrategyKind,
    /// Optional path to a schema/IR file when using OfflineIr
    pub ir_path: Option<String>,
}

impl Default for AdapterConfig {
    fn default() -> Self { Self { strategy: StrategyKind::RpcRuntime, ir_path: None } }
}

/// Encodes/decodes protocol-specific envelopes (JSON-RPC, frames, PSBT ops)
pub trait Envelope {
    /// Encode a method name and params into a transport envelope
    fn encode(&self, method: &str, params: &Value) -> Value;
    /// Decode a transport payload into a generic JSON value
    fn decode(&self, payload: &Value) -> Value;
}

/// Invocation Engine abstraction (async-friendly)
#[async_trait::async_trait]
pub trait InvocationEngine: Send + Sync {
    /// Invoke a single operation and return a value
    async fn invoke(&self, method: &str, params: &Value) -> ProtocolAdapterResult<Value>;

    /// Invoke a batch of operations
    async fn invoke_batch(&self, calls: &[(String, Value)]) -> ProtocolAdapterResult<Vec<Value>> {
        let mut out = Vec::with_capacity(calls.len());
        for (m, p) in calls {
            out.push(self.invoke(m, p).await?);
        }
        Ok(out)
    }

    /// Optional streaming API (e.g., subscriptions); default is unsupported
    fn subscribe(&self, _topic: &str, _params: &Value) -> Option<BoxStream<'static, Value>> { None }
}

/// Offline IR strategy — only loads IR; invocation returns an error
pub struct OfflineIrStrategy;

#[async_trait::async_trait]
impl InvocationEngine for OfflineIrStrategy {
    async fn invoke(&self, _method: &str, _params: &Value) -> ProtocolAdapterResult<Value> {
        Err(ProtocolAdapterError::Message(
            "Invocation not supported in OfflineIr strategy".to_string(),
        ))
    }
}

/// RPC Runtime strategy — delegates to existing RpcAdapter backends
pub struct RpcRuntimeStrategy {
    backend: Box<dyn crate::rpc_adapter::ProtocolBackend + Send + Sync>,
}

impl RpcRuntimeStrategy {
    /// Construct with an existing backend
    pub fn new(backend: Box<dyn crate::rpc_adapter::ProtocolBackend + Send + Sync>) -> Self {
        Self { backend }
    }
}

#[async_trait::async_trait]
impl InvocationEngine for RpcRuntimeStrategy {
    async fn invoke(&self, method: &str, params: &Value) -> ProtocolAdapterResult<Value> {
        let res = self
            .backend
            .call(method, params.clone())
            .await
            .map_err(|e| ProtocolAdapterError::Message(e.to_string()))?;
        Ok(res)
    }
}

/// Non-RPC strategy scaffold — placeholder for future transports
pub struct NonRpcStrategy;

#[async_trait::async_trait]
impl InvocationEngine for NonRpcStrategy {
    async fn invoke(&self, _method: &str, _params: &Value) -> ProtocolAdapterResult<Value> {
        Err(ProtocolAdapterError::Message("Non-RPC strategy not yet implemented".to_string()))
    }
}

/// Facade: one public place to load IR and invoke methods
pub struct AdapterFacade {
    engine: Box<dyn InvocationEngine>,
}

impl AdapterFacade {
    /// Create a facade from the selected strategy
    pub fn from_config(
        config: AdapterConfig,
        backend: Option<Box<dyn crate::rpc_adapter::ProtocolBackend + Send + Sync>>,
    ) -> Self {
        let engine: Box<dyn InvocationEngine> = match config.strategy {
            StrategyKind::OfflineIr => Box::new(OfflineIrStrategy),
            StrategyKind::RpcRuntime => {
                let be = backend.expect("RpcRuntime requires a backend");
                Box::new(RpcRuntimeStrategy::new(be))
            }
            StrategyKind::NonRpc => Box::new(NonRpcStrategy),
        };
        Self { engine }
    }

    /// Load ProtocolIR via the chosen strategy (OfflineIr expects `ir_path`)
    pub fn load_protocol_ir(&self, config: &AdapterConfig) -> ProtocolAdapterResult<ProtocolIR> {
        match config.strategy {
            StrategyKind::OfflineIr => {
                let path = config
                    .ir_path
                    .as_ref()
                    .ok_or_else(|| ProtocolAdapterError::Message("Missing ir_path".to_string()))?;
                ProtocolIR::from_file(Path::new(path))
                    .map_err(|e| ProtocolAdapterError::Message(e.to_string()))
            }
            _ => Err(ProtocolAdapterError::Message(
                "load_protocol_ir is only available for OfflineIr strategy".to_string(),
            )),
        }
    }

    /// Execute a single call via the chosen strategy
    pub async fn execute(&self, method: &str, params: &Value) -> ProtocolAdapterResult<Value> {
        self.engine.invoke(method, params).await
    }
}
