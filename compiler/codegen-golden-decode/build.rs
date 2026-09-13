//! Regenerates `responses.rs` from canonical IR whenever this crate or the IR changes.

use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use ethos_ir::{ProtocolIR, RpcDef};
use ethos_types::ProtocolVersion;

fn main() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let workspace_root = ethos_path::workspace_root_from_manifest(&manifest_dir, 2);

    let ir_path = ethos_path::canonical_bitcoin_ir_path(&workspace_root);
    println!("cargo:rerun-if-changed={}", ir_path.display());
    println!("cargo:rerun-if-changed={}", manifest_dir.join("../codegen").display());

    let ir = ProtocolIR::from_file(&ir_path)
        .unwrap_or_else(|e| panic!("load IR {}: {e}", ir_path.display()));
    let methods: Vec<RpcDef> = ir.get_rpc_methods().into_iter().cloned().collect();

    let version = ProtocolVersion::from_str("30.0.0").expect("protocol version");
    let gen = ethos_codegen::generators::VersionSpecificResponseTypeGenerator::new(
        version,
        "bitcoin_core".to_string(),
    );
    let files = gen.generate(&methods).expect("generate responses");
    let responses = files
        .into_iter()
        .find(|(name, _)| name == "responses.rs")
        .expect("responses.rs in codegen output")
        .1;

    let dest = out_dir.join("generated_responses.rs");
    fs::write(&dest, responses).unwrap_or_else(|e| panic!("write {}: {e}", dest.display()));
}
