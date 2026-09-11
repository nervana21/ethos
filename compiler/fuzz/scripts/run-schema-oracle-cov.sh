#!/usr/bin/env bash
# Coverage-guided schema_oracle libFuzzer — Docker only (Linux container).
# Never run cargo fuzz on the macOS host.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
NAME="${ETHOS_FUZZ_CONTAINER:-ethos-fuzz}"
SEED_SRC="$ROOT/resources/testdata/schema_oracle_cov_seeds"
CORPUS="$ROOT/compiler/fuzz/fuzz/corpus/schema_oracle"

mkdir -p "$CORPUS"
if [ -d "$SEED_SRC" ]; then
  cp -n "$SEED_SRC"/*.bin "$CORPUS"/ 2>/dev/null || cp "$SEED_SRC"/*.bin "$CORPUS"/ || true
fi

"$ROOT/compiler/fuzz/scripts/ensure-ethos-fuzz.sh" >/dev/null

# Extra args after script name forward to libFuzzer.
# Default continuous (no -max_total_time). Timed: -- -max_total_time=60
EXTRA=()
if [ "$#" -gt 0 ]; then
  EXTRA=("$@")
else
  EXTRA=(-max_len=256)
fi
QUOTED=$(printf '%q ' "${EXTRA[@]}")

# Use bash -c (not -lc): login shells reset PATH and drop /usr/local/cargo/bin.
docker exec -i "$NAME" bash -c "
set -euo pipefail
export PATH=\"/usr/local/cargo/bin:\${PATH:-/usr/bin:/bin}\"
cd /work/compiler/fuzz
export CARGO_PROFILE_RELEASE_LTO=false
export CARGO_BUILD_JOBS=\"\${CARGO_BUILD_JOBS:-2}\"
cargo +nightly fuzz run schema_oracle -- ${QUOTED}
"
