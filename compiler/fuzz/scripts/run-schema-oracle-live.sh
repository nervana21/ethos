#!/usr/bin/env bash
# Live schema-oracle: Docker cargo run + host corpus bitcoind (IR-matched).
#
# Why host bitcoind: corpus build is Darwin Mach-O; OpenRPC/IR dump is from that
# tree (e.g. v31.99.0-dev). Image bitcoin/bitcoin:30.2 ≠ IR → false SchemaMismatch.
# Oracle still runs in Linux Docker; only the node is host-side.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
COMPOSE_DIR="$ROOT/compiler/fuzz"
BITCOIND_CRATE="${ETHOS_BITCOIND_PATH:-$ROOT/../ethos-bitcoind}"
BITCOIND="${BITCOIND_PATH:-$ROOT/corpus/bitcoin/build/bin/bitcoind}"
BITCOIN_CLI="${BITCOIN_CLI_PATH:-$(dirname "$BITCOIND")/bitcoin-cli}"
DATADIR="${SCHEMA_ORACLE_DATADIR:-$ROOT/outputs/schema_oracle_regtest}"
RPC_PORT="${SCHEMA_ORACLE_RPC_PORT:-18443}"
RPC_USER="${SCHEMA_ORACLE_RPC_USER:-user}"
RPC_PASS="${SCHEMA_ORACLE_RPC_PASS:-pass}"
# Docker Desktop Mac already has host.docker.internal; Linux CI may need host-gateway.
RPC_HOST="${SCHEMA_ORACLE_RPC_HOST:-host.docker.internal}"
MODE="${1:-continuous}"
shift || true
# just recipe `*args` keeps the `--` from `just foo -- --flag` — strip for clap.
if [[ "${1:-}" == "--" ]]; then
  shift
fi

if [[ ! -f "$BITCOIND_CRATE/Cargo.toml" ]]; then
  echo "missing ethos-bitcoind at $BITCOIND_CRATE (set ETHOS_BITCOIND_PATH or clone sibling)" >&2
  exit 1
fi
if [[ ! -x "$BITCOIND" ]]; then
  echo "missing runnable bitcoind at $BITCOIND (set BITCOIND_PATH; build corpus Core first)" >&2
  exit 1
fi
if [[ ! -x "$BITCOIN_CLI" ]]; then
  echo "missing runnable bitcoin-cli at $BITCOIN_CLI (set BITCOIN_CLI_PATH)" >&2
  exit 1
fi

cd "$COMPOSE_DIR"
# Compose bind is ../../../ethos-bitcoind → /ethos-bitcoind (matches Cargo path dep).
export ETHOS_BITCOIND_HOST_PATH="$BITCOIND_CRATE"

# Free host :RPC_PORT — old compose bitcoin:30.2 publishes 18443.
if docker compose ps --status running --services 2>/dev/null | grep -qx bitcoind; then
  echo "stopping compose bitcoind (bitcoin:30.2) so host corpus node can bind :${RPC_PORT}"
  docker compose stop bitcoind >/dev/null
fi

BITCOIND_PID=""
STARTED_BITCOIND=0
cleanup() {
  if [[ "$STARTED_BITCOIND" -eq 1 && -n "$BITCOIND_PID" ]]; then
    echo "stopping host corpus bitcoind pid=$BITCOIND_PID"
    kill "$BITCOIND_PID" 2>/dev/null || true
    wait "$BITCOIND_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

rpc_ready() {
  "$BITCOIN_CLI" -regtest -datadir="$DATADIR" \
    -rpcuser="$RPC_USER" -rpcpassword="$RPC_PASS" -rpcport="$RPC_PORT" \
    getblockchaininfo >/dev/null 2>&1
}

mkdir -p "$DATADIR"
if rpc_ready; then
  echo "reusing existing corpus bitcoind on :${RPC_PORT} (datadir=$DATADIR)"
else
  echo "spawn host corpus bitcoind: $BITCOIND"
  "$BITCOIND" -regtest -datadir="$DATADIR" \
    -rpcuser="$RPC_USER" -rpcpassword="$RPC_PASS" \
    -rpcport="$RPC_PORT" \
    -rpcbind=127.0.0.1 -rpcbind=0.0.0.0 \
    -rpcallowip=127.0.0.1 -rpcallowip=0.0.0.0/0 \
    -server -fallbackfee=0.00001 \
    -printtoconsole=0 &
  BITCOIND_PID=$!
  STARTED_BITCOIND=1

  echo "waiting for host bitcoind RPC on :${RPC_PORT}…"
  ready=0
  for _ in $(seq 1 90); do
    if rpc_ready; then
      ready=1
      break
    fi
    if ! kill -0 "$BITCOIND_PID" 2>/dev/null; then
      echo "bitcoind exited before ready (pid=$BITCOIND_PID)" >&2
      exit 1
    fi
    sleep 1
  done
  if [[ "$ready" -ne 1 ]]; then
    echo "bitcoind RPC not ready after 90s" >&2
    exit 1
  fi
  echo "host corpus bitcoind ready"
fi

SUBVER=$("$BITCOIN_CLI" -regtest -datadir="$DATADIR" \
  -rpcuser="$RPC_USER" -rpcpassword="$RPC_PASS" -rpcport="$RPC_PORT" \
  getnetworkinfo 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin).get("subversion",""))' || true)
echo "bitcoind subversion: ${SUBVER:-unknown} (IR dump expects corpus tip, not v30.2)"

EXTRA_ARGS=$(printf '%q ' "$@")
# bash -c not -lc: login PATH drop /usr/local/cargo/bin.
PATH_EXPORT='export PATH="/usr/local/cargo/bin:${PATH:-/usr/bin:/bin}"'
RPC_URL="http://${RPC_HOST}:${RPC_PORT}"
case "$MODE" in
  continuous|fuzz)
    INNER="${PATH_EXPORT}; cd /work && cargo run -p ethos-schema-oracle -- --rpc-url ${RPC_URL} --rpc-user ${RPC_USER} --rpc-pass ${RPC_PASS} --continuous --save-rejects ${EXTRA_ARGS}"
    ;;
  smoke)
    INNER="${PATH_EXPORT}; cd /work && cargo run -p ethos-schema-oracle -- --rpc-url ${RPC_URL} --rpc-user ${RPC_USER} --rpc-pass ${RPC_PASS} ${EXTRA_ARGS}"
    ;;
  *)
    echo "usage: $0 continuous|smoke [schema-oracle args...]" >&2
    exit 2
    ;;
esac

# --no-deps: do not start compose bitcoin:30.2 via depends_on.
# host.docker.internal via compose extra_hosts (Compose v5 has no run --add-host).
docker compose run --rm --remove-orphans --no-deps \
  fuzz bash -c "set -euo pipefail; ${INNER}"
