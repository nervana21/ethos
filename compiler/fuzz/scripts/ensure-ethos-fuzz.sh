#!/usr/bin/env bash
# Ensure persistent ethos-fuzz container (libFuzzer / cargo-fuzz only — never host).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
NAME="${ETHOS_FUZZ_CONTAINER:-ethos-fuzz}"
IMAGE="${ETHOS_FUZZ_IMAGE:-bitcoin-rpc-fuzz:latest}"
COMPOSE_DIR="$ROOT/compiler/fuzz"
BITCOIND_CRATE="${ETHOS_BITCOIND_PATH:-$ROOT/../ethos-bitcoind}"

if ! docker inspect "$NAME" >/dev/null 2>&1; then
  if ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
    echo "building $IMAGE (once)…"
    (cd "$COMPOSE_DIR" && docker compose build fuzz)
  fi
  VOLS=(-v "$ROOT:/work")
  if [[ -f "$BITCOIND_CRATE/Cargo.toml" ]]; then
    VOLS+=(-v "$BITCOIND_CRATE:/ethos-bitcoind:ro")
  fi
  docker run -d --name "$NAME" \
    "${VOLS[@]}" \
    -w /work/compiler/fuzz \
    -e CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}" \
    "$IMAGE" \
    sleep infinity
fi
docker start "$NAME" >/dev/null 2>&1 || true
echo "$NAME"
