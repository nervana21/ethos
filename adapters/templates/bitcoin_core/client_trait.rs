// Generated client trait for Bitcoin Core {{VERSION}}

use async_trait::async_trait;
use crate::transport::{TransportTrait, TransportError};
use crate::transport::core::TransportExt;
use serde::de::DeserializeOwned;
{{IMPORTS}}

{{PARAM_STRUCTS}}

#[doc = r#"A versioned client trait for Bitcoin Core {{VERSION}}"#]
#[async_trait]
pub trait BitcoinClient: Send + Sync + TransportTrait + TransportExt + RpcDispatchExt {
    type Error;

{{TRAIT_METHOD_SIGNATURES}}
}

/// Helper to route calls to the node or wallet namespace automatically.
pub trait RpcDispatchExt: TransportTrait + TransportExt {
    /// Dispatch JSON-RPC methods by name.
    fn dispatch_json<R: DeserializeOwned>(
        &self,
        method: &str,
        params: &[serde_json::Value],
    ) -> impl Future<Output = Result<R, TransportError>> + Send {
        async move {
            self.call(method, params).await
        }
    }
}

impl<T: TransportTrait + TransportExt + ?Sized> RpcDispatchExt for T {}

// helper trait, so any TransportTrait gets a wallet_call by default
pub trait WalletTransportExt: TransportTrait + TransportExt {
    fn wallet_call<T: serde::Serialize + std::marker::Sync, R: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: &[T],
    ) -> impl std::future::Future<Output = Result<R, crate::transport::TransportError>> + Send { async {
        // Convert params to Value before passing to call
        let value_params: Vec<serde_json::Value> = params
            .iter()
            .map(|p| serde_json::to_value(p).unwrap())
            .collect();
        self.call(method, &value_params).await
    }}
}

impl<T: TransportTrait + TransportExt + ?Sized> WalletTransportExt for T {}

// Provide default implementation for any type that implements TransportTrait + TransportExt
#[async_trait]
impl<T: TransportTrait + TransportExt + Send + Sync> BitcoinClient for T {
    type Error = TransportError;

{{TRAIT_METHOD_IMPLEMENTATIONS}}
}
