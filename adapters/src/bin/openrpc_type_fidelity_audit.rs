// SPDX-License-Identifier: CC0-1.0

//! Schema-first OpenRPC fidelity gate for Bitcoin Core `getopenrpcinfo` / `rpc.discover` dumps.
//!
//! Hard fail (exit 1): P0 findings only.
//! P1/P2 are reported for upstream follow-ups but do not fail the gate (Core may still omit
//! call-site enums and some `x-bitcoin-discriminated-result` metadata).
//!
//! Core `RPCArg::Type::NUM` dumps as JSON Schema `type: number` by contract. Ethos must not
//! treat that as a fidelity defect (name-heuristic `integer` is out of the final set).

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
struct Finding {
    rule: String,
    severity: String,
    method: String,
    field: String,
    message: String,
}

fn walk_has_null_schema(schema: &Value) -> bool {
    match schema {
        Value::Object(map) => {
            if map.get("type").is_some_and(|t| t == "null") {
                return true;
            }
            if map
                .get("type")
                .and_then(Value::as_array)
                .is_some_and(|arr| arr.iter().any(|v| v == "null"))
            {
                return true;
            }
            for key in [
                "oneOf",
                "anyOf",
                "allOf",
                "items",
                "prefixItems",
                "properties",
                "additionalProperties",
            ] {
                if let Some(v) = map.get(key) {
                    match v {
                        Value::Array(arr) =>
                            if arr.iter().any(walk_has_null_schema) {
                                return true;
                            },
                        Value::Object(obj) if key == "properties" => {
                            if obj.values().any(walk_has_null_schema) {
                                return true;
                            }
                        }
                        _ =>
                            if walk_has_null_schema(v) {
                                return true;
                            },
                    }
                }
            }
            false
        }
        _ => false,
    }
}

fn walk_has_enum(schema: &Value) -> bool {
    match schema {
        Value::Object(map) => {
            if map.contains_key("enum") {
                return true;
            }
            map.values().any(walk_has_enum)
        }
        Value::Array(arr) => arr.iter().any(walk_has_enum),
        _ => false,
    }
}

fn oneof_has_null_branch(schema: &Value) -> bool {
    let Some(branches) = schema.get("oneOf").and_then(Value::as_array) else {
        return false;
    };
    branches.iter().any(|b| {
        b.get("type").and_then(Value::as_str) == Some("null")
            || b.get("oneOf").and_then(Value::as_array).is_some_and(|nested| {
                nested.iter().any(|v| v.get("type").and_then(Value::as_str) == Some("null"))
            })
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "Usage: {} <openrpc.json> [--json-report <path>]",
            args.first().map_or("openrpc_type_fidelity_audit", String::as_str)
        );
        std::process::exit(2);
    }

    let openrpc_path = PathBuf::from(&args[1]);
    let mut json_report: Option<PathBuf> = None;
    if args.len() == 4 && args[2] == "--json-report" {
        json_report = Some(PathBuf::from(&args[3]));
    }

    let raw = fs::read_to_string(&openrpc_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", openrpc_path.display()));
    let doc: Value = serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("failed to parse {}: {e}", openrpc_path.display()));

    let openrpc_ver = doc.get("openrpc").and_then(Value::as_str).unwrap_or("");
    if openrpc_ver.is_empty() {
        eprintln!("missing top-level openrpc version");
        std::process::exit(1);
    }

    let methods = doc
        .get("methods")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{} missing top-level methods[]", openrpc_path.display()));

    let mut findings: Vec<Finding> = Vec::new();
    let mut seen_rules: BTreeSet<String> = BTreeSet::new();

    let mut has_discover = false;
    let mut has_getopenrpcinfo = false;

    for method in methods {
        let method_name =
            method.get("name").and_then(Value::as_str).unwrap_or("<unnamed>").to_string();
        if method_name == "rpc.discover" {
            has_discover = true;
        }
        if method_name == "getopenrpcinfo" {
            has_getopenrpcinfo = true;
        }

        // Schema-first Core dumps must not require legacy x-bitcoin-arguments / x-bitcoin-results.
        if method.get("x-bitcoin-arguments").is_some() && method.get("params").is_none() {
            findings.push(Finding {
                rule: "legacy_arguments_without_params".to_string(),
                severity: "P1".to_string(),
                method: method_name.clone(),
                field: "params".to_string(),
                message: "Has x-bitcoin-arguments but no OpenRPC params[] (unexpected hybrid)."
                    .to_string(),
            });
            seen_rules.insert("legacy_arguments_without_params".to_string());
        }

        let params = method.get("params").and_then(Value::as_array).cloned().unwrap_or_default();
        for param in params {
            let param_name =
                param.get("name").and_then(Value::as_str).unwrap_or("<unnamed>").to_string();
            let desc =
                param.get("description").and_then(Value::as_str).unwrap_or("").to_ascii_lowercase();
            let schema = param.get("schema").cloned().unwrap_or(Value::Null);

            if desc.contains("null") && !walk_has_null_schema(&schema) {
                findings.push(Finding {
                    rule: "nullable_in_description_but_schema_not_nullable".to_string(),
                    severity: "P1".to_string(),
                    method: method_name.clone(),
                    field: format!("param:{param_name}"),
                    message: "Description references null, but schema does not allow null."
                        .to_string(),
                });
                seen_rules.insert("nullable_in_description_but_schema_not_nullable".to_string());
            }

            if (desc.contains("must be one of") || desc.contains("one of ("))
                && !walk_has_enum(&schema)
            {
                findings.push(Finding {
                    rule: "enum_in_description_but_schema_missing_enum".to_string(),
                    severity: "P1".to_string(),
                    method: method_name.clone(),
                    field: format!("param:{param_name}"),
                    message: "Description constrains allowed values, but schema has no enum."
                        .to_string(),
                });
                seen_rules.insert("enum_in_description_but_schema_missing_enum".to_string());
            }
        }

        if method_name == "getaddednodeinfo" {
            let req = method
                .pointer("/result/schema/items/required")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let has_addresses_required = req.iter().any(|v| v == "addresses");
            let uses_conditional_schema =
                method.pointer("/result/schema/items/allOf").and_then(Value::as_array).is_some();
            if has_addresses_required && !uses_conditional_schema {
                findings.push(Finding {
                    rule: "conditional_field_modeled_as_unconditionally_required".to_string(),
                    severity: "P2".to_string(),
                    method: method_name.clone(),
                    field: "result[].addresses".to_string(),
                    message: "addresses is required but docs say only when connected=true. \
                              Refresh openrpc.json from a Core build that emits \
                              getaddednodeinfo if/then (required_when_true)."
                        .to_string(),
                });
                seen_rules
                    .insert("conditional_field_modeled_as_unconditionally_required".to_string());
            }
        }

        if [
            "getrawmempool",
            "getmempoolancestors",
            "getmempooldescendants",
            "scanblocks",
            "scantxoutset",
        ]
        .contains(&method_name.as_str())
        {
            let result_schema = method.pointer("/result/schema").cloned().unwrap_or(Value::Null);
            if result_schema.get("oneOf").and_then(Value::as_array).is_none() {
                findings.push(Finding {
                    rule: "discriminated_result_missing_oneof".to_string(),
                    severity: "P0".to_string(),
                    method: method_name.clone(),
                    field: "result.schema".to_string(),
                    message: "Expected conditional-result method to expose oneOf schema branches."
                        .to_string(),
                });
                seen_rules.insert("discriminated_result_missing_oneof".to_string());
            }
            // Soft: Core currently only stamps x-bitcoin-discriminated-result on a few methods.
            if result_schema.get("x-bitcoin-discriminated-result").is_none() {
                findings.push(Finding {
                    rule: "discriminated_result_missing_metadata".to_string(),
                    severity: "P2".to_string(),
                    method: method_name.clone(),
                    field: "result.schema".to_string(),
                    message:
                        "Advisory: x-bitcoin-discriminated-result missing; refresh dump from Core fidelity tip."
                            .to_string(),
                });
                seen_rules.insert("discriminated_result_missing_metadata".to_string());
            }
        }

        if ["scanblocks", "scantxoutset"].contains(&method_name.as_str()) {
            let result_schema = method.pointer("/result/schema").cloned().unwrap_or(Value::Null);
            if !oneof_has_null_branch(&result_schema) {
                findings.push(Finding {
                    rule: "scan_status_branch_missing_null_variant".to_string(),
                    severity: "P0".to_string(),
                    method: method_name.clone(),
                    field: "result.schema.oneOf".to_string(),
                    message: "Expected a null status branch when no scan is in progress."
                        .to_string(),
                });
                seen_rules.insert("scan_status_branch_missing_null_variant".to_string());
            }
        }
    }

    if !has_discover {
        findings.push(Finding {
            rule: "missing_rpc_discover".to_string(),
            severity: "P0".to_string(),
            method: "rpc.discover".to_string(),
            field: "methods".to_string(),
            message: "OpenRPC 1.4.1 dumps should include the rpc.discover service method."
                .to_string(),
        });
        seen_rules.insert("missing_rpc_discover".to_string());
    }
    if !has_getopenrpcinfo {
        findings.push(Finding {
            rule: "missing_getopenrpcinfo".to_string(),
            severity: "P0".to_string(),
            method: "getopenrpcinfo".to_string(),
            field: "methods".to_string(),
            message: "Dump should include getopenrpcinfo.".to_string(),
        });
        seen_rules.insert("missing_getopenrpcinfo".to_string());
    }

    if let Some(path) = json_report {
        let payload = serde_json::json!({
            "openrpc_version": openrpc_ver,
            "rule_count": seen_rules.len(),
            "finding_count": findings.len(),
            "findings": findings,
        });
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .unwrap_or_else(|e| panic!("failed to create {}: {e}", parent.display()));
        }
        // serde_json pretty output has no trailing newline. pre-commit
        // end-of-file-fixer adds one and aborts the commit, then the next
        // regen deletes it again.
        let mut out = serde_json::to_string_pretty(&payload).expect("serialize report");
        if !out.ends_with('\n') {
            out.push('\n');
        }
        fs::write(&path, out).unwrap_or_else(|e| panic!("failed to write {}: {e}", path.display()));
        println!("wrote {}", path.display());
    }

    for finding in &findings {
        println!(
            "[{}][{}] {} {} - {}",
            finding.severity, finding.rule, finding.method, finding.field, finding.message
        );
    }
    let p0 = findings.iter().filter(|f| f.severity == "P0").count();
    println!("rules_triggered={} findings={} p0={}", seen_rules.len(), findings.len(), p0);

    if p0 == 0 {
        std::process::exit(0);
    }
    std::process::exit(1);
}
