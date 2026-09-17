// SPDX-License-Identifier: CC0-1.0

//! Serde **decode** goldens: real JSON → generated Raw structs.
//!
//! Inventory: `ethos_codegen::generators::raw_response_policy::RPC_DECODE_GOLDEN_FIXTURES`.
//! Override-linked fixtures must also appear there (enforced in `ethos-codegen` policy tests).

use ethos_codegen::generators::raw_response_policy::RPC_DECODE_GOLDEN_FIXTURES;
use ethos_codegen_golden_decode::{
    AnalyzePsbtResponse, DecodePsbtResponse, DecodeRawTransactionResponse,
    EstimateSmartFeeResponse, FinalizePsbtResponse, GetAddrManInfoResponse,
    GetBestBlockHashResponse, GetBlockCountResponse, GetBlockHeaderResponse, GetBlockResponse,
    GetBlockTemplateResponse, GetBlockchainInfoResponse, GetChainTipsResponse,
    GetConnectionCountResponse, GetDeploymentInfoResponse, GetDifficultyResponse,
    GetIndexInfoResponse, GetMempoolInfoResponse, GetMiningInfoResponse, GetNetTotalsResponse,
    GetNetworkHashPsResponse, GetNetworkInfoResponse, GetPeerInfoResponse, GetRawMempoolResponse,
    GetRawTransactionResponse, GetTxOutResponse, ListDescriptorsResponse, UptimeResponse,
    ValidateAddressResponse,
};

fn fixture(name: &str) -> String {
    let path = ethos_path::rpc_golden_path(
        &ethos_path::workspace_root_from_manifest(env!("CARGO_MANIFEST_DIR"), 2),
        name,
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Decodes every fixture in [`RPC_DECODE_GOLDEN_FIXTURES`].
#[test]
fn golden_decode_all_inventory_fixtures() {
    for (rpc, filename) in RPC_DECODE_GOLDEN_FIXTURES {
        let raw = fixture(filename);
        let ctx = format!("rpc={rpc} file={filename}");
        match (*rpc, *filename) {
            ("analyzepsbt", "analyzepsbt_result_min.json") => {
                let _: AnalyzePsbtResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("decodepsbt", "decodepsbt_result_min.json") => {
                let _: DecodePsbtResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("decoderawtransaction", "decoderawtransaction_result_min.json") => {
                let _: DecodeRawTransactionResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("estimatesmartfee", "estimatesmartfee_result_min.json") => {
                let _: EstimateSmartFeeResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("finalizepsbt", "finalizepsbt_result_min.json") => {
                let _: FinalizePsbtResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("getaddrmaninfo", "getaddrmaninfo_min.json") => {
                let _: GetAddrManInfoResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("getbestblockhash", "getbestblockhash_result_min.json") => {
                let _: GetBestBlockHashResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("getblock", "getblock_hex_min.json") => {
                let _: GetBlockResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("getblock", "getblock_verbosity1_min.json") => {
                let _: GetBlockResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("getblockchaininfo", "getblockchaininfo_result_min.json") => {
                let _: GetBlockchainInfoResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("getblockcount", "getblockcount_result_min.json") => {
                let _: GetBlockCountResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("getblockheader", "getblockheader_hex_min.json") => {
                let _: GetBlockHeaderResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("getblockheader", "getblockheader_verbose_min.json") => {
                let _: GetBlockHeaderResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("getblocktemplate", "getblocktemplate_verbose_min.json") => {
                let _: GetBlockTemplateResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("getchaintips", "getchaintips_result_min.json") => {
                let _: GetChainTipsResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("getconnectioncount", "getconnectioncount_result_min.json") => {
                let _: GetConnectionCountResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("getdeploymentinfo", "getdeploymentinfo_min.json") => {
                let _: GetDeploymentInfoResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("getdifficulty", "getdifficulty_result_min.json") => {
                let _: GetDifficultyResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("getindexinfo", "getindexinfo_result_min.json") => {
                let _: GetIndexInfoResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("getmempoolinfo", "getmempoolinfo_min.json") => {
                let _: GetMempoolInfoResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("getmininginfo", "getmininginfo_min.json") => {
                let _: GetMiningInfoResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("getnettotals", "getnettotals_min.json") => {
                let _: GetNetTotalsResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("getnetworkhashps", "getnetworkhashps_result_min.json") => {
                let _: GetNetworkHashPsResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("getnetworkinfo", "getnetworkinfo_min.json") => {
                let _: GetNetworkInfoResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("getpeerinfo", "getpeerinfo_result_min.json") => {
                let _: GetPeerInfoResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("getrawmempool", "getrawmempool_txids_min.json") => {
                let _: GetRawMempoolResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("getrawtransaction", "getrawtransaction_hex_min.json") => {
                let _: GetRawTransactionResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("getrawtransaction", "getrawtransaction_verbose_min.json") => {
                let _: GetRawTransactionResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("gettxout", "gettxout_min.json") => {
                let _: GetTxOutResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("listdescriptors", "listdescriptors_result_min.json") => {
                let _: ListDescriptorsResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("uptime", "uptime_result_min.json") => {
                let _: UptimeResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("validateaddress", "validateaddress_result_min.json") => {
                let _: ValidateAddressResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            _ => panic!("{ctx}: add a match arm in golden_decode_all_inventory_fixtures"),
        }
    }
}
