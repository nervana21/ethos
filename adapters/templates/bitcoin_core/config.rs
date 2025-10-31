//! Configuration interface for Bitcoin RPC clients

#[derive(Debug, Clone)]
pub struct Config {
    /// The RPC URL endpoint for the Bitcoin Core daemon
    pub rpc_url: String,
    /// Username for RPC authentication
    pub rpc_user: String,
    /// Password for RPC authentication
    pub rpc_password: String,
}
