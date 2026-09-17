//! Integration: schema oracle against pinned IR + golden fixtures.

use ethos_analysis::{
    classify, default_allowlist, find_rpc, generate_params, pick_rpc, resolve_disc_arm,
    run_oracle_case, summarize, InvokeError, OracleClass, RpcInvoker,
};
use ir::ProtocolIR;
use path::{canonical_bitcoin_ir_path, rpc_golden_path, workspace_root_from_manifest};
use serde_json::{json, Value};

fn repo_root() -> std::path::PathBuf { workspace_root_from_manifest(env!("CARGO_MANIFEST_DIR"), 2) }

fn load_ir() -> ProtocolIR {
    let path = canonical_bitcoin_ir_path(&repo_root());
    ProtocolIR::from_file(&path).unwrap_or_else(|e| panic!("load IR {}: {e}", path.display()))
}

fn load_golden(name: &str) -> Value {
    let path = rpc_golden_path(&repo_root(), name);
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read golden {}: {e}", path.display()));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("parse golden {}: {e}", path.display()))
}

struct FixtureInvoker {
    by_method: std::collections::HashMap<String, Value>,
}

impl RpcInvoker for FixtureInvoker {
    fn invoke(&mut self, method: &str, _params: &[Value]) -> Result<Value, InvokeError> {
        self.by_method.get(method).cloned().ok_or_else(|| InvokeError::Rpc {
            code: Some(-32601),
            message: format!("Unknown method: {method}"),
        })
    }
}

#[test]
fn golden_getdifficulty_matches_ir() {
    let ir = load_ir();
    let rpc = find_rpc(&ir, "getdifficulty").expect("getdifficulty in IR");
    let golden = load_golden("getdifficulty_result_min.json");
    let class = classify(rpc, &[], Ok(golden), None);
    assert_eq!(class, OracleClass::Ok, "golden must match IR result");
}

#[test]
fn golden_getblockchaininfo_matches_ir() {
    let ir = load_ir();
    let rpc = find_rpc(&ir, "getblockchaininfo").expect("getblockchaininfo in IR");
    let golden = load_golden("getblockchaininfo_result_min.json");
    let class = classify(rpc, &[], Ok(golden), None);
    assert_eq!(class, OracleClass::Ok, "golden must match IR result");
}

#[test]
fn injected_wrong_shape_is_schema_mismatch() {
    let ir = load_ir();
    let rpc = find_rpc(&ir, "getdifficulty").expect("getdifficulty in IR");
    let class = classify(rpc, &[], Ok(json!({"nope": true})), None);
    assert!(
        matches!(class, OracleClass::SchemaMismatch { .. }),
        "wrong shape must be schema_mismatch, got {class:?}"
    );
}

#[test]
fn fixture_invoker_end_to_end_ok() {
    let ir = load_ir();
    let mut inv = FixtureInvoker {
        by_method: [(
            "getconnectioncount".to_string(),
            load_golden("getconnectioncount_result_min.json"),
        )]
        .into_iter()
        .collect(),
    };
    let rpc = find_rpc(&ir, "getconnectioncount").expect("getconnectioncount");
    let report = run_oracle_case(rpc, b"abc", &mut inv, None);
    assert_eq!(report.class, OracleClass::Ok);
}

#[test]
fn allowlist_pick_and_generate_smoke() {
    let ir = load_ir();
    let rpc = pick_rpc(&ir, b"\x03\x04\x05", default_allowlist()).expect("allowlist hit");
    let params = generate_params(rpc, b"\x10\x11\x12\x13");
    // Allowlisted methods are mostly zero-arg; generation must not panic.
    assert!(params.len() <= rpc.params.len());
}

#[test]
fn summarize_counts_findings() {
    let reports = vec![
        ethos_analysis::OracleReport { method: "a".into(), params: vec![], class: OracleClass::Ok },
        ethos_analysis::OracleReport {
            method: "b".into(),
            params: vec![],
            class: OracleClass::SchemaMismatch { detail: "x".into() },
        },
        ethos_analysis::OracleReport {
            method: "c".into(),
            params: vec![],
            class: OracleClass::ExpectedReject { code: Some(-8), message: "y".into() },
        },
    ];
    let s = summarize(&reports);
    assert_eq!(s.get("ok"), Some(&1));
    assert_eq!(s.get("schema_mismatch"), Some(&1));
    assert_eq!(s.get("expected_reject"), Some(&1));
}

#[test]
fn getblock_disc_arm_hex_ok_object_mismatch() {
    let ir = load_ir();
    let rpc = find_rpc(&ir, "getblock").expect("getblock");
    let params_v0 = vec![json!("00".repeat(32)), json!(0)];
    let Some(arm) = resolve_disc_arm(rpc, &params_v0) else {
        // Stock Core OpenRPC has no x-bitcoin-discriminated-result on getblock.
        assert!(rpc.result_discriminator.is_none(), "unstamped getblock must not carry a disc");
        return;
    };
    assert_eq!(arm.protocol_type.as_deref(), Some("string"));

    let class_ok = classify(rpc, &params_v0, Ok(json!("00")), None);
    assert_eq!(class_ok, OracleClass::Ok, "hex body must match verbosity0 arm");

    let class_bad = classify(rpc, &params_v0, Ok(json!({"hash": "x"})), None);
    assert!(
        matches!(class_bad, OracleClass::SchemaMismatch { .. }),
        "object body must fail verbosity0 arm, got {class_bad:?}"
    );
}

#[test]
fn getblock_omitted_verbosity_defaults_to_arm1() {
    let ir = load_ir();
    let rpc = find_rpc(&ir, "getblock").expect("getblock");
    // Only blockhash — Core default verbosity=1.
    let params = vec![json!("00".repeat(32))];
    let Some(arm) = resolve_disc_arm(rpc, &params) else {
        assert!(rpc.result_discriminator.is_none(), "unstamped getblock must not carry a disc");
        return;
    };
    assert_eq!(arm.kind, ir::TypeKind::Object, "default verbosity 1 = object arm");
}

#[test]
fn getblockheader_verbose_false_is_hex_arm() {
    let ir = load_ir();
    let rpc = find_rpc(&ir, "getblockheader").expect("getblockheader");
    let params = vec![json!("00".repeat(32)), json!(false)];
    let Some(arm) = resolve_disc_arm(rpc, &params) else {
        assert!(
            rpc.result_discriminator.is_none(),
            "unstamped getblockheader must not carry a disc"
        );
        return;
    };
    assert_eq!(arm.protocol_type.as_deref(), Some("string"));
    let class = classify(rpc, &params, Ok(json!("00ab")), None);
    assert_eq!(class, OracleClass::Ok);
}
