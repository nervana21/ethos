//! Registry for version-specific type generators
//!
//! This module provides a registry that manages different version-specific
//! generators for various implementations (Bitcoin Core, Core Lightning, etc.).

use ir::{ProtocolIR, RpcDef};
use types::ProtocolVersion;

use super::bitcoin_core_versioned::BitcoinCoreVersionedGenerator;
use super::core_lightning_versioned::CoreLightningVersionedGenerator;
use super::versioned_generator::VersionedTypeGenerator;
use crate::Result;

/// Registry for managing version-specific type generators
///
/// This registry is bound to a specific implementation and version at construction time,
/// eliminating the need to pass these parameters to generation methods.
pub struct VersionedGeneratorRegistry {
    implementation: String,
    version: ProtocolVersion,
    generator: Box<dyn VersionedTypeGenerator>,
}

impl VersionedGeneratorRegistry {
    /// Create a version-specific registry from IR
    ///
    /// This method creates the appropriate generator for the specified implementation
    /// using the provided IR instead of loading from separate metadata files.
    pub fn from_ir(
        implementation: &str,
        version: ProtocolVersion,
        ir: &ProtocolIR,
    ) -> Result<Self> {
        let generator: Box<dyn VersionedTypeGenerator> = match implementation {
            "bitcoin_core" => {
                let bitcoin_gen = BitcoinCoreVersionedGenerator::from_ir(version.clone(), ir)
                    .map_err(|e| {
                        format!("Failed to create Bitcoin Core versioned generator from IR: {}", e)
                    })?;
                Box::new(bitcoin_gen)
            }
            "core_lightning" => {
                let cln_gen = CoreLightningVersionedGenerator::from_ir(version.clone(), ir)
                    .map_err(|e| {
                        format!(
                            "Failed to create Core Lightning versioned generator from IR: {}",
                            e
                        )
                    })?;
                Box::new(cln_gen)
            }
            _ => {
                return Err(format!(
                    "No version-specific generator available for implementation: {}",
                    implementation
                )
                .into());
            }
        };

        Ok(Self { implementation: implementation.to_string(), version, generator })
    }

    /// Get the implementation this registry is bound to
    pub fn implementation(&self) -> &str { &self.implementation }

    /// Get the version this registry is bound to
    pub fn version(&self) -> &ProtocolVersion { &self.version }

    /// Generate response types using the bound generator and version
    pub fn generate_response_types(&self, methods: &[RpcDef]) -> Result<Vec<(String, String)>> {
        self.generator.generate_response_types(methods)
    }

    /// Generate client trait using the bound generator and version
    pub fn generate_client_trait(&self, methods: &[RpcDef]) -> Result<Vec<(String, String)>> {
        self.generator.generate_client_trait(&self.implementation, methods)
    }
}
