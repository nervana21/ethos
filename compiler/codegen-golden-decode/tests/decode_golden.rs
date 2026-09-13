// SPDX-License-Identifier: CC0-1.0

//! Serde **decode** goldens: real JSON → generated Raw structs.
//!
//! Inventory: `ethos_codegen::generators::raw_response_policy::RPC_DECODE_GOLDEN_FIXTURES`.
//! Override-linked fixtures must also appear there (enforced in `ethos-codegen` policy tests).

use ethos_codegen::generators::raw_response_policy::RPC_DECODE_GOLDEN_FIXTURES;
use ethos_codegen_golden_decode::{
    AnalyzePsbtResponse, DecodePsbtResponse, DecodeRawTransactionResponse,
    EstimateSmartFeeResponse, FinalizePsbtResponse, GetBlockResponse, GetBlockTemplateResponse,
    GetBlockchainInfoResponse, GetConnectionCountResponse, GetDifficultyResponse,
    GetRawTransactionResponse, ListDescriptorsResponse, ValidateAddressResponse,
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
            ("getblock", "getblock_hex_min.json") => {
                let _: GetBlockResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("getblockchaininfo", "getblockchaininfo_result_min.json") => {
                let _: GetBlockchainInfoResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("getblocktemplate", "getblocktemplate_verbose_min.json") => {
                let _: GetBlockTemplateResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("getconnectioncount", "getconnectioncount_result_min.json") => {
                let _: GetConnectionCountResponse =
                    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{ctx}: {e}"));
            }
            ("getdifficulty", "getdifficulty_result_min.json") => {
                let _: GetDifficultyResponse =
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
            ("listdescriptors", "listdescriptors_result_min.json") => {
                let _: ListDescriptorsResponse =
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
