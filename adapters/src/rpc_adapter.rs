//! Unified Protocol Adapter
//!
//! This adapter is responsible for ingesting an external protocol schema
//! (as produced in `resources/ir/**`), and converting that schema into Ethos's
//! internal IR (`ir::ProtocolIR`). In other words: given a protocol-specific
//! interface (e.g., Bitcoin Core RPC, Floresta API),
//! this adapter maps the protocol operations into our canonical intermediate
//! representation so the rest of the pipeline (compiler, codegen, fuzzing, backends)
//! can operate uniformly.
//!
//! How it fits together
//! - Input: a protocol-specific schema that defines protocol operations.
//! - Conversion: adapter-specific logic extracts and normalizes operations
//!   into `ProtocolIR` via `extract_protocol_ir`.
//! - Output: a `ProtocolIR` that downstream components can consume without
//!   specific knowledge about the original protocol or transport details.

use std::path::Path;

use ir::ProtocolIR;
use plugins::{AdapterPlugin, Plugin};
use types::Implementation;

use crate::normalization_registry::AdapterKind;
use crate::{ProtocolAdapter, ProtocolAdapterResult, CAP_RPC};

/// Unified protocol adapter that can handle multiple Bitcoin ecosystem protocols and frameworks
pub struct RpcAdapter {
    /// The specific implementation (e.g., "bitcoin_core")
    pub implementation: Implementation,
    /// Version metadata for this adapter (e.g., 30.0.0, 25.09.1).
    pub version: Option<String>,
    /// Unified backend trait object for protocol interaction, encapsulating all backend logic
    backend: Box<dyn ProtocolBackend + Send + Sync>,
}

/// Abstraction for protocol backend invocation
///
/// This trait abstracts over different invocation methods (RPC calls, direct method calls, etc.)
/// by using JSON as a universal format. Implementations convert between their native types
/// and JSON for cross-adapter compatibility (fuzzing, codegen, etc.).
///
/// The trait is transport-agnostic: backends can use RPC (network), direct calls (embedded),
/// or any other mechanism - they all expose the same JSON-based interface.
#[async_trait::async_trait]
pub trait ProtocolBackend: Send + Sync {
    /// Return a stable identifier for this backend implementation
    fn name(&self) -> &'static str;
    /// Return the backend runtime/client version string.
    fn version(&self) -> String;
    /// List supported capabilities (e.g., `CAP_RPC`)
    fn capabilities(&self) -> Vec<&'static str>;
    /// Extract the protocol IR from the provided path
    fn extract_protocol_ir(&self, path: &std::path::Path) -> ProtocolAdapterResult<ProtocolIR>;
    /// Perform a protocol operation with the given method and parameters
    ///
    /// This method takes a method name and JSON parameters, returning JSON results.
    /// The JSON format is universal regardless of the underlying transport (RPC, direct calls, etc.).
    async fn call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>>;
    /// Normalize a backend-specific output value into a canonical shape
    fn normalize_output(&self, value: &serde_json::Value) -> serde_json::Value;
}

/// Provider-based backend registration
pub struct RegisteredBackend {
    /// The implementation of the backend
    pub implementation: types::Implementation,
    /// The function to build the backend
    pub build: fn() -> crate::ProtocolAdapterResult<Box<dyn ProtocolBackend + Send + Sync>>,
}

/// Trait for providing a backend
pub trait BackendProvider {
    /// The implementation of the backend
    fn implementation() -> types::Implementation;
    /// The function to build the backend
    fn build() -> crate::ProtocolAdapterResult<Box<dyn ProtocolBackend + Send + Sync>>;
}

/// Registered backends
pub static REGISTERED_BACKENDS: &[RegisteredBackend] = &[];

impl RpcAdapter {
    /// Create a new RPC adapter for a specific implementation
    pub fn new(
        implementation: Implementation,
        version: Option<String>,
    ) -> crate::ProtocolAdapterResult<Self> {
        let entry = REGISTERED_BACKENDS
            .iter()
            .find(|r| r.implementation == implementation)
            .ok_or_else(|| {
                crate::ProtocolAdapterError::UnsupportedImplementation(implementation.to_string())
            })?;
        let backend = (entry.build)()?;
        Ok(Self::with_backend(implementation, version, backend))
    }

    /// Create a new protocol adapter for a specific implementation with a provided backend
    pub fn with_backend(
        implementation: Implementation,
        version: Option<String>,
        backend: Box<dyn ProtocolBackend + Send + Sync>,
    ) -> Self {
        Self { implementation, version, backend }
    }

    /// Create a new RPC adapter from pre-loaded ProtocolIR
    pub fn from_ir(
        implementation: Implementation,
        ir: ProtocolIR,
    ) -> crate::ProtocolAdapterResult<Self> {
        let version = Some(ir.version().to_string());
        Self::new(implementation, version)
    }
}

impl ProtocolAdapter for RpcAdapter {
    fn name(&self) -> &'static str { self.implementation.as_str() }

    /// Return the adapter/IR (schema) version for this adapter.
    fn version(&self) -> String {
        self.version
            .clone()
            .expect("RpcAdapter version not set; construct with explicit version or via from_ir()")
    }

    fn capabilities(&self) -> Vec<&'static str> { vec![CAP_RPC] }

    fn extract_protocol_ir(&self, path: &Path) -> ProtocolAdapterResult<ProtocolIR> {
        match ProtocolIR::from_file(path) {
            Ok(ir) => Ok(ir),
            Err(e) => Err(crate::ProtocolAdapterError::Message(e.to_string())),
        }
    }
}

impl Plugin for RpcAdapter {
    fn name(&self) -> &'static str { ProtocolAdapter::name(self) }
}

impl AdapterPlugin for RpcAdapter {
    fn extract_protocol_ir(
        &self,
        path: &Path,
    ) -> Result<ProtocolIR, Box<dyn std::error::Error + Send + Sync>> {
        ProtocolAdapter::extract_protocol_ir(self, path)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }
}

#[async_trait::async_trait]
impl fuzz_types::ProtocolAdapter for RpcAdapter {
    fn name(&self) -> &'static str { self.implementation.as_str() }

    async fn apply_fuzz_case(
        &self,
        case: &fuzz_types::FuzzCase,
    ) -> Result<fuzz_types::FuzzResult, Box<dyn std::error::Error + Send + Sync>> {
        let start_time = std::time::Instant::now();

        // Translate canonical method name to adapter-specific name
        let adapter_kind = AdapterKind::BitcoinCore;
        let method_name =
            crate::normalization_registry::NormalizationRegistry::for_adapter(adapter_kind)
                .unwrap_or_default()
                .to_adapter_method(adapter_kind, &case.method_name);

        let params = serde_json::to_value(&case.parameters)?;
        let result = self.backend.call(&method_name, params).await;

        let (success, response, error) = match result {
            Ok(value) => (true, value, None),
            Err(e) => (false, serde_json::Value::Null, Some(e.to_string())),
        };

        let execution_time = start_time.elapsed().as_millis() as u64;

        Ok(fuzz_types::FuzzResult {
            adapter_name: <Self as fuzz_types::ProtocolAdapter>::name(self).to_string(),
            raw_response: response,
            success,
            error,
            normalized_error: None,
            execution_time_ms: execution_time,
        })
    }

    fn normalize_output(&self, value: &serde_json::Value) -> serde_json::Value {
        // Use unified normalization registry created on demand
        let adapter_kind = AdapterKind::BitcoinCore;
        let registry =
            crate::normalization_registry::NormalizationRegistry::for_adapter(adapter_kind)
                .unwrap_or_default();
        let (normalized, _metadata) = registry.normalize_value(value);
        normalized
    }
}
