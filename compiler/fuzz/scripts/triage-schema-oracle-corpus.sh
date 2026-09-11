#!/usr/bin/env bash
# Summarize schema_oracle_corpus by class. Oracle hits = schema_mismatch|decode_fail only.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
CORPUS="${1:-$ROOT/resources/testdata/schema_oracle_corpus}"
python3 - "$CORPUS" <<'PY'
import json, sys
from pathlib import Path
from collections import Counter
root = Path(sys.argv[1])
if not root.is_dir():
    print(f"missing corpus dir: {root}", file=sys.stderr)
    sys.exit(2)
c = Counter()
oracle = []
rejects = []
other = []
for p in sorted(root.glob("*.json")):
    d = json.loads(p.read_text())
    cls = d.get("class", "?")
    c[cls] += 1
    row = f"{p.name}\t{d.get('method')}\t{cls}"
    if cls in ("schema_mismatch", "decode_fail"):
        oracle.append((row, d))
    elif cls == "expected_reject":
        rejects.append(row)
    else:
        other.append(row)
print("=== schema-oracle corpus triage ===")
print(f"dir: {root}")
print("counts:", dict(sorted(c.items())))
print(f"oracle_hits: {len(oracle)}  (feed Core/IR only these)")
print(f"expected_reject: {len(rejects)}  (noise unless hunting reject paths)")
if other:
    print(f"other: {len(other)}")
    for r in other[:20]:
        print(" ", r)
if oracle:
    print("--- oracle (action required) ---")
    for row, d in oracle:
        print(row)
        detail = d.get("detail")
        if isinstance(detail, str) and len(detail) > 200:
            detail = detail[:200] + "…"
        print("  detail:", detail)
else:
    print("--- no SchemaMismatch / DecodeFail yet ---")
    print("next: just schema-oracle-fuzz-box && just schema-oracle-cov")
    print("      just schema-oracle-fuzz -- --duration-secs 120")
PY
