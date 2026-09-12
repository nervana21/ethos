#!/usr/bin/env bash
# Live schema-oracle against docker-compose bitcoind (no host bitcoind / no host fuzz).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
COMPOSE_DIR="$ROOT/compiler/fuzz"
BITCOIND_CRATE="${ETHOS_BITCOIND_PATH:-$ROOT/../ethos-bitcoind}"
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

cd "$COMPOSE_DIR"
# Compose bind is ../../../ethos-bitcoind → /ethos-bitcoind (matches Cargo path dep).
# Override when ETHOS_BITCOIND_PATH is set to a non-sibling checkout.
export ETHOS_BITCOIND_HOST_PATH="$BITCOIND_CRATE"
docker compose up -d bitcoind

echo "waiting for bitcoind RPC…"
for _ in $(seq 1 90); do
  if docker compose exec -T bitcoind \
    bitcoin-cli -regtest -rpcuser=user -rpcpassword=pass getblockchaininfo >/dev/null 2>&1; then
    echo "bitcoind ready"
    break
  fi
  sleep 1
done

EXTRA_ARGS=$(printf '%q ' "$@")
# bash -c not -lc: login PATH drop /usr/local/cargo/bin.
PATH_EXPORT='export PATH="/usr/local/cargo/bin:${PATH:-/usr/bin:/bin}"'
case "$MODE" in
  continuous|fuzz)
    INNER="${PATH_EXPORT}; cd /work && cargo run -p ethos-schema-oracle -- --rpc-url http://bitcoind:18443 --rpc-user user --rpc-pass pass --continuous --save-rejects ${EXTRA_ARGS}"
    ;;
  smoke)
    INNER="${PATH_EXPORT}; cd /work && cargo run -p ethos-schema-oracle -- --rpc-url http://bitcoind:18443 --rpc-user user --rpc-pass pass ${EXTRA_ARGS}"
    ;;
  *)
    echo "usage: $0 continuous|smoke [schema-oracle args...]" >&2
    exit 2
    ;;
esac

docker compose run --rm --remove-orphans fuzz bash -c "set -euo pipefail; ${INNER}"
