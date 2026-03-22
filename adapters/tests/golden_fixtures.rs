// SPDX-License-Identifier: CC0-1.0

//! Golden JSON samples vs canonical [`ir::TypeDef`] trees from `bitcoin.ir.json`.

use std::path::PathBuf;

use ir::json_golden::validate_json_matches_type;
use ir::{ProtocolDef, ProtocolIR, TypeDef};

fn project_root() -> PathBuf {
    // Integration tests run with CARGO_MANIFEST_DIR = the `adapters/` crate root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn load_bitcoin_ir() -> ProtocolIR {
    let path = project_root().join("resources/ir/bitcoin.ir.json");
    ProtocolIR::from_file(&path).unwrap_or_else(|e| panic!("load {}: {e}", path.display()))
}

fn rpc_result<'a>(ir: &'a ProtocolIR, name: &str) -> &'a TypeDef {
    for module in ir.modules() {
        for def in module.definitions() {
            if let ProtocolDef::RpcMethod(rpc) = def {
                if rpc.name == name {
                    return rpc
                        .result
                        .as_ref()
                        .unwrap_or_else(|| panic!("RPC {name} missing or has no result"));
                }
            }
        }
    }
    panic!("RPC {name} missing or has no result");
}

#[test]
fn golden_analyzepsbt_matches_ir() {
    let ir = load_bitcoin_ir();
    let ty = rpc_result(&ir, "analyzepsbt");
    let path = project_root().join("resources/testdata/rpc_golden/analyzepsbt_result_min.json");
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let value: serde_json::Value = serde_json::from_str(&raw).expect("json");
    validate_json_matches_type(ty, &value).unwrap();
}

#[test]
fn golden_listdescriptors_matches_ir() {
    let ir = load_bitcoin_ir();
    let ty = rpc_result(&ir, "listdescriptors");
    let path = project_root().join("resources/testdata/rpc_golden/listdescriptors_result_min.json");
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let value: serde_json::Value = serde_json::from_str(&raw).expect("json");
    validate_json_matches_type(ty, &value).unwrap();
}

#[test]
fn golden_decodepsbt_matches_ir() {
    let ir = load_bitcoin_ir();
    let ty = rpc_result(&ir, "decodepsbt");
    let path = project_root().join("resources/testdata/rpc_golden/decodepsbt_result_min.json");
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let value: serde_json::Value = serde_json::from_str(&raw).expect("json");
    validate_json_matches_type(ty, &value).unwrap();
}

#[test]
fn golden_decoderawtransaction_matches_ir() {
    let ir = load_bitcoin_ir();
    let ty = rpc_result(&ir, "decoderawtransaction");
    let path =
        project_root().join("resources/testdata/rpc_golden/decoderawtransaction_result_min.json");
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let value: serde_json::Value = serde_json::from_str(&raw).expect("json");
    validate_json_matches_type(ty, &value).unwrap();
}

#[test]
fn golden_estimatesmartfee_matches_ir() {
    let ir = load_bitcoin_ir();
    let ty = rpc_result(&ir, "estimatesmartfee");
    let path =
        project_root().join("resources/testdata/rpc_golden/estimatesmartfee_result_min.json");
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let value: serde_json::Value = serde_json::from_str(&raw).expect("json");
    validate_json_matches_type(ty, &value).unwrap();
}

#[test]
fn golden_finalizepsbt_matches_ir() {
    let ir = load_bitcoin_ir();
    let ty = rpc_result(&ir, "finalizepsbt");
    let path = project_root().join("resources/testdata/rpc_golden/finalizepsbt_result_min.json");
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let value: serde_json::Value = serde_json::from_str(&raw).expect("json");
    validate_json_matches_type(ty, &value).unwrap();
}

#[test]
fn golden_getblockchaininfo_matches_ir() {
    let ir = load_bitcoin_ir();
    let ty = rpc_result(&ir, "getblockchaininfo");
    let path =
        project_root().join("resources/testdata/rpc_golden/getblockchaininfo_result_min.json");
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let value: serde_json::Value = serde_json::from_str(&raw).expect("json");
    validate_json_matches_type(ty, &value).unwrap();
}

#[test]
fn golden_getconnectioncount_matches_ir() {
    let ir = load_bitcoin_ir();
    let ty = rpc_result(&ir, "getconnectioncount");
    let path =
        project_root().join("resources/testdata/rpc_golden/getconnectioncount_result_min.json");
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let value: serde_json::Value = serde_json::from_str(&raw).expect("json");
    validate_json_matches_type(ty, &value).unwrap();
}

#[test]
fn golden_getdifficulty_matches_ir() {
    let ir = load_bitcoin_ir();
    let ty = rpc_result(&ir, "getdifficulty");
    let path = project_root().join("resources/testdata/rpc_golden/getdifficulty_result_min.json");
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let value: serde_json::Value = serde_json::from_str(&raw).expect("json");
    validate_json_matches_type(ty, &value).unwrap();
}

#[test]
fn golden_validateaddress_matches_ir() {
    let ir = load_bitcoin_ir();
    let ty = rpc_result(&ir, "validateaddress");
    let path = project_root().join("resources/testdata/rpc_golden/validateaddress_result_min.json");
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let value: serde_json::Value = serde_json::from_str(&raw).expect("json");
    validate_json_matches_type(ty, &value).unwrap();
}
