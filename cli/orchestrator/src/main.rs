//! Ethos CLI orchestrator
//!
//! This binary provides the main entry point for Ethos,
//! offering various subcommands for code generation and pipeline execution.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]

use std::env;
use std::path::PathBuf;

use types::{Implementation, ProtocolVersion};

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    // Handle help flag
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("Ethos");
        println!();
        println!("USAGE:");
        println!("    ethos-cli [SUBCOMMAND] [FLAGS]");
        println!();
        println!("SUBCOMMANDS:");
        println!("    pipeline                       Run code generation pipeline");
        println!("    --help, -h                    Show this help message");
        println!("FLAGS:");
        println!("    --input <ir_file>             Load ProtocolIR directly from .ir.json file (optional if --implementation is provided)");
        println!("    --implementation <impl>       Implementation to generate (bitcoin_core) [REQUIRED]");
        println!("    --version <version>           Override version (e.g., v30.2.3)");
        println!("    --output <path>               Write generated crate to <path> (e.g. a separate git repo). Preserves .git for easier diff review.");
        println!();
        println!("EXAMPLES:");
        println!("    ethos-cli pipeline --input resources/ir/bitcoin.ir.json --implementation bitcoin_core");
        println!("    ethos-cli pipeline --implementation bitcoin_core --output ../ethos-bitcoind   # generate into a separate repo");
        return;
    }

    // Handle dump-ir subcommand
    if args.iter().any(|a| a == "dump-ir") {
        // --implementation <impl>
        let impl_arg =
            args.iter().position(|a| a == "--implementation").and_then(|i| args.get(i + 1));

        // --output <path>
        let out_arg = args.iter().position(|a| a == "--output").and_then(|i| args.get(i + 1));

        if impl_arg.is_none() || out_arg.is_none() {
            eprintln!("Error: dump-ir requires --implementation <impl> and --output <path>");
            eprintln!("Use 'ethos-cli --help' for usage information");
            std::process::exit(1);
        }

        let implementation_str =
            impl_arg.expect("validated: --implementation argument must follow");
        let output_path = PathBuf::from(out_arg.expect("validated: --output argument must follow"));

        // Resolve IR path via registry
        match dump_ir_for_implementation(implementation_str, &output_path) {
            Ok(_) => {
                println!("IR for '{}' written to {}", implementation_str, output_path.display());
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("Failed to dump IR: {}", e);
                std::process::exit(1);
            }
        }
    }

    // Handle pipeline subcommand
    if args.iter().any(|a| a == "pipeline") {
        use ir::ProtocolIR;
        use registry::ir_resolver::IrResolver;

        // Get implementation first (required for auto-resolution)
        let implementation =
            match args.iter().position(|a| a == "--implementation").and_then(|i| args.get(i + 1)) {
                Some(impl_str) => match impl_str.parse::<Implementation>() {
                    Ok(impl_type) => impl_type,
                    Err(e) => {
                        eprintln!("Error: Invalid implementation '{}': {}", impl_str, e);
                        std::process::exit(1);
                    }
                },
                None => {
                    eprintln!("Error: --implementation <impl> is required");
                    eprintln!("Use 'ethos-cli --help' for usage information");
                    std::process::exit(1);
                }
            };

        // Resolve IR file: use --input if provided, otherwise auto-resolve from implementation
        let ir_path = match args.iter().position(|a| a == "--input").and_then(|i| args.get(i + 1)) {
            Some(ir_file) => PathBuf::from(ir_file),
            None => {
                // Auto-resolve IR file from implementation using registry
                let resolver = match IrResolver::new() {
                    Ok(resolver) => resolver,
                    Err(e) => {
                        eprintln!("Error: Failed to create IR resolver: {}", e);
                        std::process::exit(1);
                    }
                };
                match resolver.resolve_ir_path_for_implementation(&implementation) {
                    Ok(path) => path,
                    Err(e) => {
                        eprintln!("Error: Failed to resolve IR file for {}: {}", implementation, e);
                        std::process::exit(1);
                    }
                }
            }
        };

        let ir = match ProtocolIR::from_file(&ir_path) {
            Ok(ir) => ir,
            Err(e) => {
                eprintln!("Error: Failed to load IR from file '{}': {}", ir_path.display(), e);
                std::process::exit(1);
            }
        };

        let version_str = args
            .iter()
            .position(|a| a == "--version")
            .and_then(|i| args.get(i + 1))
            .map(|s| s.as_str())
            .unwrap_or_else(|| get_latest_version_for_implementation(&implementation));

        let protocol_version = match ProtocolVersion::from_string_with_protocol(
            version_str,
            Some(implementation.to_string()),
        ) {
            Ok(version) => version,
            Err(e) => {
                eprintln!("Error: Failed to parse version '{}': {}", version_str, e);
                std::process::exit(1);
            }
        };

        // Output directory: --output <path> or default outputs/generated/{crate_name}
        let output_arg = args.iter().position(|a| a == "--output").and_then(|i| args.get(i + 1));
        let crate_dir = match output_arg {
            Some(path) => PathBuf::from(path),
            None => {
                let project_root = match path::find_project_root() {
                    Ok(root) => root,
                    Err(e) => {
                        eprintln!("Error: Failed to locate project root: {}", e);
                        std::process::exit(1);
                    }
                };
                project_root
                    .join(format!("outputs/generated/{}", implementation.published_crate_name()))
            }
        };

        if let Err(e) = pipeline::prepare_output_dir(&crate_dir) {
            eprintln!("Error: Failed to prepare output dir: {}", e);
            std::process::exit(1);
        }

        // Run compilation with the loaded IR
        if let Err(e) = compile_with_ir(ir, implementation, &protocol_version, &crate_dir) {
            eprintln!("IR compilation failed: {}", e);
            std::process::exit(1);
        }
        return;
    }

    // No default behavior - show help if no valid subcommand provided
    eprintln!("Error: No valid subcommand provided");
    eprintln!("Use 'ethos-cli --help' for usage information");
    std::process::exit(1);
}

/// Dump the ProtocolIR for a given implementation to a file
fn dump_ir_for_implementation(
    implementation: &str,
    output_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    use ir::ProtocolIR;
    use registry::ir_resolver::IrResolver;
    use types::Implementation as ImplEnum;

    let impl_enum = implementation.parse::<ImplEnum>()?;
    let resolver = IrResolver::new()?;
    let ir_path = resolver.resolve_ir_path_for_implementation(&impl_enum)?;

    let ir = ProtocolIR::from_file(&ir_path)?;
    ir.to_file(output_path)?;
    Ok(())
}

/// Compile using a pre-loaded ProtocolIR instead of extracting from schema
fn compile_with_ir(
    mut ir: ir::ProtocolIR,
    implementation: Implementation,
    version: &ProtocolVersion,
    output_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    use path::find_project_root;
    use pipeline::codegen_orchestration::{analyze_implementation, generate_into};
    use pipeline::project_setup::setup_project_files;
    use pipeline::protocol_compiler::EthosCompiler;
    use pipeline::template_management::create_source_directory_with_templates;

    // Create source directory structure and copy template files
    let src_dir = create_source_directory_with_templates(output_dir, implementation)?;

    // Run compiler passes (validation, canonicalization, etc.)
    let compiler = EthosCompiler::new();
    ir = compiler.run_compiler_passes(ir, output_dir)?;

    // Setup project files (Cargo.toml, README, etc.)
    setup_project_files(output_dir, version, implementation)?;

    // Run semantic analysis on the IR
    let project_root = find_project_root()?;
    let compiler_ctx = analyze_implementation(implementation, ir, version, project_root)?;

    // Generate code
    generate_into(&src_dir, &compiler_ctx)?;

    println!("Compilation completed successfully.");
    Ok(())
}

/// Get the latest known version for a given implementation
///
/// Returns the most recent stable version for each implementation.
/// These versions should be updated as new stable releases become available.
fn get_latest_version_for_implementation(implementation: &Implementation) -> &'static str {
    match implementation {
        Implementation::BitcoinCore => "v30.2.0",
        Implementation::CoreLightning => "v25.09.1",
        Implementation::Lnd => "v0.20.0",
        Implementation::RustLightning => "v0.1.0",
    }
}
