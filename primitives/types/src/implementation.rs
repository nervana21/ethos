//! Type-safe implementation names for Bitcoin protocol implementations.
//!
//! This module provides the `Implementation` enum that provides
//! compile-time validation.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Protocol names for Bitcoin protocol implementations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Protocol {
	/// Bitcoin protocol
	#[default]
	#[serde(rename = "bitcoin")]
	Bitcoin,
	/// Lightning protocol
	#[serde(rename = "lightning")]
	Lightning,
}

impl Protocol {
	/// Get the string representation of the protocol name.
	pub fn as_str(&self) -> &'static str {
		match self {
			Protocol::Bitcoin => "bitcoin",
			Protocol::Lightning => "lightning",
		}
	}
}

impl FromStr for Protocol {
	type Err = String;

	fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
		match s {
			"bitcoin" => Ok(Protocol::Bitcoin),
			"lightning" => Ok(Protocol::Lightning),
			_ => Err(format!("Unknown protocol name: {}", s)),
		}
	}
}

impl fmt::Display for Protocol {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.as_str())
	}
}

/// Type-safe implementation names for Bitcoin protocol implementations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Implementation {
	/// Bitcoin Core implementation
	#[default]
	BitcoinCore,
	/// Core Lightning implementation
	CoreLightning,
	/// LND (Lightning Network Daemon) implementation
	Lnd,
	/// Rust Lightning implementation
	RustLightning,
}

impl Implementation {
	/// Get the string representation of the implementation name.
	pub fn as_str(&self) -> &'static str {
		match self {
			Implementation::BitcoinCore => "bitcoin_core",
			Implementation::CoreLightning => "core_lightning",
			Implementation::Lnd => "lnd",
			Implementation::RustLightning => "rust_lightning",
		}
	}

	/// Get the protocol name that this implementation supports.
	pub fn protocol_name(&self) -> String {
		match self {
			Implementation::BitcoinCore => "bitcoin".to_string(),
			Implementation::CoreLightning => "lightning".to_string(),
			Implementation::Lnd => "lightning".to_string(),
			Implementation::RustLightning => "lightning".to_string(),
		}
	}

	/// Get the human-readable display name for the implementation.
	pub fn display_name(&self) -> &'static str {
		match self {
			Implementation::BitcoinCore => "Bitcoin Core",
			Implementation::CoreLightning => "Core Lightning",
			Implementation::Lnd => "Lightning Network Daemon",
			Implementation::RustLightning => "Rust Lightning",
		}
	}

	/// Get the crate name for the implementation (with hyphens).
	pub fn crate_name(&self) -> &'static str {
		match self {
			Implementation::BitcoinCore => "bitcoin-core",
			Implementation::CoreLightning => "core-lightning",
			Implementation::Lnd => "lnd",
			Implementation::RustLightning => "rust-lightning",
		}
	}

	/// Get the client directory name for the implementation.
	pub fn client_dir_name(&self) -> &'static str {
		match self {
			Implementation::BitcoinCore => "bitcoin_core_clients",
			Implementation::CoreLightning => "core_lightning_clients",
			Implementation::Lnd => "lnd_clients",
			Implementation::RustLightning => "rust_lightning_clients",
		}
	}

	/// Get the transport protocol for the implementation.
	pub fn transport_protocol(&self) -> &'static str {
		match self {
			Implementation::BitcoinCore => "http",
			Implementation::CoreLightning => "unix",
			Implementation::Lnd => "http",
			Implementation::RustLightning => "http",
		}
	}

	/// Get the executable name for the implementation.
	pub fn executable_name(&self) -> &'static str {
		match self {
			Implementation::BitcoinCore => "bitcoind",
			Implementation::CoreLightning => "lightningd",
			Implementation::Lnd => "lnd",
			Implementation::RustLightning => "lightning",
		}
	}

	/// Get the test client class name prefix.
	pub fn test_client_prefix(&self) -> &'static str {
		match self {
			Implementation::BitcoinCore => "BitcoinTestClient",
			Implementation::CoreLightning => "CoreLightningTestClient",
			Implementation::Lnd => "LndTestClient",
			Implementation::RustLightning => "RustLightningTestClient",
		}
	}

	/// Get the node manager name for the implementation.
	pub fn node_manager_name(&self) -> &'static str {
		match self {
			Implementation::BitcoinCore => "BitcoinNodeManager",
			Implementation::CoreLightning => "CoreLightningNodeManager",
			Implementation::Lnd => "LndNodeManager",
			Implementation::RustLightning => "RustLightningNodeManager",
		}
	}

	/// Get the client class name prefix.
	pub fn client_prefix(&self) -> &'static str {
		match self {
			Implementation::BitcoinCore => "BitcoinClient",
			Implementation::CoreLightning => "CoreLightningClient",
			Implementation::Lnd => "LndClient",
			Implementation::RustLightning => "RustLightningClient",
		}
	}

	/// Get the example method name for documentation.
	pub fn example_method(&self) -> &'static str {
		match self {
			Implementation::BitcoinCore => "getblockchaininfo",
			Implementation::CoreLightning => "getinfo",
			Implementation::Lnd => "getinfo",
			Implementation::RustLightning => "getinfo",
		}
	}

	/// Get the example method description for documentation.
	pub fn example_description(&self) -> &'static str {
		match self {
			Implementation::BitcoinCore => "Blockchain info",
			Implementation::CoreLightning => "Node info",
			Implementation::Lnd => "Node info",
			Implementation::RustLightning => "Node info",
		}
	}

	/// Get node metadata for this implementation
	pub fn node_metadata(&self) -> crate::node_metadata::NodeMetadata {
		match self {
			Implementation::BitcoinCore => crate::node_metadata::NodeMetadata {
				executable: "bitcoind".to_string(),
				transport: "http".to_string(),
				requires_auth: true,
				cli_args: crate::node_metadata::CliArgs::new()
					.add_value_arg("chain", "-chain={}")
					.add_value_arg("datadir", "-datadir={}")
					.add_value_arg("rpc_port", "-rpcport={}")
					.add_value_arg("rpc_bind", "-rpcbind=127.0.0.1:{}")
					.add_value_arg("rpc_user", "-rpcuser={}")
					.add_value_arg("rpc_password", "-rpcpassword={}")
					.add_static_arg("-listen=0")
					.add_static_arg("-rpcallowip=127.0.0.1")
					.add_static_arg("-fallbackfee=0.0002")
					.add_static_arg("-server=1")
					.add_static_arg("-prune=1"),
				readiness_method: "getnetworkinfo".to_string(),
				initialization_error_codes: vec![-28, -4],
				socket_path_pattern: None,
			},
			Implementation::CoreLightning => crate::node_metadata::NodeMetadata {
				executable: "lightningd".to_string(),
				transport: "unix".to_string(),
				requires_auth: false,
				cli_args: crate::node_metadata::CliArgs::new()
					.add_value_arg("network", "--network={}")
					.add_value_arg("lightning_dir", "--lightning-dir={}")
					.add_value_arg("bind_addr", "--bind-addr=127.0.0.1:{}")
					.add_value_arg("log_file", "--log-file={}/lightningd.log")
					.add_static_arg("--daemon")
					.add_static_arg("--disable-plugin=cln-grpc")
					.add_static_arg("--disable-plugin=clnrest")
					.add_static_arg("--disable-plugin=wss-proxy")
					.add_static_arg("--disable-plugin=cln-lsps-service")
					.add_static_arg("--disable-plugin=cln-lsps-client")
					.add_static_arg("--disable-plugin=cln-bip353"),
				readiness_method: "getinfo".to_string(),
				initialization_error_codes: vec![],
				socket_path_pattern: Some("{datadir}/regtest/lightning-rpc".to_string()),
			},
			Implementation::Lnd => crate::node_metadata::NodeMetadata {
				executable: "lnd".to_string(),
				transport: "http".to_string(),
				requires_auth: false,
				cli_args: crate::node_metadata::CliArgs::new()
					.add_value_arg("datadir", "--datadir={}")
					.add_value_arg("rpc_port", "--rpcport={}")
					.add_static_arg("--regtest")
					.add_static_arg("--no-macaroons"),
				readiness_method: "getinfo".to_string(),
				initialization_error_codes: vec![],
				socket_path_pattern: None,
			},
			Implementation::RustLightning => crate::node_metadata::NodeMetadata {
				executable: "lightning".to_string(),
				transport: "http".to_string(),
				requires_auth: false,
				cli_args: crate::node_metadata::CliArgs::new()
					.add_value_arg("network", "--network={}")
					.add_value_arg("datadir", "--data-dir={}")
					.add_static_arg("--regtest"),
				readiness_method: "getinfo".to_string(),
				initialization_error_codes: vec![],
				socket_path_pattern: None,
			},
		}
	}

	/// Create a type adapter for this implementation.
	///
	/// # Returns
	///
	/// Returns a boxed type adapter suitable for code generation.
	///
	/// # Errors
	///
	/// Returns an error if the implementation doesn't have a type adapter yet.
	/// Currently supported: BitcoinCore, CoreLightning
	pub fn create_type_adapter(&self) -> Result<Box<dyn crate::type_adapter::TypeAdapter>, String> {
		match self {
			Implementation::BitcoinCore => Ok(Box::new(crate::adapters::BitcoinCoreAdapter)),
			Implementation::CoreLightning => Ok(Box::new(crate::adapters::CoreLightningAdapter)),
			Implementation::Lnd | Implementation::RustLightning => Err(format!(
				"Type adapter not yet implemented for {}. \
					 Currently supported: bitcoin_core, core_lightning",
				self.as_str()
			)),
		}
	}
}

impl FromStr for Implementation {
	type Err = String;

	fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
		match s {
			"bitcoin_core" => Ok(Implementation::BitcoinCore),
			"core_lightning" => Ok(Implementation::CoreLightning),
			"lnd" => Ok(Implementation::Lnd),
			"rust_lightning" => Ok(Implementation::RustLightning),
			_ => Err(format!("Unknown implementation name: {}", s)),
		}
	}
}

impl From<&str> for Implementation {
	fn from(s: &str) -> Self {
		Self::from_str(s).unwrap_or_else(|_| panic!("Invalid implementation name: {}", s))
	}
}

impl From<String> for Implementation {
	fn from(s: String) -> Self {
		Self::from_str(&s).unwrap_or_else(|_| panic!("Invalid implementation name: {}", s))
	}
}

impl fmt::Display for Implementation {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.as_str())
	}
}
