#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]

//! Ethos Fuzz Library
//!
//! Fuzzing utilities, deterministic RNG, and schema-oracle RPC glue.

pub mod corpus_manager;
pub mod deterministic_rng;
pub mod observability;
pub mod schema_oracle_cov;

pub use schema_oracle_cov::{
    fuzz_schema_oracle_cov, run_cov_guided_case, schema_oracle_seed_corpus_dir, CovGuidedInvoker,
    CovGuidedOutcome, ResponseBank,
};

use std::collections::HashMap;
use std::path::Path;

use ethos_adapters::RpcAdapter;
use ethos_analysis::{
    default_allowlist, pick_rpc, run_oracle_case, summarize, DifferentialAnalyzer, InvokeError,
    OracleReport, RpcInvoker,
};
use fuzz_types::{FuzzCase, ProtocolAdapter};
use ir::ProtocolIR;
use serde_json::Value;
use types::Implementation;

/// Load pinned Bitcoin IR.
pub fn load_bitcoin_ir(path: &Path) -> Result<ProtocolIR, String> {
    ProtocolIR::from_file(path).map_err(|e| e.to_string())
}

/// Default path to pinned IR from the ethos repo root.
pub fn default_bitcoin_ir_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources/ir/bitcoin.ir.json")
}

/// Invoker that always returns a fixed result or error (unit / offline fuzz).
pub struct StaticInvoker {
    /// Outcome for every invoke.
    pub outcome: Result<Value, InvokeError>,
}

impl RpcInvoker for StaticInvoker {
    fn invoke(&mut self, _method: &str, _params: &[Value]) -> Result<Value, InvokeError> {
        self.outcome.clone()
    }
}

/// Schema-oracle pass over allowlisted methods using `data` as entropy.
pub fn fuzz_schema_oracle_case(
    ir: &ProtocolIR,
    data: &[u8],
    invoker: &mut dyn RpcInvoker,
) -> Option<OracleReport> {
    let rpc = pick_rpc(ir, data, default_allowlist())?;
    let seed = if data.len() > 1 { &data[1..] } else { data };
    Some(run_oracle_case(rpc, seed, invoker, None))
}

/// Run N deterministic offline cases against a static success value.
pub fn smoke_schema_oracle_offline(
    ir: &ProtocolIR,
    response: Value,
    rounds: usize,
) -> Vec<OracleReport> {
    let mut inv = StaticInvoker { outcome: Ok(response) };
    let mut reports = Vec::with_capacity(rounds);
    for i in 0..rounds {
        let mut buf = [0u8; 8];
        buf[0] = i as u8;
        buf[1] = (i >> 8) as u8;
        if let Some(r) = fuzz_schema_oracle_case(ir, &buf, &mut inv) {
            reports.push(r);
        }
    }
    reports
}

/// Summarize oracle reports by class.
pub fn summarize_oracle_reports(
    reports: &[OracleReport],
) -> std::collections::BTreeMap<&'static str, usize> {
    summarize(reports)
}

/// Schema / JSON smoke entry used by the `schema` fuzz target.
pub fn fuzz_schema_case(data: &[u8]) {
    fuzz_schema_oracle_cov(data);
}

/// Transport fuzz placeholder.
pub fn fuzz_transport_case(data: &[u8]) {
    let _ = data;
}

/// Allowlisted Core RPC method names for fuzz input synthesis.
pub fn enumerate_methods() -> Vec<&'static str> {
    default_allowlist().to_vec()
}

/// Parse raw fuzz bytes into a [`FuzzCase`].
pub fn parse_fuzz_input_to_case(data: &[u8]) -> FuzzCase {
    if let Ok(json) = serde_json::from_slice::<Value>(data) {
        if let Some(obj) = json.as_object() {
            let method_name = obj
                .get("method")
                .and_then(|v| v.as_str())
                .unwrap_or("getblockchaininfo")
                .to_string();
            let parameters = obj
                .get("params")
                .and_then(|v| v.as_object())
                .map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect::<HashMap<_, _>>())
                .unwrap_or_default();
            return FuzzCase { method_name, parameters, expected_result_type: None };
        }
    }

    let methods = enumerate_methods();
    let idx = if data.is_empty() { 0 } else { (data[0] as usize) % methods.len() };
    let method_name = methods[idx].to_string();

    deterministic_rng::init_with_seed(if data.len() > 1 { &data[1..] } else { data });

    FuzzCase {
        method_name,
        parameters: HashMap::new(),
        expected_result_type: Some("object".to_string()),
    }
}

/// Run a fuzz case against registered RPC adapters (none by default).
pub fn fuzz_rpc_case(case: FuzzCase) {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    rt.block_on(async move {
        let adapters = build_default_rpc_adapters();
        for adapter in adapters {
            let _ = adapter.apply_fuzz_case(&case).await;
        }
    });
}

/// Differential multi-adapter pass (no-op until ≥2 backends register).
pub fn fuzz_differential_case(case: FuzzCase) {
    let adapters = build_default_rpc_adapters()
        .into_iter()
        .map(|a| a as Box<dyn ProtocolAdapter>)
        .collect::<Vec<_>>();
    if adapters.len() < 2 {
        return;
    }
    let analyzer = DifferentialAnalyzer::new(adapters);
    let _ = analyzer.run_fuzz_case(&case);
}

fn build_default_rpc_adapters() -> Vec<Box<RpcAdapter>> {
    let _ = Implementation::BitcoinCore;
    Vec::new()
}
