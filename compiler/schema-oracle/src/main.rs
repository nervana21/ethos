//! Live schema-oracle smoke against regtest bitcoind.
//!
//! Spawns (or attaches to) a node, generates IR-shaped params, classifies wire
//! results against Δ. Exit 1 on any `schema_mismatch` / `decode_fail`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use ethos_analysis::{
    default_allowlist, find_rpc, generate_params, invoke_error_from_rpc_body, pick_rpc, summarize,
    InvokeError, OracleReport,
};
use ethos_bitcoind::transport::{DefaultTransport, TransportError, TransportTrait};
use ethos_bitcoind::{BitcoinNodeManager, NodeManager, TestConfig};
use ir::ProtocolIR;

#[derive(Debug, Parser)]
#[command(name = "schema-oracle", about = "Δ-driven RPC schema oracle smoke")]
struct Args {
    /// Path to bitcoin.ir.json (default: resources/ir/bitcoin.ir.json from repo root).
    #[arg(long, env = "SCHEMA_ORACLE_IR")]
    ir: Option<PathBuf>,

    /// Rounds of allowlist fuzz (each round picks one method + generates params).
    #[arg(long, default_value_t = 32)]
    rounds: usize,

    /// Also call each allowlisted method once with empty/generated params (coverage pass).
    #[arg(long, default_value_t = true)]
    sweep: bool,

    /// Attach to existing RPC URL instead of spawning bitcoind (e.g. http://127.0.0.1:18443/).
    #[arg(long, env = "SCHEMA_ORACLE_RPC_URL")]
    rpc_url: Option<String>,

    /// RPC user when attaching (default rpcuser).
    #[arg(long, env = "RPC_USER", default_value = "rpcuser")]
    rpc_user: String,

    /// RPC password when attaching (default rpcpassword).
    #[arg(long, env = "RPC_PASS", default_value = "rpcpassword")]
    rpc_pass: String,

    /// Path to bitcoind when spawning (BITCOIND_PATH / corpus build / PATH).
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

struct LiveSession {
    transport: Arc<DefaultTransport>,
    /// Keep node alive for the session when we spawned it.
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
        // Keep node light for oracle smoke.
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

async fn run_one(
    rpc_name: &str,
    ir: &ProtocolIR,
    data: &[u8],
    session: &LiveSession,
    verbose: bool,
) -> Option<OracleReport> {
    let rpc = find_rpc(ir, rpc_name)?;
    let params = generate_params(rpc, data);
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
    let ir_path = args.ir.unwrap_or_else(default_ir_path);
    let ir = match ProtocolIR::from_file(&ir_path) {
        Ok(ir) => ir,
        Err(e) => {
            eprintln!("load IR {}: {e}", ir_path.display());
            return ExitCode::from(2);
        }
    };

    let session = if let Some(url) = args.rpc_url {
        eprintln!("attach {}", url);
        LiveSession::attach(url, args.rpc_user, args.rpc_pass)
    } else {
        let bitcoind = match resolve_bitcoind(args.bitcoind) {
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

    let mut reports: Vec<OracleReport> = Vec::new();

    if args.sweep {
        eprintln!("sweep allowlist…");
        for (i, name) in default_allowlist().iter().enumerate() {
            let seed = [i as u8, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77];
            if let Some(r) = run_one(name, &ir, &seed, &session, args.verbose).await {
                reports.push(r);
            } else {
                eprintln!("  skip {name} (not in IR)");
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
        let Some(rpc) = pick_rpc(&ir, &buf, default_allowlist()) else {
            continue;
        };
        let seed = &buf[1..];
        let params = generate_params(rpc, seed);
        let outcome = session.invoke(&rpc.name, &params).await;
        let class = ethos_analysis::classify(rpc, outcome, None);
        let report = OracleReport { method: rpc.name.clone(), params, class };
        if args.verbose || report.class.is_oracle_finding() {
            println!("  {} params={} class={:?}", report.method, report.params.len(), report.class);
        }
        reports.push(report);
    }

    let summary = summarize(&reports);
    eprintln!("summary: {summary:?} (n={})", reports.len());

    let findings: Vec<_> = reports.iter().filter(|r| r.class.is_oracle_finding()).collect();
    let exit = if !findings.is_empty() {
        eprintln!("oracle findings: {}", findings.len());
        for f in &findings {
            eprintln!("  {} -> {:?}", f.method, f.class);
        }
        ExitCode::from(1)
    } else {
        let oks = summary.get("ok").copied().unwrap_or(0);
        if oks == 0 {
            eprintln!("no Ok classifications — node or IR likely broken");
            ExitCode::from(1)
        } else {
            eprintln!("schema-oracle smoke ok ({oks} Ok)");
            ExitCode::SUCCESS
        }
    };

    session.shutdown().await;
    exit
}
