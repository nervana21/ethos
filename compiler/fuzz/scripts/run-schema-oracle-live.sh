#!/usr/bin/env bash
# Live schema-oracle against docker-compose bitcoind (no host bitcoind / no host fuzz).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
COMPOSE_DIR="$ROOT/compiler/fuzz"
MODE="${1:-continuous}"
shift || true

cd "$COMPOSE_DIR"
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
