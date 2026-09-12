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

Image: `bitcoin-rpc-fuzz:latest` (`compiler/fuzz/Dockerfile.fuzz`). Mounts: ethos root → `/work`; sibling `ethos-bitcoind` → `/ethos-bitcoind` (path dep of `ethos-schema-oracle`; override with `ETHOS_BITCOIND_PATH`).

Scripts use `bash -c` + explicit `PATH=/usr/local/cargo/bin:…` inside the container. Avoid `bash -lc` — login shells reset PATH and yield `cargo: command not found`.

## Live Δ hunt (host corpus bitcoind + Docker schema-oracle)

IR dump (`resources/ir/openrpc.json`, e.g. `v31.99.0-dev`) must match the live node.
Corpus `bitcoind` is Darwin Mach-O — cannot run inside Linux Docker — so the live
script spawns **host** `$BITCOIND_PATH` (default `corpus/bitcoin/build/bin/bitcoind`)
and points the Docker oracle at `http://host.docker.internal:18443`.

Do **not** use compose `bitcoin/bitcoin:30.2` for this hunt (version skew → false
`SchemaMismatch` on fields like `limitclustercount` / `coinbase_tx` / `inv_buckets`).

```sh
just schema-oracle-fuzz -- --duration-secs 60   # host corpus bitcoind + Docker continuous
just schema-oracle-smoke -- --rounds 32         # host corpus bitcoind + Docker smoke
# optional: BITCOIND_PATH=/path/to/bitcoind just schema-oracle-smoke -- --rounds 8
```

Override: `BITCOIND_PATH`, `BITCOIN_CLI_PATH`, `SCHEMA_ORACLE_DATADIR`,
`SCHEMA_ORACLE_RPC_PORT`, `SCHEMA_ORACLE_RPC_HOST`.

## Agent boundary

- Do **not** Shell-exec `cargo fuzz`, `docker exec … fuzz run`, or live fuzz “to verify”.
- Give copy-paste commands; verify from user pasteback only.
- Unit tests (`cargo test … schema_oracle_cov`) stay on host — not fuzz.
