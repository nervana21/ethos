// SPDX-License-Identifier: CC0-1.0

//! Strict OpenRPC schema keyword allowlist audit (Draft 7).
//!
//! Soft warnings (invalid `default`) print to stdout; hard failures exit 1.

use std::path::PathBuf;
use std::process::ExitCode;
use std::{env, fs};

use ethos_adapters::bitcoin_core::openrpc_schema_audit;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("Usage: openrpc_schema_keyword_audit <openrpc.json> [--json-report <path>]");
        return ExitCode::from(2);
    };
    let mut json_report: Option<PathBuf> = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--json-report" => {
                let Some(out) = args.next() else {
                    eprintln!("--json-report requires a path");
                    return ExitCode::from(2);
                };
                json_report = Some(PathBuf::from(out));
            }
            other => {
                eprintln!("unknown argument: {other}");
                return ExitCode::from(2);
            }
        }
    }

    let raw = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let document: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("failed to parse {path}: {e}");
            return ExitCode::FAILURE;
        }
    };

    match openrpc_schema_audit::inspect(&document) {
        Ok(report) => {
            print!("{report}");
            if let Some(out) = json_report {
                let payload = serde_json::json!({
                    "method_count": report.method_count,
                    "parameter_count": report.parameter_count,
                    "schema_count": report.schema_count,
                    "keyword_counts": report.keyword_counts,
                    "warnings": report.warnings,
                });
                if let Err(e) = fs::write(&out, format!("{}\n", payload)) {
                    eprintln!("failed to write {}: {e}", out.display());
                    return ExitCode::FAILURE;
                }
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("openrpc schema keyword audit failed: {error}");
            ExitCode::FAILURE
        }
    }
}
