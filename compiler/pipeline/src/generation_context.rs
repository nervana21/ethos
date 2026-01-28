//! Generation context for code generation pipeline.
//!
//! This module provides a unified context that encapsulates all metadata needed
//! for code generation.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use analysis::CompilerDiagnostics;
use codegen::generators::versioned_registry::VersionedGeneratorRegistry;
use ir::{ProtocolIR, RpcDef};
use types::Implementation;

use crate::PipelineError;

/// Context containing all metadata needed for code generation
pub struct GenerationContext {
    /// The implementation being generated for
    pub implementation: Implementation,
    /// The protocol IR containing semantic analysis results
    pub protocol_ir: ProtocolIR,
    /// The RPC method definitions to generate code for
    pub rpc_methods: Vec<RpcDef>,
    /// Collected external symbols used by generators, keyed by crate name (e.g., "bitcoin")
    pub used_external_symbols: Arc<UsedExternalSymbols>,
    /// The compiler diagnostics from analysis
    pub diagnostics: CompilerDiagnostics,
    /// The versioned generator registry containing version and implementation-specific generators
    pub versioned_registry: VersionedGeneratorRegistry,
    /// The base output directory for generated files
    pub base_output_dir: PathBuf,
}

impl GenerationContext {
    /// Create a new builder for GenerationContext
    pub fn builder() -> GenerationContextBuilder { GenerationContextBuilder::default() }

    /// Get the protocol display name
    pub fn protocol_name(&self) -> String { self.implementation.display_name().to_string() }

    /// Get the client trait name for this implementation
    pub fn client_name(&self) -> String {
        self.implementation.client_prefix().to_string()
    }

    /// Get the artifact name (crate name without version)
    pub fn artifact_name(&self) -> String { self.implementation.crate_name().to_string() }

    /// Gets the full published crate name
    pub fn full_crate_name(&self) -> String {
        self.implementation.published_crate_name().to_string()
    }

    /// Get the transport protocol for this implementation
    pub fn transport_protocol(&self) -> String {
        self.implementation.transport_protocol().to_string()
    }
}

#[derive(Default)]
/// Builder for GenerationContext
pub struct GenerationContextBuilder {
    /// The implementation being generated for
    implementation: Option<Implementation>,
    /// The protocol IR containing semantic analysis results
    protocol_ir: Option<ProtocolIR>,
    /// The RPC method definitions to generate code for
    rpc_methods: Option<Vec<RpcDef>>,
    /// Collected external symbols used by generators (shared)
    used_external_symbols: Option<Arc<UsedExternalSymbols>>,
    /// The compiler diagnostics from analysis
    diagnostics: Option<CompilerDiagnostics>,
    /// The versioned generator registry containing version and implementation-specific generators
    versioned_registry: Option<VersionedGeneratorRegistry>,
    /// The base output directory for generated files
    base_output_dir: Option<PathBuf>,
}

impl GenerationContextBuilder {
    /// Set the implementation
    pub fn implementation(mut self, implementation: Implementation) -> Self {
        self.implementation = Some(implementation);
        self
    }

    /// Set the RPC methods
    pub fn rpc_methods(mut self, rpc_methods: Vec<RpcDef>) -> Self {
        self.rpc_methods = Some(rpc_methods);
        self
    }

    /// Provide the external symbol collector (optional; created by default if not provided)
    pub fn used_external_symbols(mut self, collector: Arc<UsedExternalSymbols>) -> Self {
        self.used_external_symbols = Some(collector);
        self
    }

    /// Set the versioned registry
    pub fn versioned_registry(mut self, registry: VersionedGeneratorRegistry) -> Self {
        self.versioned_registry = Some(registry);
        self
    }

    /// Set the output directory
    pub fn output_dir(mut self, dir: PathBuf) -> Self {
        self.base_output_dir = Some(dir);
        self
    }

    /// Set the protocol IR
    pub fn protocol_ir(mut self, ir: ProtocolIR) -> Self {
        self.protocol_ir = Some(ir);
        self
    }

    /// Set the compiler diagnostics
    pub fn diagnostics(mut self, diagnostics: CompilerDiagnostics) -> Self {
        self.diagnostics = Some(diagnostics);
        self
    }

    /// Build the GenerationContext
    pub fn build(self) -> Result<GenerationContext, PipelineError> {
        Ok(GenerationContext {
            implementation: self
                .implementation
                .ok_or_else(|| PipelineError::Message("implementation is required".to_string()))?,
            protocol_ir: self
                .protocol_ir
                .ok_or_else(|| PipelineError::Message("protocol_ir is required".to_string()))?,
            rpc_methods: self
                .rpc_methods
                .ok_or_else(|| PipelineError::Message("rpc_methods is required".to_string()))?,
            used_external_symbols: self
                .used_external_symbols
                .unwrap_or_else(|| Arc::new(UsedExternalSymbols::new())),
            diagnostics: self
                .diagnostics
                .ok_or_else(|| PipelineError::Message("diagnostics is required".to_string()))?,
            versioned_registry: self.versioned_registry.ok_or_else(|| {
                PipelineError::Message("versioned_registry is required".to_string())
            })?,
            base_output_dir: self
                .base_output_dir
                .ok_or_else(|| PipelineError::Message("base_output_dir is required".to_string()))?,
        })
    }
}

/// Thread-safe registry of external symbols used by code generators
#[derive(Default)]
pub struct UsedExternalSymbols {
    inner: Mutex<BTreeMap<String, BTreeSet<String>>>,
}

impl UsedExternalSymbols {
    /// Create a new instance of UsedExternalSymbols
    pub fn new() -> Self { Self { inner: Mutex::new(BTreeMap::new()) } }

    /// Record usage of a symbol from a given crate (e.g., ("bitcoin", "Address"))
    pub fn record(&self, crate_name: &str, symbol: &str) {
        let mut map = self.inner.lock().expect("collector poisoned");
        map.entry(crate_name.to_string()).or_default().insert(symbol.to_string());
    }

    /// Get a sorted list of symbols recorded for a given crate
    pub fn symbols_for_crate(&self, crate_name: &str) -> Vec<String> {
        let map = self.inner.lock().expect("collector poisoned");
        map.get(crate_name).map(|set| set.iter().cloned().collect()).unwrap_or_default()
    }
}
