// SPDX-License-Identifier: CC0-1.0

//! Golden JSON samples vs canonical [`ir::TypeDef`] trees from `bitcoin.ir.json`.

use ir::json_golden::validate_json_matches_type;
use ir::{ProtocolDef, ProtocolIR, TypeDef};
use path::{canonical_bitcoin_ir_path, rpc_golden_path, workspace_root_from_manifest};

const GOLDEN_FIXTURES: &[(&str, &str)] = &[
    ("analyzepsbt", "analyzepsbt_result_min.json"),
    ("listdescriptors", "listdescriptors_result_min.json"),
    ("decodepsbt", "decodepsbt_result_min.json"),
    ("decoderawtransaction", "decoderawtransaction_result_min.json"),
    ("estimatesmartfee", "estimatesmartfee_result_min.json"),
    ("finalizepsbt", "finalizepsbt_result_min.json"),
    ("getblockchaininfo", "getblockchaininfo_result_min.json"),
    ("getconnectioncount", "getconnectioncount_result_min.json"),
    ("getdifficulty", "getdifficulty_result_min.json"),
    ("validateaddress", "validateaddress_result_min.json"),
];

fn project_root() -> std::path::PathBuf {
    workspace_root_from_manifest(env!("CARGO_MANIFEST_DIR"), 1)
}

fn load_bitcoin_ir() -> ProtocolIR {
    let path = canonical_bitcoin_ir_path(&project_root());
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
fn golden_fixtures_match_ir() {
    let ir = load_bitcoin_ir();
    let root = project_root();
    for &(rpc, filename) in GOLDEN_FIXTURES {
        let ty = rpc_result(&ir, rpc);
        let path = rpc_golden_path(&root, filename);
        let raw =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let value: serde_json::Value = serde_json::from_str(&raw).expect("json");
        validate_json_matches_type(ty, &value)
            .unwrap_or_else(|e| panic!("{rpc} / {filename}: {e}"));
    }
}
