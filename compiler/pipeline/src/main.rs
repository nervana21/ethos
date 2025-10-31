//! Pipeline CLI tool for the Ethos compiler.
//!
//! This binary provides a command-line interface for running the Ethos compilation pipeline.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]

use std::env;
use std::path::PathBuf;

use types::{Implementation, ProtocolVersion};

fn main() {
    let args: Vec<String> = env::args().collect();

    match args.len() {
        1 => {
            // No arguments: build all implementations from registry
            if let Err(e) = pipeline::run_all() {
                eprintln!("pipeline failed: {}", e);
                std::process::exit(1);
            }
        }
        4 => {
            // Three arguments: build specific implementation (protocol spec path ignored in IR mode)
            let _protocol_spec_path = PathBuf::from(&args[1]);
            let implementation = match args[2].parse::<Implementation>() {
                Ok(impl_name) => impl_name,
                Err(e) => {
                    eprintln!("Error: Invalid implementation name '{}': {}", args[2], e);
                    std::process::exit(1);
                }
            };
            let version = ProtocolVersion::from_string_with_protocol(
                &args[3],
                Some(implementation.to_string()),
            )
            .expect("Failed to parse version");

            if let Err(e) = pipeline::compile_from_ir(implementation, &version, None) {
                eprintln!("pipeline failed: {}", e);
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!("Usage:");
            eprintln!(
                "  {}                                    # Build all implementations from registry",
                args[0]
            );
            eprintln!(
                "  {} <protocol_spec> <implementation> <version>  # Build specific implementation",
                args[0]
            );
            eprintln!(
                "  {} --dump-ir <implementation>         # Extract IR from schema and save to file",
                args[0]
            );
            eprintln!("Examples:");
            eprintln!("  {}                                    # Build bitcoin_core v30.0 and core_lightning v25.09.1", args[0]);
            eprintln!("  {} resources/bitcoin-api.json bitcoin_core v30.0", args[0]);
            eprintln!("  {} resources/lightning-api.json core_lightning v25.09.1", args[0]);
            eprintln!("  {} --dump-ir bitcoin_core", args[0]);
            std::process::exit(1);
        }
    }
}
