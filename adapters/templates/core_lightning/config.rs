//! Configuration interface for Core Lightning RPC clients

/// Configuration for Core Lightning RPC client
#[derive(Debug, Clone)]
pub struct Config {
    /// The RPC URL endpoint for the Core Lightning daemon
    pub rpc_url: String,
    /// Username for RPC authentication
    pub rpc_user: String,
    /// Password for RPC authentication
    pub rpc_password: String,
}
