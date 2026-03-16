//! Configuration interface for Bitcoin RPC clients

use std::fmt;

#[derive(Clone)]
pub struct Config {
    /// The RPC URL endpoint for the Bitcoin Core daemon
    pub rpc_url: String,
    /// Username for RPC authentication
    pub rpc_user: String,
    /// Password for RPC authentication
    pub rpc_password: String,
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("rpc_url", &self.rpc_url)
            .field("rpc_user", &"[redacted]")
            .field("rpc_password", &"[redacted]")
            .finish()
    }
}
