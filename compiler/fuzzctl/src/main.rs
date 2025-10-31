//! Standalone fuzzing runner for Ethos
//!
//! This tool provides a command-line interface for running fuzzing operations
//! using Docker Compose, managing fuzzing artifacts, and generating feedback reports.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![deny(clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clap::{Parser, Subcommand};

#[derive(Debug)]
enum FuzzctlError {
    Io(std::io::Error),
    Json(serde_json::Error),
    CtrlC(ctrlc::Error),
    Custom(String),
}

/// Display implementation for FuzzctlError.
impl std::fmt::Display for FuzzctlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FuzzctlError::Io(e) => write!(f, "IO error: {}", e),
            FuzzctlError::Json(e) => write!(f, "JSON error: {}", e),
            FuzzctlError::CtrlC(e) => write!(f, "CtrlC error: {}", e),
            FuzzctlError::Custom(msg) => write!(f, "{}", msg),
        }
    }
}

/// Error trait implementation for FuzzctlError.
impl std::error::Error for FuzzctlError {}

/// Convert from std::io::Error to FuzzctlError.
impl From<std::io::Error> for FuzzctlError {
    fn from(e: std::io::Error) -> Self { FuzzctlError::Io(e) }
}

/// Convert from serde_json::Error to FuzzctlError.
impl From<serde_json::Error> for FuzzctlError {
    fn from(e: serde_json::Error) -> Self { FuzzctlError::Json(e) }
}

/// Convert from ctrlc::Error to FuzzctlError.
impl From<ctrlc::Error> for FuzzctlError {
    fn from(e: ctrlc::Error) -> Self { FuzzctlError::CtrlC(e) }
}

/// Command-line interface configuration for fuzzctl.
#[derive(Parser, Debug)]
#[command(
    name = "fuzzctl",
    about = "Standalone fuzzing runner for Ethos",
    version,
    disable_help_subcommand = false
)]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,
    /// Directory containing fuzz infra (e.g., docker-compose.yml)
    #[arg(long, default_value = "compiler/fuzz", global = true)]
    fuzz_dir: PathBuf,
    /// Service name to run (docker-compose)
    #[arg(long, default_value = "fuzz-target", global = true)]
    service: String,
    /// Docker compose profile to enable
    #[arg(long, default_value = "fuzz", global = true)]
    profile: String,
    /// Output directory to place artifacts and feedback file
    #[arg(long, default_value = "outputs/generated", global = true)]
    output_dir: PathBuf,
}

/// Available fuzzctl commands.
#[derive(Subcommand, Debug)]
enum Commands {
    /// Run the fuzzing service (local docker or dry-run)
    Run {
        #[arg(long)]
        timeout_secs: Option<u64>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        detach: bool,
    },
    /// Generate a quick seed corpus
    GenerateCorpus,
    /// Analyze existing corpus/crashes for safe feedback
    Analyze,
    /// Export a fuzz_feedback.json file to output dir
    ExportFeedback {
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Stream logs from the running compose service
    Logs {
        #[arg(long, default_value = "false")]
        follow: bool,
    },
    /// Collect artifacts (corpus/crashes) to output_dir
    Collect,
}

/// Main entry point for the fuzzctl application.
fn main() -> Result<(), FuzzctlError> {
    let cli = Cli::parse();

    match cli.cmd {
        Commands::Run { timeout_secs, dry_run, detach } => {
            println!(
                "[fuzzctl] Running in {:?} (service: {}, profile: {}, dry_run: {}, detach: {})",
                cli.fuzz_dir, cli.service, cli.profile, dry_run, detach
            );
            std::fs::create_dir_all(&cli.output_dir)?;
            let logs_dir = cli.output_dir.join("fuzz-logs");
            std::fs::create_dir_all(&logs_dir)?;

            let start = std::time::Instant::now();
            let exit_code = if dry_run {
                0
            } else if detach {
                run_docker_detached(&cli.fuzz_dir, &cli.service, &cli.profile, &cli.output_dir)?;
                0
            } else {
                run_docker_capture(
                    &cli.fuzz_dir,
                    &cli.service,
                    &cli.profile,
                    timeout_secs,
                    &logs_dir,
                )?
            };
            let duration = start.elapsed().as_secs();

            // Count corpus and crashes from the dynamic output directory
            let (corpus_entries, crash_count) = count_artifacts(&cli.output_dir, &cli.service);

            write_feedback(
                &cli.output_dir,
                duration,
                exit_code,
                corpus_entries as u64,
                crash_count as u64,
            )?;
            println!(
                "[fuzzctl] Wrote fuzz_feedback.json to {:?}",
                cli.output_dir.join("fuzz_feedback.json")
            );
        }
        Commands::Logs { follow } => {
            stream_logs(&cli.fuzz_dir, &cli.output_dir, &cli.service, follow)?;
        }
        Commands::Collect => {
            collect_artifacts(&cli.fuzz_dir, &cli.output_dir, &cli.service)?;
        }
        Commands::GenerateCorpus => {
            println!("[fuzzctl] generate-corpus (stub)");
        }
        Commands::Analyze => {
            println!("[fuzzctl] analyze (stub)");
        }
        Commands::ExportFeedback { out } => {
            let out_dir = out.unwrap_or_else(|| cli.output_dir.clone());
            std::fs::create_dir_all(&out_dir)?;
            write_feedback(&out_dir, 0, 0, 0, 0)?;
            println!(
                "[fuzzctl] exported fuzz_feedback.json to {:?}",
                out_dir.join("fuzz_feedback.json")
            );
        }
    }

    Ok(())
}

/// Set environment variables for dynamic Docker paths.
fn set_fuzz_env_vars(_out_dir: &Path, service: &str) {
    let target = determine_fuzz_target(service);
    let target_normalized = target.replace("-", "_");

    let corpus_path = format!("/work/outputs/generated/fuzz/{}/corpus", target_normalized);
    let crashes_path = format!("/work/outputs/generated/fuzz/{}/crashes", target_normalized);

    std::env::set_var("FUZZ_CORPUS_PATH", corpus_path);
    std::env::set_var("FUZZ_CRASHES_PATH", crashes_path);
}

/// Run Docker Compose service and capture output with timeout.
fn run_docker_capture(
    fuzz_dir: &Path,
    service: &str,
    profile: &str,
    timeout_secs: Option<u64>,
    logs_dir: &Path,
) -> Result<i32, FuzzctlError> {
    use std::fs::File;
    use std::io::{Read, Write};
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    let compose_file_rel = fuzz_dir.join("docker-compose.yml");
    if !compose_file_rel.exists() {
        return Err(FuzzctlError::Custom(format!(
            "docker-compose.yml not found at {:?}",
            compose_file_rel
        )));
    }
    let compose_file = std::fs::canonicalize(&compose_file_rel)?;

    let (bin, prefix) = docker_compose_cmd();
    let mut cmd = Command::new(bin);
    for a in prefix {
        cmd.arg(a);
    }
    cmd.arg("-f")
        .arg(&compose_file)
        .arg("--profile")
        .arg(profile)
        .arg("up")
        .arg("--build")
        .arg("--abort-on-container-exit")
        .arg(service)
        .current_dir(fuzz_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Warn if user selected the inert 'fuzz' service
    if service == "fuzz" {
        eprintln!("[fuzzctl] Warning: service 'fuzz' is inert (tails). Prefer 'fuzz-target' with profile 'fuzz'.");
    }

    let mut child = cmd.spawn()?;

    // Install Ctrl-C handler to request shutdown
    let terminate = Arc::new(AtomicBool::new(false));
    #[cfg(feature = "signals")]
    {
        let term_flag = terminate.clone();
        ctrlc::set_handler(move || {
            term_flag.store(true, Ordering::SeqCst);
        })?;
    }

    // Prepare log files
    let mut out_file = File::create(logs_dir.join("fuzz_stdout.log"))?;
    let mut err_file = File::create(logs_dir.join("fuzz_stderr.log"))?;

    // Pipe readers
    let mut child_stdout = child.stdout.take().expect("stdout piped");
    let mut child_stderr = child.stderr.take().expect("stderr piped");

    // Stream logs concurrently
    let out_handle = std::thread::spawn(move || -> Result<(), FuzzctlError> {
        let mut buf = [0u8; 8192];
        loop {
            let n = child_stdout.read(&mut buf)?;
            if n == 0 {
                break;
            }
            out_file.write_all(&buf[..n])?;
        }
        Ok(())
    });
    let err_handle = std::thread::spawn(move || -> Result<(), FuzzctlError> {
        let mut buf = [0u8; 8192];
        loop {
            let n = child_stderr.read(&mut buf)?;
            if n == 0 {
                break;
            }
            err_file.write_all(&buf[..n])?;
        }
        Ok(())
    });

    // Wait with timeout (0 means infinite)
    let timeout = timeout_secs.unwrap_or(600);
    let status_code = if timeout == 0 {
        // Infinite wait until the process exits or termination requested
        loop {
            if let Some(status) = child.try_wait()? {
                break status.code().unwrap_or(1);
            }
            if terminate.load(Ordering::SeqCst) {
                let _ = child.kill();
                let _ = child.wait();
                break 130; // interrupted
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    } else {
        let deadline = Instant::now() + Duration::from_secs(timeout);
        loop {
            if let Some(status) = child.try_wait()? {
                break status.code().unwrap_or(1);
            }
            if terminate.load(Ordering::SeqCst) {
                let _ = child.kill();
                let _ = child.wait();
                break 130; // interrupted
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                break 124; // conventional timeout code
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    };

    // Ensure log threads finish
    let _ = out_handle.join().unwrap_or(Ok(()));
    let _ = err_handle.join().unwrap_or(Ok(()));

    Ok(status_code)
}

/// Run Docker Compose service in detached mode.
fn run_docker_detached(
    fuzz_dir: &Path,
    service: &str,
    profile: &str,
    out_dir: &Path,
) -> Result<(), FuzzctlError> {
    // Set environment variables for dynamic paths
    set_fuzz_env_vars(out_dir, service);

    let compose_file_rel = fuzz_dir.join("docker-compose.yml");
    if !compose_file_rel.exists() {
        return Err(FuzzctlError::Custom(format!(
            "docker-compose.yml not found at {:?}",
            compose_file_rel
        )));
    }
    let compose_file = std::fs::canonicalize(&compose_file_rel)?;

    let (bin, prefix) = docker_compose_cmd();
    let mut cmd = Command::new(bin);
    for a in prefix {
        cmd.arg(a);
    }
    cmd.arg("-f")
        .arg(&compose_file)
        .arg("--profile")
        .arg(profile)
        .arg("up")
        .arg("-d")
        .arg("--build")
        .arg(service)
        .current_dir(fuzz_dir);

    let output = cmd.output()?;
    if !output.status.success() {
        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let stderr_str = String::from_utf8_lossy(&output.stderr);
        return Err(FuzzctlError::Custom(format!(
            "compose up -d failed (code {:?})\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            stdout_str,
            stderr_str
        )));
    }

    // Query container ids for the service
    let (bin, prefix) = docker_compose_cmd();
    let ps_out = std::process::Command::new(&bin)
        .args(prefix.clone())
        .arg("-f")
        .arg(&compose_file)
        .arg("--profile")
        .arg(profile)
        .arg("ps")
        .arg("-q")
        .arg(service)
        .current_dir(fuzz_dir)
        .output()?;
    let container_ids: Vec<String> = String::from_utf8_lossy(&ps_out.stdout)
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let state_dir = out_dir.join("fuzzctl");
    std::fs::create_dir_all(&state_dir)?;
    let state_path = state_dir.join("state.json");
    let state = serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "compose": compose_file.to_string_lossy(),
        "service": service,
        "profile": profile,
        "containers": container_ids,
    });
    std::fs::write(state_path, serde_json::to_string_pretty(&state)?)?;
    println!(
        "[fuzzctl] Detached run started. State written to {}/fuzzctl/state.json",
        out_dir.to_string_lossy()
    );
    Ok(())
}

/// Stream logs from the running Docker Compose service.
fn stream_logs(
    fuzz_dir: &Path,
    out_dir: &Path,
    service: &str,
    follow: bool,
) -> Result<(), FuzzctlError> {
    // If we have container ids from a detached run, prefer docker logs on specific container(s)
    let state_path = out_dir.join("fuzzctl/state.json");
    if let Ok(state_raw) = std::fs::read_to_string(&state_path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&state_raw) {
            if let Some(ids) = val.get("containers").and_then(|v| v.as_array()) {
                let ids: Vec<String> =
                    ids.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
                if !ids.is_empty() {
                    // When using plain docker logs, ignore compose and stream specific container(s)
                    if follow {
                        // Spawn one child per container and stream concurrently
                        use std::process::Stdio;
                        let mut children = Vec::new();
                        for id in ids.iter() {
                            let mut cmd = Command::new("docker");
                            cmd.arg("logs")
                                .arg("-f")
                                .arg(id)
                                .stdout(Stdio::inherit())
                                .stderr(Stdio::inherit());
                            let child = cmd.spawn()?;
                            children.push(child);
                        }
                        // Wait for all children to exit
                        for mut child in children {
                            let _ = child.wait();
                        }
                    } else {
                        for id in ids {
                            let status = Command::new("docker").arg("logs").arg(id).status()?;
                            if !status.success() {
                                return Err(FuzzctlError::Custom("docker logs failed".to_string()));
                            }
                        }
                    }
                    return Ok(());
                }
            }
        }
    }

    // Fallback to compose logs for the specified service
    let compose_file = std::fs::canonicalize(fuzz_dir.join("docker-compose.yml"))?;
    let (bin, prefix) = docker_compose_cmd();
    let mut cmd = Command::new(bin);
    for a in prefix {
        cmd.arg(a);
    }
    cmd.arg("-f").arg(&compose_file).arg("logs").arg(service);
    if follow {
        cmd.arg("-f");
    }
    let status = cmd.status()?;
    if !status.success() {
        return Err(FuzzctlError::Custom("logs command failed".to_string()));
    }
    Ok(())
}

/// Determine the fuzzing target from service name or environment.
fn determine_fuzz_target(service: &str) -> String {
    // Use FUZZ_TARGET environment variable if set, otherwise use service name
    std::env::var("FUZZ_TARGET").unwrap_or_else(|_| service.to_string())
}

/// Build dynamic paths for fuzzing artifacts.
fn build_fuzz_paths(
    fuzz_dir: &Path,
    out_dir: &Path,
    service: &str,
) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let target = determine_fuzz_target(service);
    let target_normalized = target.replace("-", "_");

    // Source paths in fuzz directory (legacy structure)
    let src_corpus = fuzz_dir.join(format!("{}-corpus", target_normalized));
    let src_crashes = fuzz_dir.join(format!("{}-crashes", target_normalized));

    // Destination paths in outputs/generated/fuzz/target/
    let dst_corpus = out_dir.join("fuzz").join(&target_normalized).join("corpus");
    let dst_crashes = out_dir.join("fuzz").join(&target_normalized).join("crashes");

    (src_corpus, src_crashes, dst_corpus, dst_crashes)
}

/// Collect fuzzing artifacts (corpus/crashes) from Docker containers.
fn collect_artifacts(fuzz_dir: &Path, out_dir: &Path, service: &str) -> Result<(), FuzzctlError> {
    use std::fs;

    use walkdir::WalkDir;
    let (src_corpus, src_crashes, dst_corpus, dst_crashes) =
        build_fuzz_paths(fuzz_dir, out_dir, service);
    fs::create_dir_all(&dst_corpus)?;
    fs::create_dir_all(&dst_crashes)?;
    let allow_untrusted =
        std::env::var("ETHOS_ALLOW_UNTRUSTED_CORPUS").ok().unwrap_or_default() == "1";
    if !allow_untrusted {
        return Err(FuzzctlError::Custom(
            "Refusing to collect artifacts: set ETHOS_ALLOW_UNTRUSTED_CORPUS=1 to proceed"
                .to_string(),
        ));
    }

    // Local copy first
    for (src, dst) in
        [(src_corpus.clone(), dst_corpus.clone()), (src_crashes.clone(), dst_crashes.clone())]
    {
        if src.exists() {
            for entry in WalkDir::new(&src).into_iter().flatten() {
                let path = entry.path();
                if path.is_file() {
                    let rel = path.strip_prefix(&src).expect("Failed to strip prefix");
                    let target = dst.join(rel);
                    if let Some(parent) = target.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    std::fs::copy(path, target.clone()).ok();
                }
            }
        }
    }

    // If nothing was found locally, attempt docker cp from recorded container(s)
    let (_, _, dst_corpus, dst_crashes) = build_fuzz_paths(fuzz_dir, out_dir, service);
    let dst_has_files = count_files(&dst_corpus) + count_files(&dst_crashes) > 0;
    if !dst_has_files {
        let state_path = out_dir.join("fuzzctl/state.json");
        if let Ok(state_raw) = std::fs::read_to_string(&state_path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&state_raw) {
                if let Some(ids_val) = val.get("containers").and_then(|v| v.as_array()) {
                    let ids: Vec<String> =
                        ids_val.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
                    if !ids.is_empty() {
                        let tmp_dir = out_dir.join("host_tmp");
                        fs::create_dir_all(&tmp_dir)?;
                        // For each container, copy its corpus/crashes into a namespaced temp dir
                        for id in ids.iter() {
                            let container_tmp = tmp_dir.join(id);
                            let corpus_dst = container_tmp.join("corpus");
                            let crashes_dst = container_tmp.join("crashes");
                            let _ = std::fs::remove_dir_all(&corpus_dst);
                            let _ = std::fs::remove_dir_all(&crashes_dst);
                            let _ = std::fs::create_dir_all(&corpus_dst);
                            let _ = std::fs::create_dir_all(&crashes_dst);
                            let target = determine_fuzz_target(service);
                            let target_normalized = target.replace("-", "_");
                            let _ = Command::new("docker")
                                .arg("cp")
                                .arg(format!(
                                    "{}:/work/compiler/fuzz/{}-corpus",
                                    id, target_normalized
                                ))
                                .arg(&corpus_dst)
                                .status();
                            let _ = Command::new("docker")
                                .arg("cp")
                                .arg(format!(
                                    "{}:/work/compiler/fuzz/{}-crashes",
                                    id, target_normalized
                                ))
                                .arg(&crashes_dst)
                                .status();
                            // Merge into final destination
                            let (_, _, dst_corpus, dst_crashes) =
                                build_fuzz_paths(fuzz_dir, out_dir, service);
                            for (src, dst) in [(corpus_dst, dst_corpus), (crashes_dst, dst_crashes)]
                            {
                                for entry in WalkDir::new(&src).into_iter().flatten() {
                                    let path = entry.path();
                                    if path.is_file() {
                                        let rel = path
                                            .strip_prefix(&src)
                                            .expect("Failed to strip prefix");
                                        // Prefix by container id to avoid collisions
                                        let target = dst.join(id).join(rel);
                                        if let Some(parent) = target.parent() {
                                            fs::create_dir_all(parent)?;
                                        }
                                        let _ = std::fs::copy(path, target);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    println!("[fuzzctl] Collected artifacts into {}", out_dir.to_string_lossy());
    Ok(())
}

/// Determine the correct Docker Compose command to use.
fn docker_compose_cmd() -> (String, Vec<String>) {
    if which::which("docker-compose").is_ok() {
        ("docker-compose".to_string(), vec![])
    } else {
        ("docker".to_string(), vec!["compose".to_string()])
    }
}

/// Feedback data structure for fuzzing results.
#[derive(serde::Serialize)]
struct Feedback {
    timestamp: String,
    duration_seconds: u64,
    exit_code: i32,
    corpus_entries: u64,
    crash_count: u64,
    findings: Vec<serde_json::Value>,
    logs: Logs,
    summary: String,
}

/// Log data structure for Docker output.
#[derive(serde::Serialize)]
struct Logs {
    docker_stdout: String,
    docker_stderr: String,
}

/// Write fuzzing feedback to a JSON file.
fn write_feedback(
    out_dir: &Path,
    duration_seconds: u64,
    exit_code: i32,
    corpus_entries: u64,
    crash_count: u64,
) -> Result<(), FuzzctlError> {
    let feedback = Feedback {
        timestamp: chrono::Utc::now().to_rfc3339(),
        duration_seconds,
        exit_code,
        corpus_entries,
        crash_count,
        findings: vec![
            serde_json::json!({"type": "summary", "exit": exit_code, "corpus": corpus_entries, "crashes": crash_count}),
        ],
        logs: Logs { docker_stdout: "".to_string(), docker_stderr: "".to_string() },
        summary: format!(
            "status={} corpus={} crashes={} duration_s={}",
            exit_code, corpus_entries, crash_count, duration_seconds
        ),
    };
    let path = out_dir.join("fuzz_feedback.json");
    let json = serde_json::to_string_pretty(&feedback)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Count corpus entries and crashes in the output directory.
fn count_artifacts(out_dir: &Path, service: &str) -> (usize, usize) {
    let allow_untrusted = std::env::var("ETHOS_ALLOW_UNTRUSTED_CORPUS").is_ok();
    if !allow_untrusted {
        return (0, 0);
    }
    let target = determine_fuzz_target(service);
    let target_normalized = target.replace("-", "_");
    let corpus = out_dir.join("fuzz").join(&target_normalized).join("corpus");
    let crashes = out_dir.join("fuzz").join(&target_normalized).join("crashes");
    (count_files(&corpus), count_files(&crashes))
}

/// Recursively count files in a directory.
fn count_files(dir: &std::path::Path) -> usize {
    fn walk(p: &std::path::Path, acc: &mut usize) {
        if let Ok(read) = std::fs::read_dir(p) {
            for entry in read.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, acc);
                } else {
                    *acc += 1;
                }
            }
        }
    }
    if !dir.exists() {
        return 0;
    }
    let mut total = 0;
    walk(dir, &mut total);
    total
}
