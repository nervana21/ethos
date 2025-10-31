// create temp dir for outputs and run fuzzctl commands
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn dry_run_writes_feedback() {
    let tmp = tempdir().unwrap();
    let out = tmp.path().join("gen");
    std::fs::create_dir_all(&out).unwrap();
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ethos-fuzzctl"));
    cmd.args([
        "--fuzz-dir",
        "compiler/fuzz",
        "--output-dir",
        out.to_str().unwrap(),
        "run",
        "--dry-run",
        "--timeout-secs",
        "0",
    ]);
    cmd.assert().success();
    let fb = out.join("fuzz_feedback.json");
    assert!(fb.exists(), "expected feedback at {:?}", fb);
}

#[test]
fn collect_is_gated_by_env() {
    let tmp = tempdir().unwrap();
    let out = tmp.path().join("gen");
    std::fs::create_dir_all(&out).unwrap();
    // Without env var => fail
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ethos-fuzzctl"));
    cmd.args(["--fuzz-dir", "compiler/fuzz", "--output-dir", out.to_str().unwrap(), "collect"]);
    cmd.assert().failure().stderr(predicate::str::contains("ETHOS_ALLOW_UNTRUSTED_CORPUS=1"));
    // With env var => succeed
    let mut cmd2 = Command::new(assert_cmd::cargo::cargo_bin!("ethos-fuzzctl"));
    cmd2.env("ETHOS_ALLOW_UNTRUSTED_CORPUS", "1");
    cmd2.args(["--fuzz-dir", "compiler/fuzz", "--output-dir", out.to_str().unwrap(), "collect"]);
    cmd2.assert().success();
}

#[test]
fn export_feedback_creates_file() {
    let tmp = tempdir().unwrap();
    let out = tmp.path().join("gen");
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ethos-fuzzctl"));
    cmd.args(["export-feedback", "--out"]).arg(&out);
    cmd.assert().success();
    assert!(out.join("fuzz_feedback.json").exists());
}
