# Ethos fuzz — Docker only

**Hard rule:** never run `cargo fuzz` / libFuzzer / continuous schema-oracle hunt on the macOS host. All fuzz goes through Linux Docker (`ethos-fuzz` persistent box or `compiler/fuzz` compose).

## Why

Host Darwin + ASAN/libFuzzer deadlocks. Corpus policy matches Bitcoin Core / Floresta: agent instructs; human runs in standalone terminal; prove via pasteback.

## Persistent box (`ethos-fuzz`)

```sh
# from ethos repo root
./compiler/fuzz/scripts/ensure-ethos-fuzz.sh
just schema-oracle-cov                  # continuous cov-guided (default)
just schema-oracle-cov -- -max_total_time=60
just schema-oracle-cov -- -runs=500
```

Image: `bitcoin-rpc-fuzz:latest` (`compiler/fuzz/Dockerfile.fuzz`). Mount: ethos root → `/work`.

Scripts use `bash -c` + explicit `PATH=/usr/local/cargo/bin:…` inside the container. Avoid `bash -lc` — login shells reset PATH and yield `cargo: command not found`.

## Live Δ hunt (bitcoind + schema-oracle)

```sh
just schema-oracle-fuzz -- --duration-secs 60   # docker bitcoind + continuous
just schema-oracle-smoke -- --rounds 32         # docker bitcoind + smoke
```

## Agent boundary

- Do **not** Shell-exec `cargo fuzz`, `docker exec … fuzz run`, or live fuzz “to verify”.
- Give copy-paste commands; verify from user pasteback only.
- Unit tests (`cargo test … schema_oracle_cov`) stay on host — not fuzz.
