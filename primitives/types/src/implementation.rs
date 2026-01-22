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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.as_str()) }
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

/// Metadata for an implementation variant.
struct ImplementationMetadata {
    as_str: &'static str,
    protocol_name: &'static str,
    display_name: &'static str,
    crate_name: &'static str,
    client_dir_name: &'static str,
    transport_protocol: &'static str,
    executable_name: &'static str,
    test_client_prefix: &'static str,
    node_manager_name: &'static str,
    client_prefix: &'static str,
    example_method: &'static str,
    example_description: &'static str,
}

impl ImplementationMetadata {
    const fn new(
        as_str: &'static str,
        protocol_name: &'static str,
        display_name: &'static str,
        crate_name: &'static str,
        client_dir_name: &'static str,
        transport_protocol: &'static str,
        executable_name: &'static str,
        test_client_prefix: &'static str,
        node_manager_name: &'static str,
        client_prefix: &'static str,
        example_method: &'static str,
        example_description: &'static str,
    ) -> Self {
        Self {
            as_str,
            protocol_name,
            display_name,
            crate_name,
            client_dir_name,
            transport_protocol,
            executable_name,
            test_client_prefix,
            node_manager_name,
            client_prefix,
            example_method,
            example_description,
        }
    }
}

const IMPLEMENTATION_METADATA: [ImplementationMetadata; 4] = [
    ImplementationMetadata::new(
        "bitcoin_core",
        "bitcoin",
        "Bitcoin Core",
        "bitcoin-core",
        "bitcoin_core_clients",
        "http",
        "bitcoind",
        "BitcoinTestClient",
        "BitcoinNodeManager",
        "BitcoinClient",
        "getblockchaininfo",
        "Blockchain info",
    ),
    ImplementationMetadata::new(
        "core_lightning",
        "lightning",
        "Core Lightning",
        "core-lightning",
        "core_lightning_clients",
        "unix",
        "lightningd",
        "CoreLightningTestClient",
        "CoreLightningNodeManager",
        "CoreLightningClient",
        "getinfo",
        "Node info",
    ),
    ImplementationMetadata::new(
        "lnd",
        "lightning",
        "Lightning Network Daemon",
        "lnd",
        "lnd_clients",
        "http",
        "lnd",
        "LndTestClient",
        "LndNodeManager",
        "LndClient",
        "getinfo",
        "Node info",
    ),
    ImplementationMetadata::new(
        "rust_lightning",
        "lightning",
        "Rust Lightning",
        "rust-lightning",
        "rust_lightning_clients",
        "http",
        "lightning",
        "RustLightningTestClient",
        "RustLightningNodeManager",
        "RustLightningClient",
        "getinfo",
        "Node info",
    ),
];

impl Implementation {
    /// Get the metadata for this implementation.
    fn metadata(&self) -> &'static ImplementationMetadata {
        let index = match self {
            Implementation::BitcoinCore => 0,
            Implementation::CoreLightning => 1,
            Implementation::Lnd => 2,
            Implementation::RustLightning => 3,
        };
        &IMPLEMENTATION_METADATA[index]
    }

    /// Get the string representation of the implementation name.
    pub fn as_str(&self) -> &'static str { self.metadata().as_str }

    /// Get the protocol name that this implementation supports.
    pub fn protocol_name(&self) -> String { self.metadata().protocol_name.to_string() }

    /// Get the human-readable display name for the implementation.
    pub fn display_name(&self) -> &'static str { self.metadata().display_name }

    /// Get the crate name for the implementation (with hyphens).
    pub fn crate_name(&self) -> &'static str { self.metadata().crate_name }

    /// Get the client directory name for the implementation.
    pub fn client_dir_name(&self) -> &'static str { self.metadata().client_dir_name }

    /// Get the transport protocol for the implementation.
    pub fn transport_protocol(&self) -> &'static str { self.metadata().transport_protocol }

    /// Get the executable name for the implementation.
    pub fn executable_name(&self) -> &'static str { self.metadata().executable_name }

    /// Get the test client class name prefix.
    pub fn test_client_prefix(&self) -> &'static str { self.metadata().test_client_prefix }

    /// Get the node manager name for the implementation.
    pub fn node_manager_name(&self) -> &'static str { self.metadata().node_manager_name }

    /// Get the client class name prefix.
    pub fn client_prefix(&self) -> &'static str { self.metadata().client_prefix }

    /// Get the example method name for documentation.
    pub fn example_method(&self) -> &'static str { self.metadata().example_method }

    /// Get the example method description for documentation.
    pub fn example_description(&self) -> &'static str { self.metadata().example_description }

    /// Get the published crate name for this implementation (e.g., "ethos-bitcoind").
    pub fn published_crate_name(&self) -> &'static str {
        match self {
            Implementation::BitcoinCore => "ethos-bitcoind",
            Implementation::CoreLightning => "ethos-core-lightning",
            Implementation::Lnd => "ethos-lnd",
            Implementation::RustLightning => "ethos-rust-lightning",
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.as_str()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_as_str() {
        assert_eq!(Protocol::Bitcoin.as_str(), "bitcoin");
        assert_eq!(Protocol::Lightning.as_str(), "lightning");
    }

    #[test]
    fn test_implementation_as_str() {
        assert_eq!(Implementation::BitcoinCore.as_str(), "bitcoin_core");
        assert_eq!(Implementation::CoreLightning.as_str(), "core_lightning");
        assert_eq!(Implementation::Lnd.as_str(), "lnd");
        assert_eq!(Implementation::RustLightning.as_str(), "rust_lightning");
    }

    #[test]
    fn test_implementation_protocol_name() {
        assert_eq!(Implementation::BitcoinCore.protocol_name(), "bitcoin".to_string());
        assert_eq!(Implementation::CoreLightning.protocol_name(), "lightning".to_string());
        assert_eq!(Implementation::Lnd.protocol_name(), "lightning".to_string());
        assert_eq!(Implementation::RustLightning.protocol_name(), "lightning".to_string());
    }

    #[test]
    fn test_implementation_display_name() {
        assert_eq!(Implementation::BitcoinCore.display_name(), "Bitcoin Core");
        assert_eq!(Implementation::CoreLightning.display_name(), "Core Lightning");
        assert_eq!(Implementation::Lnd.display_name(), "Lightning Network Daemon");
        assert_eq!(Implementation::RustLightning.display_name(), "Rust Lightning");
    }

    #[test]
    fn test_implementation_crate_name() {
        assert_eq!(Implementation::BitcoinCore.crate_name(), "bitcoin-core");
        assert_eq!(Implementation::CoreLightning.crate_name(), "core-lightning");
        assert_eq!(Implementation::Lnd.crate_name(), "lnd");
        assert_eq!(Implementation::RustLightning.crate_name(), "rust-lightning");
    }

    #[test]
    fn test_implementation_client_dir_name() {
        assert_eq!(Implementation::BitcoinCore.client_dir_name(), "bitcoin_core_clients");
        assert_eq!(Implementation::CoreLightning.client_dir_name(), "core_lightning_clients");
        assert_eq!(Implementation::Lnd.client_dir_name(), "lnd_clients");
        assert_eq!(Implementation::RustLightning.client_dir_name(), "rust_lightning_clients");
    }

    #[test]
    fn test_implementation_transport_protocol() {
        assert_eq!(Implementation::BitcoinCore.transport_protocol(), "http");
        assert_eq!(Implementation::CoreLightning.transport_protocol(), "unix");
        assert_eq!(Implementation::Lnd.transport_protocol(), "http");
        assert_eq!(Implementation::RustLightning.transport_protocol(), "http");
    }

    #[test]
    fn test_implementation_executable_name() {
        assert_eq!(Implementation::BitcoinCore.executable_name(), "bitcoind");
        assert_eq!(Implementation::CoreLightning.executable_name(), "lightningd");
        assert_eq!(Implementation::Lnd.executable_name(), "lnd");
        assert_eq!(Implementation::RustLightning.executable_name(), "lightning");
    }

    #[test]
    fn test_implementation_test_client_prefix() {
        assert_eq!(Implementation::BitcoinCore.test_client_prefix(), "BitcoinTestClient");
        assert_eq!(Implementation::CoreLightning.test_client_prefix(), "CoreLightningTestClient");
        assert_eq!(Implementation::Lnd.test_client_prefix(), "LndTestClient");
        assert_eq!(Implementation::RustLightning.test_client_prefix(), "RustLightningTestClient");
    }

    #[test]
    fn test_implementation_node_manager_name() {
        assert_eq!(Implementation::BitcoinCore.node_manager_name(), "BitcoinNodeManager");
        assert_eq!(Implementation::CoreLightning.node_manager_name(), "CoreLightningNodeManager");
        assert_eq!(Implementation::Lnd.node_manager_name(), "LndNodeManager");
        assert_eq!(Implementation::RustLightning.node_manager_name(), "RustLightningNodeManager");
    }

    #[test]
    fn test_implementation_client_prefix() {
        assert_eq!(Implementation::BitcoinCore.client_prefix(), "BitcoinClient");
        assert_eq!(Implementation::CoreLightning.client_prefix(), "CoreLightningClient");
        assert_eq!(Implementation::Lnd.client_prefix(), "LndClient");
        assert_eq!(Implementation::RustLightning.client_prefix(), "RustLightningClient");
    }

    #[test]
    fn test_implementation_example_method() {
        assert_eq!(Implementation::BitcoinCore.example_method(), "getblockchaininfo");
        assert_eq!(Implementation::CoreLightning.example_method(), "getinfo");
        assert_eq!(Implementation::Lnd.example_method(), "getinfo");
        assert_eq!(Implementation::RustLightning.example_method(), "getinfo");
    }

    #[test]
    fn test_implementation_example_description() {
        assert_eq!(Implementation::BitcoinCore.example_description(), "Blockchain info");
        assert_eq!(Implementation::CoreLightning.example_description(), "Node info");
        assert_eq!(Implementation::Lnd.example_description(), "Node info");
        assert_eq!(Implementation::RustLightning.example_description(), "Node info");
    }

    #[test]
    fn test_implementation_node_metadata() {
        let bitcoin_core_meta = Implementation::BitcoinCore.node_metadata();
        assert_eq!(bitcoin_core_meta.executable, "bitcoind");
        assert_eq!(bitcoin_core_meta.transport, "http");
        assert!(bitcoin_core_meta.requires_auth);
        assert_eq!(bitcoin_core_meta.readiness_method, "getnetworkinfo");
        assert_eq!(bitcoin_core_meta.initialization_error_codes, vec![-28, -4]);
        assert!(bitcoin_core_meta.socket_path_pattern.is_none());
        assert_eq!(
            bitcoin_core_meta.cli_args.value_args.get("chain"),
            Some(&"-chain={}".to_string())
        );
        assert!(bitcoin_core_meta.cli_args.static_args.contains(&"-listen=0".to_string()));

        let core_lightning_meta = Implementation::CoreLightning.node_metadata();
        assert_eq!(core_lightning_meta.executable, "lightningd");
        assert_eq!(core_lightning_meta.transport, "unix");
        assert!(!core_lightning_meta.requires_auth);
        assert_eq!(core_lightning_meta.readiness_method, "getinfo");
        assert!(core_lightning_meta.initialization_error_codes.is_empty());
        assert_eq!(
            core_lightning_meta.socket_path_pattern.as_deref(),
            Some("{datadir}/regtest/lightning-rpc")
        );
        assert_eq!(
            core_lightning_meta.cli_args.value_args.get("network"),
            Some(&"--network={}".to_string())
        );
        assert!(core_lightning_meta.cli_args.static_args.contains(&"--daemon".to_string()));

        let lnd_meta = Implementation::Lnd.node_metadata();
        assert_eq!(lnd_meta.executable, "lnd");
        assert_eq!(lnd_meta.transport, "http");
        assert!(!lnd_meta.requires_auth);
        assert_eq!(lnd_meta.readiness_method, "getinfo");
        assert!(lnd_meta.initialization_error_codes.is_empty());
        assert!(lnd_meta.socket_path_pattern.is_none());
        assert_eq!(lnd_meta.cli_args.value_args.get("datadir"), Some(&"--datadir={}".to_string()));
        assert!(lnd_meta.cli_args.static_args.contains(&"--regtest".to_string()));

        let rust_lightning_meta = Implementation::RustLightning.node_metadata();
        assert_eq!(rust_lightning_meta.executable, "lightning");
        assert_eq!(rust_lightning_meta.transport, "http");
        assert!(!rust_lightning_meta.requires_auth);
        assert_eq!(rust_lightning_meta.readiness_method, "getinfo");
        assert!(rust_lightning_meta.initialization_error_codes.is_empty());
        assert!(rust_lightning_meta.socket_path_pattern.is_none());
        assert_eq!(
            rust_lightning_meta.cli_args.value_args.get("network"),
            Some(&"--network={}".to_string())
        );
        assert!(rust_lightning_meta.cli_args.static_args.contains(&"--regtest".to_string()));
    }

    #[test]
    fn test_implementation_create_type_adapter() {
        let bitcoin_core_adapter = Implementation::BitcoinCore.create_type_adapter();
        assert!(bitcoin_core_adapter.is_ok());

        let core_lightning_adapter = Implementation::CoreLightning.create_type_adapter();
        assert!(core_lightning_adapter.is_ok());

        let lnd_adapter = Implementation::Lnd.create_type_adapter();
        assert!(lnd_adapter.is_err());
    }
}
