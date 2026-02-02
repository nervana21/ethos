//! Bitcoin Core version-specific type generator

use ir::{ProtocolIR, RpcDef};
use types::ProtocolVersion;

use super::version_specific_client_trait::VersionSpecificClientTraitGenerator;
use super::version_specific_response_type::VersionSpecificResponseTypeGenerator;
use super::versioned_generator::VersionedTypeGenerator;
use crate::{CodeGenerator, Result};

/// Bitcoin Core version-specific type generator
pub struct BitcoinCoreVersionedGenerator {
    version: ProtocolVersion,
}

impl VersionedTypeGenerator for BitcoinCoreVersionedGenerator {
    fn from_ir(version: ProtocolVersion, _ir: &ProtocolIR) -> Result<Self> { Ok(Self { version }) }

    fn generate_response_types(&self, methods: &[RpcDef]) -> Result<Vec<(String, String)>> {
        VersionSpecificResponseTypeGenerator::new(self.version.clone(), "bitcoin_core".to_string())
            .generate(methods)
    }

    fn generate_client_trait(
        &self,
        implementation: &str,
        methods: &[RpcDef],
    ) -> Result<Vec<(String, String)>> {
        Ok(VersionSpecificClientTraitGenerator::new(self.version.clone(), implementation)
            .generate(methods))
    }

    fn supports_version(&self, version: &ProtocolVersion) -> bool {
        // Support versions 17.0 and above
        if version.major < 17 {
            return false;
        }
        // Exclude versions 30.0.x and 30.1.x
        if version.major == 30 && version.minor < 2 {
            return false;
        }
        true
    }

    fn implementation(&self) -> &'static str { "bitcoin_core" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_ir() {
        let version = ProtocolVersion {
            version_string: "v25.0.0".to_string(),
            major: 25,
            minor: 0,
            patch: 0,
            protocol: Some("bitcoin_core".to_string()),
        };
        let ir = ProtocolIR::new(vec![]);

        let generator = <BitcoinCoreVersionedGenerator as VersionedTypeGenerator>::from_ir(
            version.clone(),
            &ir,
        )
        .unwrap();

        assert_eq!(generator.version.as_str(), version.as_str());
    }

    #[test]
    fn test_generate_response_types() {
        let version = ProtocolVersion {
            version_string: "v25.0.0".to_string(),
            major: 25,
            minor: 0,
            patch: 0,
            protocol: Some("bitcoin_core".to_string()),
        };
        let generator = BitcoinCoreVersionedGenerator { version };

        let result =
            <BitcoinCoreVersionedGenerator as VersionedTypeGenerator>::generate_response_types(
                &generator,
                &[],
            )
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "responses.rs");
        assert!(result[0].1.contains("Generated version-specific RPC response types"));
    }

    #[test]
    fn test_generate_client_trait() {
        let version = ProtocolVersion {
            version_string: "v25.0.0".to_string(),
            major: 25,
            minor: 0,
            patch: 0,
            protocol: Some("bitcoin_core".to_string()),
        };
        let generator = BitcoinCoreVersionedGenerator { version };

        let result =
            <BitcoinCoreVersionedGenerator as VersionedTypeGenerator>::generate_client_trait(
                &generator,
                "bitcoin_core",
                &[],
            )
            .unwrap();

        assert_eq!(result.len(), 2);
        let filenames: Vec<_> = result.iter().map(|(name, _)| name.as_str()).collect();
        assert!(filenames.contains(&"client.rs"));
        assert!(filenames.contains(&"mod.rs"));
    }

    #[test]
    fn test_implementation() {
        let generator = BitcoinCoreVersionedGenerator {
            version: ProtocolVersion {
                version_string: "v25.0.0".to_string(),
                major: 25,
                minor: 0,
                patch: 0,
                protocol: Some("bitcoin_core".to_string()),
            },
        };

        assert_eq!(generator.implementation(), "bitcoin_core");
    }
}
