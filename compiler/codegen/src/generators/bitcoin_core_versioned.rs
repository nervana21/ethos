//! Bitcoin Core version-specific type generator

use super::version_specific_client_trait::VersionSpecificClientTraitGenerator;
use super::version_specific_response_type::VersionSpecificResponseTypeGenerator;
use super::versioned_generator::VersionedTypeGenerator;
use crate::{CodeGenerator, Result};
use ir::{ProtocolIR, RpcDef};
use types::ProtocolVersion;

/// Bitcoin Core version-specific type generator
pub struct BitcoinCoreVersionedGenerator {
	version: ProtocolVersion,
}

impl VersionedTypeGenerator for BitcoinCoreVersionedGenerator {
	fn from_ir(version: ProtocolVersion, _ir: &ProtocolIR) -> Result<Self> {
		Ok(Self { version })
	}

	fn generate_response_types(&self, methods: &[RpcDef]) -> Result<Vec<(String, String)>> {
		VersionSpecificResponseTypeGenerator::new(self.version.clone(), "bitcoin_core".to_string())
			.generate(methods)
	}

	fn generate_client_trait(
		&self, implementation: &str, methods: &[RpcDef],
	) -> Result<Vec<(String, String)>> {
		Ok(VersionSpecificClientTraitGenerator::new(self.version.clone(), implementation)
			.generate(methods))
	}

	fn supports_version(&self, version: &ProtocolVersion) -> bool {
		let version_str = version.as_str();
		if let Some(version_num) = version_str.strip_prefix('v') {
			if let Ok(version_float) = version_num.parse::<f64>() {
				return (17.0..=30.0).contains(&version_float);
			}
		}
		false
	}

	fn implementation(&self) -> &'static str {
		"bitcoin_core"
	}
}
