# Ethos workspace justfile

set positional-arguments

NIGHTLY_VERSION := trim(read(justfile_directory() / "nightly-version"))

_default:
    @just --list

# Use dev-fast by default (incremental + opt-level 1) for fast iteration; full release is slower to compile.
# Set FAST=0 for full release builds (e.g. CI or when compiler runtime matters).
RELEASE := if env_var_or_default('FAST', '1') == '1' { "--profile dev-fast" } else { "--release" }
LATEST_VERSION := "v30.2.11"

# Process OpenRPC document (or version) into canonical IR. Input = path to OpenRPC JSON or version (e.g. {{LATEST_VERSION}}) to extract from canonical IR.
# Example: just process-openrpc resources/ir/openrpc.json  |  just process-openrpc {{LATEST_VERSION}} out.ir.json
process-openrpc input output="":
    @if [ -z "{{output}}" ]; then \
        cargo run {{RELEASE}} -p ethos-adapters --bin process_bitcoin_openrpc -- {{input}}; \
    else \
        cargo run {{RELEASE}} -p ethos-adapters --bin process_bitcoin_openrpc -- {{input}} {{output}}; \
    fi

# Generate client from IR. Set output_path to write into a repo (e.g. ../ethos-bitcoind); use version for a pinned release.
# Extra arguments (e.g. --exclude-hidden-rpcs) are forwarded to the pipeline and applied before codegen.
# Examples:
#   just generate-from-ir
#   just generate-from-ir ../ethos-bitcoind {{LATEST_VERSION}}
#   just generate-from-ir ../ethos-bitcoind {{LATEST_VERSION}} --exclude-hidden-rpcs
generate-from-ir input_file="" output_path="" version="" *pipeline_flags:
    @set --; \
    [ -n "{{output_path}}" ] && set -- "$@" --output "{{output_path}}"; \
    [ -n "{{version}}" ] && set -- "$@" --version "{{version}}"; \
    [ -n "{{input_file}}" ] && set -- "$@" --input "{{input_file}}"; \
    set -- "$@" {{pipeline_flags}}; \
    cargo run {{RELEASE}} --package ethos-cli --bin ethos-compiler -- pipeline --implementation bitcoin_core "$@"

# After codegen: stage everything in the downstream repo, write ethos HEAD's subject to `.git/SUGGESTED_COMMIT_MSG`, and copy that subject to the clipboard (macOS `pbcopy` only).
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

# Process OpenRPC → IR → generate client into repo.
# Uses the default OpenRPC file; extra flags (e.g. --exclude-hidden-rpcs) are forwarded only to the pipeline (not to OpenRPC processing).
# Set STAGE_DOWNSTREAM=1 to run `_stage-downstream` afterward (same as `process-openrpc-and-generate-stage`).
process-openrpc-and-generate output_path version="" *pipeline_flags:
    just process-openrpc resources/ir/openrpc.json resources/ir/bitcoin.ir.json && just generate-from-ir resources/ir/bitcoin.ir.json {{output_path}} {{version}} {{pipeline_flags}}
    @if [ "${STAGE_DOWNSTREAM:-0}" = 1 ]; then just _stage-downstream "{{output_path}}"; fi

# Same as process-openrpc-and-generate + `_stage-downstream`. Review with `git diff --cached` in the downstream repo, then commit.
process-openrpc-and-generate-stage output_path version="" *pipeline_flags:
    just process-openrpc-and-generate {{output_path}} {{version}} {{pipeline_flags}}
    just _stage-downstream "{{output_path}}"


# Code quality
# Format workspace.
fmt:
  cargo +{{NIGHTLY_VERSION}} fmt --all

# Run all linting checks (clippy, whitespace, links).
lint:
  cargo +{{NIGHTLY_VERSION}} clippy --quiet --all-targets --all-features -- --deny warnings
  @bash -c 'if command -v lychee >/dev/null 2>&1; then lychee .; else echo "Warning: lychee not found. Skipping link check."; echo "Install with: cargo install lychee"; fi'

# Run prek hooks on staged files (same scope as a normal commit)
prek:
    prek run

# Documentation
# Generate documentation (accepts cargo doc args, e.g. --open).
@docsrs *flags:
  RUSTDOCFLAGS="--cfg docsrs -D warnings -D rustdoc::broken-intra-doc-links" cargo +{{NIGHTLY_VERSION}} doc --all-features --no-deps {{flags}}

# Advanced/utility commands
# Run all fuzz targets
fuzz-all:
    just -f compiler/fuzz/justfile fuzz-all

# Pull all corpus repositories from manifest.toml
# Preserves local changes by stashing before pull
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

# Check for unused dependencies.
@udeps:
  cargo +{{NIGHTLY_VERSION}} udeps --workspace --all-targets

# Run security audit.
@audit:
  cargo audit

# CI
# Full sanity check.
[group('ci')]
@sane: lint
  cargo test --quiet --all-targets --no-default-features
  cargo test --quiet --all-targets --all-features

# Examples
examples:
    @echo "Examples:"
    @echo "  just sane                # Full check before push (lint + tests)"
    @echo "  just generate-from-ir            # Generate client from IR (full RPC surface)"
    @echo "  just generate-from-ir ../ethos-bitcoind {{LATEST_VERSION}}   # Generate into repo with version (full RPC surface)"
    @echo "  just generate-from-ir ../ethos-bitcoind {{LATEST_VERSION}} --exclude-hidden-rpcs   # Generate without hidden/testing-only RPCs"
    @echo "  just process-openrpc resources/ir/openrpc.json resources/ir/bitcoin.ir.json"
    @echo "  just process-openrpc-and-generate ../ethos-bitcoind   # OpenRPC → IR → generate into repo"
    @echo "  just process-openrpc-and-generate-stage ../ethos-bitcoind {{LATEST_VERSION}}   # …then stage + ethos HEAD subject as suggested commit"
    @echo "  STAGE_DOWNSTREAM=1 just process-openrpc-and-generate ../ethos-bitcoind {{LATEST_VERSION}}   # same as -stage"
    @echo "  just corpus-pull         # Pull all corpus repositories"
