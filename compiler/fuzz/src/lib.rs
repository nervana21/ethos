#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]

//! Ethos Fuzz Library
//!
//! This library provides fuzzing utilities and deterministic RNG for the Ethos project.

pub mod deterministic_rng;
pub mod corpus_manager;
pub mod observability;

use std::collections::HashMap;

use ethos_adapters::RpcAdapter;
use ethos_analysis::DifferentialAnalyzer;
use fuzz_types::{FuzzCase, ProtocolAdapter};
use serde_json::Value;
use types::Implementation;

pub fn fuzz_schema_case(data: &[u8]) {
    if data.is_empty() { return; }
    let _ = serde_json::from_slice::<serde_json::Value>(data);
}

pub fn fuzz_transport_case(data: &[u8]) {
    let _ = data;
}

pub fn enumerate_lightning_methods() -> Vec<&'static str> {
    [
        "GetInfo", "ListPeers", "ListChannels", "AddInvoice", "ListInvoices",
        "ListPayments", "ConnectPeer", "DisconnectPeer", "OpenChannel",
        "CloseChannel", "SendPayment", "PayInvoice",
    ].to_vec()
}

pub fn parse_fuzz_input_to_case(data: &[u8]) -> FuzzCase {
    if let Ok(json) = serde_json::from_slice::<Value>(data) {
        if let Some(obj) = json.as_object() {
            let method_name = obj.get("method")
                .and_then(|v| v.as_str())
                .unwrap_or("GetInfo")
                .to_string();
            let parameters = obj.get("params")
                .and_then(|v| v.as_object())
                .map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect::<HashMap<_, _>>())
                .unwrap_or_default();
            return FuzzCase { method_name, parameters, expected_result_type: None };
        }
    }

    let methods = enumerate_lightning_methods();
    let idx = if data.is_empty() { 0 } else { (data[0] as usize) % methods.len() };
    let method_name = methods[idx].to_string();

    deterministic_rng::init_with_seed(if data.len() > 1 { &data[1..] } else { data });

    let mut parameters = HashMap::new();
    if method_name == "AddInvoice" {
        let amount = deterministic_rng::random_range(1000, 1000000);
        parameters.insert("value".to_string(), Value::Number(amount.into()));
        parameters.insert("description".to_string(), Value::String(deterministic_rng::random_string(10)));
    } else if method_name == "ListPeers" {
        if deterministic_rng::random_bool() {
            parameters.insert("id".to_string(), Value::String(deterministic_rng::random_string(33)));
        }
    } else if method_name == "ListChannels" {
        if deterministic_rng::random_bool() {
            parameters.insert("peer".to_string(), Value::String(deterministic_rng::random_string(33)));
        }
    }

    FuzzCase { method_name, parameters, expected_result_type: Some("object".to_string()) }
}

pub fn fuzz_rpc_case(case: FuzzCase) {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    rt.block_on(async move {
        let adapters = build_default_rpc_adapters();
        for adapter in adapters {
            let _ = adapter.apply_fuzz_case(&case).await;
        }
    });
}

pub fn fuzz_differential_case(case: FuzzCase) {
    let adapters = build_default_rpc_adapters()
        .into_iter()
        .map(|a| a as Box<dyn ProtocolAdapter>)
        .collect::<Vec<_>>();
    if adapters.len() < 2 { return; }
    let analyzer = DifferentialAnalyzer::new(adapters);
    let _ = analyzer.run_fuzz_case(&case);
}

fn build_default_rpc_adapters() -> Vec<Box<RpcAdapter>> {
    // No backends registered by default (Bitcoin Core uses IR-from-file; add backends when needed for fuzzing)
    Vec::new()
}
