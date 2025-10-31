//! Core Lightning version-specific type generator
//!
//! This module implements the VersionedTypeGenerator trait for Core Lightning,
//! providing version-specific type generation using extracted metadata from
//! Core Lightning's JSON schemas.

use ir::{ProtocolIR, RpcDef};
use types::ProtocolVersion;

use super::version_specific_client_trait::VersionSpecificClientTraitGenerator;
use super::version_specific_response_type::VersionSpecificResponseTypeGenerator;
use super::versioned_generator::VersionedTypeGenerator;
use crate::{CodeGenerator, Result};

/// Core Lightning version-specific type generator
///
/// This generator uses extracted type metadata from Core Lightning's JSON schemas
/// to generate accurate, version-specific types for Core Lightning RPC clients.
pub struct CoreLightningVersionedGenerator {
    version: ProtocolVersion,
}

impl VersionedTypeGenerator for CoreLightningVersionedGenerator {
    fn from_ir(version: ProtocolVersion, _ir: &ProtocolIR) -> Result<Self> { Ok(Self { version }) }

    fn generate_response_types(&self, methods: &[RpcDef]) -> Result<Vec<(String, String)>> {
        VersionSpecificResponseTypeGenerator::new(
            self.version.clone(),
            "core_lightning".to_string(),
        )
        .generate(methods)
    }

    fn generate_client_trait(
        &self,
        implementation: &str,
        methods: &[RpcDef],
    ) -> Result<Vec<(String, String)>> {
        let result = VersionSpecificClientTraitGenerator::new(self.version.clone(), implementation)
            .generate(methods);

        Ok(result)
    }

    fn supports_version(&self, version: &ProtocolVersion) -> bool {
        // Core Lightning supports versions v0.10.1 through v25.09.x
        // Check if the version is within this range
        let version_str = version.as_str();

        // Handle different version formats
        if let Some(version_num) = version_str.strip_prefix('v') {
            // Try to parse as date-based version (e.g., v25.09, v25.09.1)
            if let Some((year, month)) = parse_date_version(version_num) {
                return year <= 25 && (1..=12).contains(&month);
            }

            // Try to parse as semantic version (e.g., v0.10.1)
            if let Some((major, minor, _)) = parse_semantic_version(version_num) {
                // Support versions from v0.10.1 onwards
                return major == 0 && minor >= 10;
            }
        }

        false
    }

    fn implementation(&self) -> &'static str { "core_lightning" }
}

/// Parse a date-based version string (e.g., "25.09", "25.09.1")
fn parse_date_version(version_str: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = version_str.split('.').collect();
    // Support both two-component (25.09) and three-component (25.09.1) versions
    if parts.len() >= 2 {
        if let (Ok(year), Ok(month)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
            return Some((year, month));
        }
    }
    None
}

/// Parse a semantic version string (e.g., "0.10.1")
fn parse_semantic_version(version_str: &str) -> Option<(u32, u32, u32)> {
    let parts: Vec<&str> = version_str.split('.').collect();
    if parts.len() >= 2 {
        if let (Ok(major), Ok(minor)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
            let patch = if parts.len() > 2 { parts[2].parse::<u32>().unwrap_or(0) } else { 0 };
            return Some((major, minor, patch));
        }
    }
    None
}
