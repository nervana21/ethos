//! Second oracle channel: wire JSON → generated Raw response types (`ethos-bitcoind`).
//!
//! Returns:
//! - `None` — no Raw type mapped for this method (skip channel)
//! - `Some(Ok(()))` — serde decode succeeded
//! - `Some(Err(detail))` — serde decode failed (**DecodeFail** after IR ok)

use ethos_bitcoind::{
    DecodeRawTransactionResponse, EstimateSmartFeeResponse, GetAddrManInfoResponse,
    GetBestBlockHashResponse, GetBlockCountResponse, GetBlockHashResponse, GetBlockHeaderResponse,
    GetBlockResponse, GetBlockchainInfoResponse, GetConnectionCountResponse,
    GetDescriptorInfoResponse, GetDifficultyResponse, GetMempoolInfoResponse,
    GetMiningInfoResponse, GetNetworkHashPsResponse, GetNetworkInfoResponse, GetPeerInfoResponse,
    GetRawMempoolResponse, GetRpcInfoResponse, GetTxOutResponse, HelpResponse, ListBannedResponse,
    UptimeResponse, ValidateAddressResponse,
};
use serde::de::DeserializeOwned;
use serde_json::Value;

fn try_from_value<T: DeserializeOwned>(value: &Value) -> Result<(), String> {
    serde_json::from_value::<T>(value.clone()).map(|_| ()).map_err(|e| e.to_string())
}

/// Attempt Raw serde decode for `method`. See module docs for `Option`/`Result` meaning.
pub fn try_raw_decode(method: &str, value: &Value) -> Option<Result<(), String>> {
    Some(match method {
        "decoderawtransaction" => try_from_value::<DecodeRawTransactionResponse>(value),
        "estimatesmartfee" => try_from_value::<EstimateSmartFeeResponse>(value),
        "getaddrmaninfo" => try_from_value::<GetAddrManInfoResponse>(value),
        "getbestblockhash" => try_from_value::<GetBestBlockHashResponse>(value),
        "getblock" => try_from_value::<GetBlockResponse>(value),
        "getblockchaininfo" => try_from_value::<GetBlockchainInfoResponse>(value),
        "getblockcount" => try_from_value::<GetBlockCountResponse>(value),
        "getblockhash" => try_from_value::<GetBlockHashResponse>(value),
        "getblockheader" => try_from_value::<GetBlockHeaderResponse>(value),
        "getconnectioncount" => try_from_value::<GetConnectionCountResponse>(value),
        "getdescriptorinfo" => try_from_value::<GetDescriptorInfoResponse>(value),
        "getdifficulty" => try_from_value::<GetDifficultyResponse>(value),
        "getmempoolinfo" => try_from_value::<GetMempoolInfoResponse>(value),
        "getmininginfo" => try_from_value::<GetMiningInfoResponse>(value),
        "getnetworkhashps" => try_from_value::<GetNetworkHashPsResponse>(value),
        "getnetworkinfo" => try_from_value::<GetNetworkInfoResponse>(value),
        "getpeerinfo" => try_from_value::<GetPeerInfoResponse>(value),
        "getrawmempool" => try_from_value::<GetRawMempoolResponse>(value),
        "getrpcinfo" => try_from_value::<GetRpcInfoResponse>(value),
        "gettxout" => try_from_value::<Option<GetTxOutResponse>>(value),
        "help" => try_from_value::<HelpResponse>(value),
        "listbanned" => try_from_value::<ListBannedResponse>(value),
        "uptime" => try_from_value::<UptimeResponse>(value),
        "validateaddress" => try_from_value::<ValidateAddressResponse>(value),
        _ => return None,
    })
}

/// True when [`try_raw_decode`] has a mapped Raw type for `method`.
pub fn raw_decode_mapped(method: &str) -> bool { try_raw_decode(method, &Value::Null).is_some() }

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn mapped_covers_allowlist_core() {
        for m in [
            "getblock",
            "getblockheader",
            "getblockchaininfo",
            "getblockcount",
            "getbestblockhash",
            "uptime",
            "help",
            "validateaddress",
            "decoderawtransaction",
        ] {
            assert!(raw_decode_mapped(m), "{m} should be mapped");
        }
        assert!(!raw_decode_mapped("abandontransaction"));
    }

    #[test]
    fn decode_ok_primitives_and_getblock_hex() {
        assert_eq!(try_raw_decode("getblockcount", &json!(0)), Some(Ok(())));
        assert_eq!(
            try_raw_decode(
                "getbestblockhash",
                &json!("0000000000000000000000000000000000000000000000000000000000000000")
            ),
            Some(Ok(()))
        );
        assert_eq!(try_raw_decode("getblock", &json!("00")), Some(Ok(())), "verbosity0 hex arm");
    }

    #[test]
    fn decode_fail_wrong_shape() {
        let r = try_raw_decode("getblockcount", &json!({"not": "a number"}));
        assert!(matches!(r, Some(Err(_))));
    }
}
