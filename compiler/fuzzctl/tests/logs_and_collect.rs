use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;

use assert_cmd::prelude::*;

fn make_executable(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).unwrap();
    }
}

fn write_state_json(out_dir: &Path, containers: &[&str]) {
    let state_dir = out_dir.join("fuzzctl");
    fs::create_dir_all(&state_dir).unwrap();
    let state_path = state_dir.join("state.json");
    let val = serde_json::json!({
        "timestamp": "2024-01-01T00:00:00Z",
        "compose": "/dev/null",
        "service": "fuzz-target",
        "profile": "fuzz",
        "containers": containers,
    });
    fs::write(state_path, serde_json::to_vec_pretty(&val).unwrap()).unwrap();
}

#[test]
fn logs_follow_streams_all_containers_via_docker_logs() {
    let temp = tempfile::tempdir().unwrap();
    let out_dir = temp.path().join("out");
    fs::create_dir_all(&out_dir).unwrap();
    write_state_json(&out_dir, &["c1", "c2"]);

    // Create a fake docker that records invocations
    let bin_dir = temp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let inv_log = temp.path().join("docker_invocations.txt");
    let docker_path = bin_dir.join("docker");
    {
        let mut f = fs::File::create(&docker_path).unwrap();
        // Simple POSIX shell script that logs args and exits 0
        writeln!(f, "#!/bin/sh").unwrap();
        // record the subcommand and args
        writeln!(f, "echo \"$@\" >> {}", inv_log.display()).unwrap();
        // print something and exit
        writeln!(f, "exit 0").unwrap();
    }
    make_executable(&docker_path);

    // Run fuzzctl logs --follow (should spawn docker logs -f for each id)
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ethos-fuzzctl"));
    cmd.arg("logs")
        .arg("--follow")
        .arg("--output-dir")
        .arg(&out_dir)
        .env("PATH", format!("{}:{}", bin_dir.display(), std::env::var("PATH").unwrap()));
    cmd.assert().success();

    let invocations = fs::read_to_string(inv_log).unwrap();
    assert!(invocations.contains("logs -f c1"));
    assert!(invocations.contains("logs -f c2"));
}

#[test]
fn collect_copies_from_all_recorded_containers() {
    let temp = tempfile::tempdir().unwrap();
    let out_dir = temp.path().join("out");
    fs::create_dir_all(&out_dir).unwrap();
    write_state_json(&out_dir, &["alpha", "beta"]);

    // Fake container filesystems under tmpfs/<id>/work/compiler/fuzz/{fuzz_target-corpus,fuzz_target-crashes}
    let tmpfs = temp.path().join("tmpfs");
    let alpha_corpus = tmpfs.join("alpha/work/compiler/fuzz/fuzz_target-corpus");
    let alpha_crashes = tmpfs.join("alpha/work/compiler/fuzz/fuzz_target-crashes");
    let beta_corpus = tmpfs.join("beta/work/compiler/fuzz/fuzz_target-corpus");
    let beta_crashes = tmpfs.join("beta/work/compiler/fuzz/fuzz_target-crashes");
    fs::create_dir_all(&alpha_corpus).unwrap();
    fs::create_dir_all(&alpha_crashes).unwrap();
    fs::create_dir_all(&beta_corpus).unwrap();
    fs::create_dir_all(&beta_crashes).unwrap();
    fs::write(alpha_corpus.join("a1"), b"alpha-corpus").unwrap();
    fs::write(alpha_crashes.join("ax"), b"alpha-crash").unwrap();
    fs::write(beta_corpus.join("b1"), b"beta-corpus").unwrap();
    fs::write(beta_crashes.join("bx"), b"beta-crash").unwrap();

    // Fake docker that implements `docker cp <id>:<path> <dst>` by copying from tmpfs
    let bin_dir = temp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let docker_path = bin_dir.join("docker");
    {
        let mut f = fs::File::create(&docker_path).unwrap();
        writeln!(f, "#!/bin/sh").unwrap();
        writeln!(f, "cmd=$1; shift").unwrap();
        // Only support `cp`
        writeln!(f, "if [ \"$cmd\" = cp ]; then").unwrap();
        writeln!(f, "  src=$1; dst=$2").unwrap();
        writeln!(f, "  id=`echo $src | cut -d: -f1`").unwrap();
        writeln!(f, "  sub=`echo $src | cut -d: -f2-`").unwrap();
        // Map to tmpfs/<id>/<sub>
        writeln!(f, "  real=\"{}/$id/$sub\"", tmpfs.display()).unwrap();
        writeln!(f, "  mkdir -p \"$dst\" 2>/dev/null || true").unwrap();
        writeln!(f, "  cp -R \"$real\"/* \"$dst\" 2>/dev/null || true").unwrap();
        writeln!(f, "  exit 0").unwrap();
        writeln!(f, "fi").unwrap();
        writeln!(f, "echo unsupported >&2").unwrap();
        writeln!(f, "exit 1").unwrap();
    }
    make_executable(&docker_path);

    // Ensure no local compiler/fuzz copies exist so code uses docker cp path
    let fuzz_dir = temp.path().join("fuzz");
    fs::create_dir_all(&fuzz_dir).unwrap();

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("ethos-fuzzctl"));
    cmd.arg("collect")
        .arg("--output-dir")
        .arg(&out_dir)
        .arg("--fuzz-dir")
        .arg(&fuzz_dir)
        .env("ETHOS_ALLOW_UNTRUSTED_CORPUS", "1")
        .env("PATH", format!("{}:{}", bin_dir.display(), std::env::var("PATH").unwrap()));
    cmd.assert().success();

    // Expect files under out_dir/fuzz/fuzz_target/{corpus,crashes}/<id>/...
    let out_corpus = out_dir.join("fuzz").join("fuzz_target").join("corpus");
    let out_crashes = out_dir.join("fuzz").join("fuzz_target").join("crashes");
    assert!(out_corpus.join("alpha/a1").exists());
    assert!(out_corpus.join("beta/b1").exists());
    assert!(out_crashes.join("alpha/ax").exists());
    assert!(out_crashes.join("beta/bx").exists());
}
