//! Live schema-oracle smoke + continuous fuzz against regtest bitcoind.
//!
//! Spawns (or attaches to) a node, generates/mutates IR-shaped params, classifies
//! wire results against Δ. Continuous mode writes oracle findings to a corpus dir.
//! Exit 1 on any `schema_mismatch` / `decode_fail` in smoke mode; continuous mode
//! exits 0 after duration (findings on disk are the signal).

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser;
use ethos_analysis::{
    default_allowlist, find_rpc, finding_basename, generate_params, invoke_error_from_rpc_body,
    mutate_params, pick_rpc, report_to_finding, summarize, InvokeError, OracleReport,
};
use ethos_bitcoind::transport::{DefaultTransport, TransportError, TransportTrait};
use ethos_bitcoind::{BitcoinNodeManager, NodeManager, TestConfig};
use ir::ProtocolIR;

#[derive(Debug, Parser)]
#[command(name = "schema-oracle", about = "Δ-driven RPC schema oracle smoke / continuous fuzz")]
struct Args {
    /// Path to bitcoin.ir.json (default: resources/ir/bitcoin.ir.json from repo root).
    #[arg(long, env = "SCHEMA_ORACLE_IR")]
    ir: Option<PathBuf>,

    /// Rounds of allowlist fuzz (smoke mode).
    #[arg(long, default_value_t = 32)]
    rounds: usize,

    /// Also call each allowlisted method once with generated params (coverage pass).
    #[arg(long, default_value_t = true)]
    sweep: bool,

    /// Continuous fuzz until --duration-secs elapses (writes findings corpus).
    #[arg(long)]
    continuous: bool,

    /// How long continuous mode runs.
    #[arg(long, default_value_t = 30)]
    duration_secs: u64,

    /// Directory for schema_mismatch / decode_fail JSON findings.
    #[arg(long, default_value = "resources/testdata/schema_oracle_corpus")]
    corpus_dir: PathBuf,

    /// Also persist expected_reject samples (capped) for triage.
    #[arg(long, default_value_t = false)]
    save_rejects: bool,

    /// Max reject files to keep when --save-rejects.
    #[arg(long, default_value_t = 32)]
    max_rejects: usize,

    /// Attach to existing RPC URL instead of spawning bitcoind.
    #[arg(long, env = "SCHEMA_ORACLE_RPC_URL")]
    rpc_url: Option<String>,

    /// RPC user when attaching (default rpcuser).
    #[arg(long, env = "RPC_USER", default_value = "rpcuser")]
    rpc_user: String,

    /// RPC password when attaching (default rpcpassword).
    #[arg(long, env = "RPC_PASS", default_value = "rpcpassword")]
    rpc_pass: String,

    /// Path to bitcoind when spawning.
    #[arg(long, env = "BITCOIND_PATH")]
    bitcoind: Option<PathBuf>,

    /// Print every report line (not only summary + findings).
    #[arg(long)]
    verbose: bool,
}

fn repo_root() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..") }

fn default_ir_path() -> PathBuf { repo_root().join("resources/ir/bitcoin.ir.json") }

fn corpus_bitcoind() -> PathBuf { repo_root().join("corpus/bitcoin/build/bin/bitcoind") }

fn resolve_bitcoind(explicit: Option<PathBuf>) -> Result<PathBuf, String> {
    let try_path = |path: &Path| -> Option<PathBuf> {
        let canonical = std::fs::canonicalize(path).ok()?;
        std::process::Command::new(&canonical).arg("--version").output().ok()?;
        Some(canonical)
    };

    if let Some(p) = explicit {
        return try_path(&p).ok_or_else(|| format!("BITCOIND_PATH={} not runnable", p.display()));
    }
    if let Ok(p) = std::env::var("BITCOIND_PATH") {
        let path = PathBuf::from(p);
        if let Some(c) = try_path(&path) {
            return Ok(c);
        }
        return Err(format!("BITCOIND_PATH={} not runnable", path.display()));
    }
    for cand in [
        corpus_bitcoind(),
        PathBuf::from("/usr/local/bin/bitcoind"),
        PathBuf::from("/opt/homebrew/bin/bitcoind"),
        PathBuf::from("bitcoind"),
    ] {
        if let Some(c) = try_path(&cand) {
            return Ok(c);
        }
    }
    Err("bitcoind not found; set BITCOIND_PATH".into())
}

fn map_transport(err: TransportError) -> InvokeError {
    match err {
        TransportError::Rpc(body) => invoke_error_from_rpc_body(&body),
        other => InvokeError::Transport(other.to_string()),
    }
}

fn resolve_corpus_dir(arg: &Path) -> PathBuf {
    if arg.is_absolute() {
        arg.to_path_buf()
    } else {
        repo_root().join(arg)
    }
}

fn write_finding(dir: &Path, report: &OracleReport, seed: &[u8]) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(finding_basename(report));
    let finding = report_to_finding(report, seed);
    std::fs::write(&path, serde_json::to_vec_pretty(&finding)?)?;
    Ok(path)
}

struct LiveSession {
    transport: Arc<DefaultTransport>,
    node: Option<BitcoinNodeManager>,
}

impl LiveSession {
    async fn spawn(bitcoind: PathBuf) -> Result<Self, String> {
        let tmp = repo_root().join("target/schema_oracle_tmp");
        std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
        let tmp = tmp.canonicalize().map_err(|e| e.to_string())?;
        std::env::set_var("TMPDIR", &tmp);

        let mut config = TestConfig::default();
        config.bitcoind_path = Some(bitcoind);
        config.extra_args = vec!["-prune=1".into()];

        let node = BitcoinNodeManager::new_with_config(&config).map_err(|e| e.to_string())?;
        node.start().await.map_err(|e| e.to_string())?;
        let transport = node.create_transport().await.map_err(|e| e.to_string())?;
        Ok(Self { transport, node: Some(node) })
    }

    fn attach(url: String, user: String, pass: String) -> Self {
        let transport = Arc::new(DefaultTransport::new(url, Some((user, pass))));
        Self { transport, node: None }
    }

    async fn shutdown(mut self) {
        if let Some(mut node) = self.node.take() {
            if let Err(e) = node.stop().await {
                eprintln!("warning: stop bitcoind: {e}");
            }
        }
    }

    async fn invoke(
        &self,
        method: &str,
        params: &[serde_json::Value],
    ) -> Result<serde_json::Value, InvokeError> {
        match self.transport.send_request(method, params).await {
            Ok(v) => Ok(v),
            Err(e) => Err(map_transport(e)),
        }
    }
}

async fn classify_call(
    rpc_name: &str,
    ir: &ProtocolIR,
    params: Vec<serde_json::Value>,
    session: &LiveSession,
    verbose: bool,
) -> Option<OracleReport> {
    let rpc = find_rpc(ir, rpc_name)?;
    let outcome = session.invoke(&rpc.name, &params).await;
    let class = ethos_analysis::classify(rpc, outcome, None);
    let report = OracleReport { method: rpc.name.clone(), params, class };
    if verbose || report.class.is_oracle_finding() {
        println!("  {} params={} class={:?}", report.method, report.params.len(), report.class);
    }
    Some(report)
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    let ir_path = args.ir.clone().unwrap_or_else(default_ir_path);
    let ir = match ProtocolIR::from_file(&ir_path) {
        Ok(ir) => ir,
        Err(e) => {
            eprintln!("load IR {}: {e}", ir_path.display());
            return ExitCode::from(2);
        }
    };

    let session = if let Some(url) = args.rpc_url.clone() {
        eprintln!("attach {url}");
        LiveSession::attach(url, args.rpc_user.clone(), args.rpc_pass.clone())
    } else {
        let bitcoind = match resolve_bitcoind(args.bitcoind.clone()) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(2);
            }
        };
        eprintln!("spawn {}", bitcoind.display());
        match LiveSession::spawn(bitcoind).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("spawn failed: {e}");
                return ExitCode::from(2);
            }
        }
    };

    let exit = if args.continuous {
        run_continuous(&args, &ir, &session).await
    } else {
        run_smoke(&args, &ir, &session).await
    };

    session.shutdown().await;
    exit
}

async fn run_smoke(args: &Args, ir: &ProtocolIR, session: &LiveSession) -> ExitCode {
    let mut reports: Vec<OracleReport> = Vec::new();

    if args.sweep {
        eprintln!("sweep allowlist…");
        for (i, name) in default_allowlist().iter().enumerate() {
            let seed = [i as u8, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77];
            let Some(rpc) = find_rpc(ir, name) else {
                eprintln!("  skip {name} (not in IR)");
                continue;
            };
            let params = generate_params(rpc, &seed);
            if let Some(r) = classify_call(name, ir, params, session, args.verbose).await {
                reports.push(r);
            }
        }
    }

    eprintln!("fuzz rounds={}…", args.rounds);
    for i in 0..args.rounds {
        let mut buf = [0u8; 16];
        buf[0] = (i & 0xff) as u8;
        buf[1] = ((i >> 8) & 0xff) as u8;
        buf[2] = 0xA5;
        buf[3] = 0x5A;
        let Some(rpc) = pick_rpc(ir, &buf, default_allowlist()) else {
            continue;
        };
        let seed = &buf[1..];
        let params = generate_params(rpc, seed);
        if let Some(r) = classify_call(&rpc.name, ir, params, session, args.verbose).await {
            reports.push(r);
        }
    }

    let summary = summarize(&reports);
    eprintln!("summary: {summary:?} (n={})", reports.len());

    let findings: Vec<_> = reports.iter().filter(|r| r.class.is_oracle_finding()).collect();
    if !findings.is_empty() {
        eprintln!("oracle findings: {}", findings.len());
        for f in &findings {
            eprintln!("  {} -> {:?}", f.method, f.class);
        }
        return ExitCode::from(1);
    }

    let oks = summary.get("ok").copied().unwrap_or(0);
    if oks == 0 {
        eprintln!("no Ok classifications — node or IR likely broken");
        return ExitCode::from(1);
    }

    eprintln!("schema-oracle smoke ok ({oks} Ok)");
    ExitCode::SUCCESS
}

async fn run_continuous(args: &Args, ir: &ProtocolIR, session: &LiveSession) -> ExitCode {
    let corpus = resolve_corpus_dir(&args.corpus_dir);
    if let Err(e) = std::fs::create_dir_all(&corpus) {
        eprintln!("corpus dir {}: {e}", corpus.display());
        return ExitCode::from(2);
    }
    eprintln!("continuous fuzz duration={}s corpus={}", args.duration_secs, corpus.display());

    let deadline = Instant::now() + Duration::from_secs(args.duration_secs);
    let mut reports = Vec::new();
    let mut iters: u64 = 0;
    let mut saved_findings = 0usize;
    let mut saved_rejects = 0usize;
    // Keep last params per method for mutate_params.
    let mut last_params: std::collections::HashMap<String, Vec<serde_json::Value>> =
        std::collections::HashMap::new();

    while Instant::now() < deadline {
        iters += 1;
        let mut buf = [0u8; 32];
        let t = iters.to_le_bytes();
        buf[..8].copy_from_slice(&t);
        let nanos = Instant::now().elapsed().as_nanos().to_le_bytes();
        buf[8..16].copy_from_slice(&nanos[..8]);
        for (i, b) in buf[16..].iter_mut().enumerate() {
            *b = ((iters as u8).wrapping_mul(31).wrapping_add(i as u8)).wrapping_add(0x3C);
        }

        let Some(rpc) = pick_rpc(ir, &buf, default_allowlist()) else {
            continue;
        };
        let seed = &buf[1..];
        let params = if let Some(prev) = last_params.get(&rpc.name) {
            if buf[0] & 1 == 1 {
                mutate_params(rpc, prev, seed)
            } else {
                generate_params(rpc, seed)
            }
        } else {
            generate_params(rpc, seed)
        };
        last_params.insert(rpc.name.clone(), params.clone());

        let Some(report) = classify_call(&rpc.name, ir, params, session, args.verbose).await else {
            continue;
        };

        if report.class.is_oracle_finding() {
            match write_finding(&corpus, &report, seed) {
                Ok(path) => {
                    saved_findings += 1;
                    eprintln!("FINDING {} -> {}", report.method, path.display());
                }
                Err(e) => eprintln!("write finding: {e}"),
            }
        } else if args.save_rejects
            && saved_rejects < args.max_rejects
            && matches!(report.class, ethos_analysis::OracleClass::ExpectedReject { .. })
        {
            if write_finding(&corpus, &report, seed).is_ok() {
                saved_rejects += 1;
            }
        }

        reports.push(report);
    }

    let summary = summarize(&reports);
    eprintln!(
        "continuous done iters={iters} findings_saved={saved_findings} rejects_saved={saved_rejects}"
    );
    eprintln!("summary: {summary:?} (n={})", reports.len());
    ExitCode::SUCCESS
}
