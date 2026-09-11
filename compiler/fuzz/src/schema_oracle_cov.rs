//! Coverage-guided schema-oracle runner (libFuzzer entry + offline bank).
//!
//! Explores param gen / mutate / IR classify edges under sanitizer coverage.
//! Live bitcoind stays in `ethos-schema-oracle --continuous`; this harness is the
//! in-process **cov feedback** half of the vacant niche.
//!
//! Input layout (deterministic):
//! - `data[0]` — method pick (via [`pick_rpc`])
//! - `data[1]` — mode flags
//! - `data[2..]` — entropy for gen / mutate / response synth
//!
//! Mode bits (`data[1]`):
//! - bit0 — mutate previous params for this method when available
//! - bit1..2 — response channel: `00` golden-or-synth, `01` reject, `10` cover
//!   mismatch side-path only, `11` cover DecodeFail side-path only
//! - bit3 — prefer golden when bank has method (else synth)

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ethos_analysis::{
    classify, default_allowlist, generate_params, generate_result_value, mutate_params, pick_rpc,
    InvokeError, OracleReport, RpcInvoker,
};
use ir::ProtocolIR;
use serde_json::Value;

use crate::{default_bitcoin_ir_path, load_bitcoin_ir};

/// Outcome of one cov-guided case.
#[derive(Debug, Clone)]
pub struct CovGuidedOutcome {
    /// Classified observation.
    pub report: OracleReport,
    /// True when a **golden** body hit SchemaMismatch / DecodeFail (not synth / inject).
    pub interesting: bool,
}

/// Fixture bank: method name → JSON success bodies (goldens).
#[derive(Debug, Default, Clone)]
pub struct ResponseBank {
    by_method: HashMap<String, Vec<Value>>,
}

impl ResponseBank {
    /// Empty bank.
    pub fn new() -> Self { Self::default() }

    /// Load `*_result_min.json` / known fixtures from `rpc_golden` dir.
    ///
    /// Filename stem before `_` must match RPC name, or use explicit map below.
    pub fn load_rpc_golden_dir(dir: &Path) -> Result<Self, String> {
        let mut bank = Self::new();
        if !dir.is_dir() {
            return Ok(bank);
        }
        let entries = std::fs::read_dir(dir).map_err(|e| e.to_string())?;
        for ent in entries {
            let ent = ent.map_err(|e| e.to_string())?;
            let path = ent.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let method = golden_stem_to_method(stem);
            let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let value: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
            bank.by_method.entry(method).or_default().push(value);
        }
        Ok(bank)
    }

    /// Push a fixture for `method`.
    pub fn insert(&mut self, method: &str, value: Value) {
        self.by_method.entry(method.to_string()).or_default().push(value);
    }

    /// Pick a fixture for `method` using `salt` (None if empty).
    pub fn pick(&self, method: &str, salt: u8) -> Option<&Value> {
        let vals = self.by_method.get(method)?;
        if vals.is_empty() {
            return None;
        }
        Some(&vals[(salt as usize) % vals.len()])
    }

    /// Number of methods with ≥1 fixture.
    pub fn method_count(&self) -> usize { self.by_method.len() }
}

fn golden_stem_to_method(stem: &str) -> String {
    // getblockchaininfo_result_min → getblockchaininfo
    // getrawtransaction_verbose_min → getrawtransaction
    // getblocktemplate_verbose_min → getblocktemplate
    const SUFFIXES: &[&str] =
        &["_result_min", "_verbose_min", "_hex_min", "_result", "_verbose", "_hex"];
    for suf in SUFFIXES {
        if let Some(prefix) = stem.strip_suffix(suf) {
            return prefix.to_string();
        }
    }
    stem.to_string()
}

fn default_golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources/testdata/rpc_golden")
}

fn shared_ir() -> Option<&'static ProtocolIR> {
    static IR: OnceLock<Option<ProtocolIR>> = OnceLock::new();
    IR.get_or_init(|| {
        let path = default_bitcoin_ir_path();
        load_bitcoin_ir(&path).ok()
    })
    .as_ref()
}

fn shared_bank() -> &'static ResponseBank {
    static BANK: OnceLock<ResponseBank> = OnceLock::new();
    BANK.get_or_init(|| {
        ResponseBank::load_rpc_golden_dir(&default_golden_dir()).unwrap_or_default()
    })
}

thread_local! {
    static LAST_PARAMS: std::cell::RefCell<HashMap<String, Vec<Value>>> =
        std::cell::RefCell::new(HashMap::new());
}

/// Invoker driven by mode + bank + IR result synth.
pub struct CovGuidedInvoker<'a> {
    /// RPC under test (for synth).
    pub rpc: &'a ir::RpcDef,
    /// Fixture bank.
    pub bank: &'a ResponseBank,
    /// Mode byte from input.
    pub mode: u8,
    /// Entropy for synth / pick.
    pub entropy: &'a [u8],
    /// When true, main response was intentionally not a success body.
    pub injected: bool,
    /// When true, success body came from the golden bank (not synth).
    pub from_golden: bool,
}

impl RpcInvoker for CovGuidedInvoker<'_> {
    fn invoke(&mut self, method: &str, _params: &[Value]) -> Result<Value, InvokeError> {
        let channel = (self.mode >> 1) & 0b11;
        match channel {
            0b01 => {
                self.injected = true;
                Err(InvokeError::Rpc {
                    code: Some(-8),
                    message: "cov-guided injected reject".into(),
                })
            }
            _ => {
                let prefer_golden = self.mode & 0b1000 != 0 || self.bank.pick(method, 0).is_some();
                let salt = self.entropy.first().copied().unwrap_or(0);
                if prefer_golden {
                    if let Some(v) = self.bank.pick(method, salt) {
                        self.from_golden = true;
                        return Ok(v.clone());
                    }
                }
                if let Some(ty) = self.rpc.result.as_ref() {
                    Ok(generate_result_value(ty, self.entropy))
                } else {
                    Ok(Value::Null)
                }
            }
        }
    }
}

/// Run one cov-guided case against `ir` / `bank`.
pub fn run_cov_guided_case(
    ir: &ProtocolIR,
    data: &[u8],
    bank: &ResponseBank,
) -> Option<CovGuidedOutcome> {
    if data.is_empty() {
        return None;
    }
    let rpc = pick_rpc(ir, data, default_allowlist())?;
    let mode = if data.len() > 1 { data[1] } else { 0 };
    let entropy = if data.len() > 2 { &data[2..] } else { &[][..] };

    let do_mutate = mode & 1 == 1;
    let params = LAST_PARAMS.with(|cell| {
        let mut map = cell.borrow_mut();
        let params = if do_mutate {
            if let Some(prev) = map.get(&rpc.name) {
                mutate_params(rpc, prev, entropy)
            } else {
                generate_params(rpc, entropy)
            }
        } else {
            generate_params(rpc, entropy)
        };
        map.insert(rpc.name.clone(), params.clone());
        params
    });

    let mut inv =
        CovGuidedInvoker { rpc, bank, mode, entropy, injected: false, from_golden: false };
    let outcome = inv.invoke(&rpc.name, &params);
    let injected = inv.injected;
    let from_golden = inv.from_golden;

    // Side-path: force classify edges without marking interesting.
    let channel = (mode >> 1) & 0b11;
    if channel == 0b10 {
        let _ = classify(rpc, &params, Ok(Value::Bool(false)), None);
    } else if channel == 0b11 {
        let ok_body = if let Some(ty) = rpc.result.as_ref() {
            generate_result_value(ty, entropy)
        } else {
            Value::Null
        };
        let _ = classify(rpc, &params, Ok(ok_body), Some(Err("cov-guided injected decode".into())));
    }

    let class = classify(rpc, &params, outcome, None);
    // Synth bodies can fail IR validate (gen vs check gap). Only goldens count as findings.
    let interesting = !injected && from_golden && class.is_oracle_finding();
    Some(CovGuidedOutcome {
        report: OracleReport { method: rpc.name.clone(), params, class },
        interesting,
    })
}

/// libFuzzer entry: explore edges; panic on real oracle findings so corpus keeps them.
pub fn fuzz_schema_oracle_cov(data: &[u8]) {
    let Some(ir) = shared_ir() else {
        return;
    };
    let bank = shared_bank();
    let Some(out) = run_cov_guided_case(ir, data, bank) else {
        return;
    };
    if out.interesting {
        panic!(
            "schema_oracle cov finding method={} class={:?}",
            out.report.method, out.report.class
        );
    }
}

/// Default corpus seed dir under the cargo-fuzz package.
pub fn schema_oracle_seed_corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fuzz/corpus/schema_oracle")
}

#[cfg(test)]
mod tests {
    use ethos_analysis::OracleClass;

    use super::*;

    #[test]
    fn golden_stem_mapping() {
        assert_eq!(golden_stem_to_method("getblockchaininfo_result_min"), "getblockchaininfo");
        assert_eq!(golden_stem_to_method("getrawtransaction_verbose_min"), "getrawtransaction");
    }

    #[test]
    fn bank_loads_goldens() {
        let bank = ResponseBank::load_rpc_golden_dir(&default_golden_dir()).expect("load");
        assert!(bank.method_count() >= 5, "expected goldens, got {}", bank.method_count());
        assert!(bank.pick("getblockchaininfo", 0).is_some());
    }

    #[test]
    fn cov_case_runs_offline() {
        let ir = load_bitcoin_ir(&default_bitcoin_ir_path()).expect("ir");
        let bank = ResponseBank::load_rpc_golden_dir(&default_golden_dir()).expect("bank");
        let data = [0u8, 0b1000, 1, 2, 3, 4, 5, 6, 7, 8];
        let out = run_cov_guided_case(&ir, &data, &bank).expect("case");
        // Prefer golden / synth — should not be transport.
        assert!(
            matches!(
                out.report.class,
                OracleClass::Ok
                    | OracleClass::ExpectedReject { .. }
                    | OracleClass::SchemaMismatch { .. }
            ),
            "got {:?}",
            out.report.class
        );
    }

    #[test]
    fn injected_reject_not_interesting() {
        let ir = load_bitcoin_ir(&default_bitcoin_ir_path()).expect("ir");
        let bank = ResponseBank::new();
        // mode bit1..2 = 01 → reject
        let data = [0u8, 0b010, 9, 9, 9];
        let out = run_cov_guided_case(&ir, &data, &bank).expect("case");
        assert!(matches!(out.report.class, OracleClass::ExpectedReject { .. }));
        assert!(!out.interesting);
    }
}
