//! Version-specific type generator trait for extensible implementation support
//!
//! This module defines the trait that any implementation (Bitcoin Core, etc.)
//! can implement to provide version-specific type generation capabilities.

use ir::{ProtocolIR, RpcDef};
use types::ProtocolVersion;

use crate::Result;

/// Trait for version-specific type generators
///
/// This trait allows different implementations to provide their own version-specific
/// type generation logic. Each implementation extracts metadata directly from the IR
/// and generates accurate, version-specific types.
pub trait VersionedTypeGenerator: Send + Sync {
    /// Create generator from IR
    ///
    /// This method creates a generator from ProtocolIR, extracting metadata directly
    /// from the IR instead of loading from separate files.
    fn from_ir(version: ProtocolVersion, ir: &ProtocolIR) -> Result<Self>
    where
        Self: Sized;

    /// Generate version-specific response types
    ///
    /// Generates response type definitions that are accurate for the specific
    /// version of the implementation. These types should match the actual
    /// JSON-RPC responses from that version.
    fn generate_response_types(&self, methods: &[RpcDef]) -> Result<Vec<(String, String)>>;

    /// Generate version-specific client trait
    ///
    /// Generates the client trait with method signatures that are accurate
    /// for the specific version of the implementation.
    fn generate_client_trait(
        &self,
        implementation: &str,
        methods: &[RpcDef],
    ) -> Result<Vec<(String, String)>>;

    /// Check if this generator supports a given version
    ///
    /// Returns true if this generator can handle the specified version.
    /// This allows the registry to determine which generator to use.
    fn supports_version(&self, version: &ProtocolVersion) -> bool;

    /// Get the implementation name this generator handles
    ///
    /// Returns the string identifier for the implementation this generator
    /// is designed to handle (e.g., "bitcoin_core").
    fn implementation(&self) -> &'static str;
}
