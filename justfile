# Ethos workspace justfile
#
# Public surface (three recipes):
#   just sync     Core build, dump, IR, dump audits
#   just client   pinned dump, codegen, schema-validate, core-test
#   just check    lint, dump audits, workspace tests

set positional-arguments

NIGHTLY_VERSION := trim(read(justfile_directory() / "nightly-version"))
RELEASE := if env_var_or_default('FAST', '1') == '1' { "--profile dev-fast" } else { "--release" }
LATEST_VERSION := "v30.2.11"

_default:
    @echo "just sync | client | check"
    @echo ""
    @echo "  sync *flags     corpus bitcoind, openrpc.json, IR, dump audits"
    @echo "  client *flags   pinned dump, ethos-bitcoind, core-test"
    @echo "  check           lint, dump audits, workspace tests"
    @echo ""
    @echo "Flags for sync/client: --skip-build --skip-dump --skip-client --dance --stage --no-hidden"
    @echo "Also: fmt lint docsrs corpus-pull"

# --- public ---

# Build corpus bitcoind, dump getopenrpcinfo, write IR, run dump audits. No codegen.
sync *flags:
    bash {{justfile_directory()}}/scripts/loop_openrpc.sh --skip-codegen --skip-client {{flags}}

# From pinned dump: IR + audits + codegen + schema-validate + ethos-test-client core-test.
# Typical after sync: just client --skip-build --skip-dump
client *flags:
    bash {{justfile_directory()}}/scripts/loop_openrpc.sh --skip-build --skip-dump {{flags}}

# Ethos repo hygiene on the pinned dump. Not a Core PR surface check.
check: lint
    just _audit-fidelity
    just _audit-keywords
    cargo test --workspace --quiet --all-targets --no-default-features
    cargo test --workspace --quiet --all-targets --all-features

# --- private helpers (loop_openrpc.sh) ---

_ir input output="":
    @if [ -z "{{output}}" ]; then \
        cargo run {{RELEASE}} -p ethos-adapters --bin process_bitcoin_openrpc -- {{input}}; \
    else \
        cargo run {{RELEASE}} -p ethos-adapters --bin process_bitcoin_openrpc -- {{input}} {{output}}; \
    fi

_audit-fidelity input="resources/ir/openrpc.json" report="resources/reports/openrpc_type_fidelity_report.json":
    cargo run {{RELEASE}} -p ethos-adapters --bin openrpc_type_fidelity_audit -- {{input}} --json-report {{report}}

_audit-keywords input="resources/ir/openrpc.json" report="":
    @if [ -z "{{report}}" ]; then \
        cargo run {{RELEASE}} -p ethos-adapters --bin openrpc_schema_keyword_audit -- {{input}}; \
    else \
        cargo run {{RELEASE}} -p ethos-adapters --bin openrpc_schema_keyword_audit -- {{input}} --json-report {{report}}; \
    fi

_codegen input_file="" output_path="" version="" *pipeline_flags:
    @set --; \
    [ -n "{{output_path}}" ] && set -- "$@" --output "{{output_path}}"; \
    [ -n "{{version}}" ] && set -- "$@" --version "{{version}}"; \
    [ -n "{{input_file}}" ] && set -- "$@" --input "{{input_file}}"; \
    set -- "$@" {{pipeline_flags}}; \
    cargo run {{RELEASE}} --package ethos-cli --bin ethos-compiler -- pipeline --implementation bitcoin_core "$@"

_stage-downstream output_path:
    @bash -c 'set -euo pipefail; \
      ethos_root="{{justfile_directory()}}"; out="{{output_path}}"; \
      if ! git -C "$out" rev-parse --git-dir >/dev/null 2>&1; then \
        echo "error: not a git repository: $out" >&2; exit 1; \
      fi; \
      if [ -z "$(git -C "$out" status --porcelain)" ]; then \
        echo "No changes in $out; nothing to stage."; exit 0; \
      fi; \
      git -C "$out" add -A; \
      msg_file="$(git -C "$out" rev-parse --git-dir)/SUGGESTED_COMMIT_MSG"; \
      if ! subj=$(git -C "$ethos_root" log -1 --format=%s 2>/dev/null); then \
        subj="codegen: sync from ethos"; \
      fi; \
      printf "%s\n" "$subj" > "$msg_file"; \
      clip_ok=0; \
      if command -v pbcopy >/dev/null 2>&1; then printf "%s" "$subj" | pbcopy && clip_ok=1; fi; \
      echo ""; \
      ethos_h=$(git -C "$ethos_root" rev-parse --short HEAD 2>/dev/null || echo "?"); \
      echo "Staged all changes in $out (ethos $ethos_h)."; \
      echo "Suggested subject: $subj"; \
      if [ "$clip_ok" = 1 ]; then echo "Copied suggested subject to clipboard (pbcopy)."; \
      else echo "Clipboard skipped (pbcopy not found; macOS only)."; fi; \
      echo "Commit after review: git -C \"$out\" commit -e -F \"$msg_file\""; \
    '

# --- misc ---

fmt:
    cargo +{{NIGHTLY_VERSION}} fmt --all

lint:
    cargo +{{NIGHTLY_VERSION}} clippy --quiet --all-targets --all-features -- --deny warnings
    @bash -c 'if command -v lychee >/dev/null 2>&1; then lychee .; else echo "Warning: lychee not found. Skipping link check."; echo "Install with: cargo install lychee"; fi'

prek:
    prek run

@docsrs *flags:
    RUSTDOCFLAGS="--cfg docsrs -D warnings -D rustdoc::broken-intra-doc-links" cargo +{{NIGHTLY_VERSION}} doc --all-features --no-deps {{flags}}

corpus-pull:
    @bash -c 'cd corpus && \
    for repo in $(grep -E "^\s*[a-z_-]+ = \{" ../manifest.toml | cut -d" " -f1 | tr -d " "); do \
        if [ -d "$repo" ]; then \
            echo "Pulling $repo..."; \
            cd "$repo"; \
            if ! git diff --quiet HEAD 2>/dev/null || ! git diff --cached --quiet 2>/dev/null; then \
                echo "  Stashing local changes..."; \
                git stash push -m "Auto-stash before pull" 2>/dev/null || true; \
                git pull --ff-only 2>/dev/null || echo "  Could not fast-forward"; \
                git stash pop 2>/dev/null || true; \
            else \
                git pull --ff-only 2>/dev/null || echo "  Could not fast-forward"; \
            fi; \
            cd ..; \
        else \
            echo "Directory $repo not found, skipping..."; \
        fi; \
    done'
    @echo "Done pulling corpus repositories."

@udeps:
    cargo +{{NIGHTLY_VERSION}} udeps --workspace --all-targets

@audit:
    cargo audit
