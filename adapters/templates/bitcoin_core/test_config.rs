//! Test configuration for Bitcoin RPC testing
//!
//! This module provides configuration utilities for running Bitcoin nodes in test environments.

use std::env;
use std::fmt;
use std::path::PathBuf;

use bitcoin::Network;
use crate::config::Config;

const DEFAULT_EXTRA_ARGS: [&str; 2] = ["-prune=0", "-txindex"];

/// Error returned when the configured network is not supported for node startup.
#[derive(Debug)]
pub struct UnsupportedNetwork;

impl std::fmt::Display for UnsupportedNetwork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unsupported network")
    }
}

impl std::error::Error for UnsupportedNetwork {}

/// TestConfig represents the configuration needed to run a Bitcoin node in a test environment.
/// This struct encapsulates test‑node settings: network, RPC port, username, password, and extra args.
/// Defaults are:
/// - `network = Network::Regtest`
/// - `rpc_port = 0` (auto‑select a free port)
/// - `rpc_username = "rpcuser"`
/// - `rpc_password = "rpcpassword"`
/// - `bitcoind_path = None` (use executable from PATH)
/// - `extra_args = ["-prune=0", "-txindex"]` (for full blockchain history and transaction lookup)
///
/// To override any of these, simply modify fields on `TestConfig::default()`
/// (or assign directly in code). If you prefer not to recompile for every change,
/// consider using `TestConfig::from_env()` to read overrides from environment variables.
///
/// # Examples
///
/// ```rust,ignore
/// let mut cfg = TestConfig::default();
/// cfg.network = Network::Testnet;
/// cfg.rpc_port = 18545;
/// cfg.rpc_username = "alice".into();
/// ```
///
/// # Environment Overrides
///
/// Reads `RPC_NETWORK`, `RPC_PORT`, `RPC_USER`, and `RPC_PASS`, and `BITCOIND_PATH` (path to bitcoind executable) to override defaults.
#[derive(Clone)]
pub struct TestConfig {
    /// Which Bitcoin network to run against.
    pub network: Network,
    /// The port number for RPC communication with the Bitcoin node.
    /// A value of 0 indicates that an available port should be automatically selected.
    pub rpc_port: u16,
    /// The username for RPC authentication.
    /// Can be customized to match your `bitcoin.conf` `rpcuser` setting.
    pub rpc_username: String,
    /// The password for RPC authentication.
    /// Can be customized to match your `bitcoin.conf` `rpcpassword` setting.
    pub rpc_password: String,
    /// Path to the bitcoind executable. If None, the default executable name is used (e.g. from PATH).
    pub bitcoind_path: Option<PathBuf>,
    /// Extra command-line arguments to pass to bitcoind
    pub extra_args: Vec<String>,
}

impl fmt::Debug for TestConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TestConfig")
            .field("network", &self.network)
            .field("rpc_port", &self.rpc_port)
            .field("rpc_username", &"[redacted]")
            .field("rpc_password", &"[redacted]")
            .field("bitcoind_path", &self.bitcoind_path)
            .field("extra_args", &self.extra_args)
            .finish()
    }
}

impl TestConfig {
    /// Return the value used with `-chain=<value>` for the configured network.
    /// Returns `Err(UnsupportedNetwork)` if the network variant is not supported for node startup.
    pub fn as_chain_str(&self) -> Result<&'static str, UnsupportedNetwork> {
        #[allow(unreachable_patterns)]
        match self.network {
            Network::Bitcoin => Ok("main"),
            Network::Regtest => Ok("regtest"),
            Network::Signet => Ok("signet"),
            Network::Testnet => Ok("testnet"),
            Network::Testnet4 => Ok("testnet4"),
            _ => Err(UnsupportedNetwork),
        }
    }

    /// Parse network from common strings (case-insensitive). Accepts: regtest, testnet|test,
    /// signet, mainnet|main|bitcoin, testnet4.
    pub fn network_from_str(s: &str) -> Option<Network> {
        match s.to_ascii_lowercase().as_str() {
            "regtest" => Some(Network::Regtest),
            "testnet" | "test" => Some(Network::Testnet),
            "signet" => Some(Network::Signet),
            "mainnet" | "main" | "bitcoin" => Some(Network::Bitcoin),
            "testnet4" => Some(Network::Testnet4),
            _ => None,
        }
    }

    /// Create a `TestConfig`, overriding defaults with environment variables:
    /// - `RPC_NETWORK`: overrides `network`; one of `regtest`, `testnet|test`, `signet`, `mainnet|main|bitcoin`, `testnet4`
    /// - `RPC_PORT`: overrides `rpc_port`
    /// - `RPC_USER`: overrides `rpc_username`
    /// - `RPC_PASS`: overrides `rpc_password`
    /// - `BITCOIND_PATH`: overrides `bitcoind_path` (path to the bitcoind executable)
    #[allow(clippy::field_reassign_with_default)]
    pub fn from_env() -> Self {
        let mut cfg = Self::default();
        if let Ok(net) = env::var("RPC_NETWORK") {
            if let Some(n) = Self::network_from_str(&net) {
                cfg.network = n;
            }
        }
        if let Ok(port_str) = env::var("RPC_PORT") {
            if let Ok(port) = port_str.parse() {
                cfg.rpc_port = port;
            }
        }
        if let Ok(user) = env::var("RPC_USER") {
            cfg.rpc_username = user;
        }
        if let Ok(pass) = env::var("RPC_PASS") {
            cfg.rpc_password = pass;
        }
        if let Ok(path) = env::var("BITCOIND_PATH") {
            cfg.bitcoind_path = Some(PathBuf::from(path));
        }
        cfg
    }

    /// Convert this test configuration into a full Config instance
    pub fn into_config(self) -> Config {
        Config {
            rpc_url: format!("http://127.0.0.1:{}", self.rpc_port),
            rpc_user: self.rpc_username,
            rpc_password: self.rpc_password,
        }
    }

    /// Create a TestConfig from a full Config instance
    pub fn from_config(config: &Config) -> Self {
        // Extract port from URL, defaulting to 0 if parsing fails
        let rpc_port = config.rpc_url
            .split(':')
            .next_back()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        Self {
            network: Network::Regtest, // Default to regtest for test environments
            rpc_port,
            rpc_username: config.rpc_user.clone(),
            rpc_password: config.rpc_password.clone(),
            bitcoind_path: None,
            extra_args: DEFAULT_EXTRA_ARGS.map(String::from).to_vec(),
        }
    }
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            network: Network::Regtest,
            rpc_port: 0,
            rpc_username: "rpcuser".to_string(),
            rpc_password: "rpcpassword".to_string(),
            bitcoind_path: None,
            extra_args: DEFAULT_EXTRA_ARGS.map(String::from).to_vec(),
        }
    }
}
