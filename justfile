# Ethos workspace justfile

set positional-arguments

NIGHTLY_VERSION := trim(read(justfile_directory() / "nightly-version"))

_default:
    @just --list

# Use release builds for pipeline/adapters (much faster run; first build takes longer).
# Set to "" for debug builds (e.g. when debugging the compiler).
RELEASE := "--release"
LATEST_VERSION := "v30.2.9"

# Process OpenRPC document (or version) into canonical IR. Input = path to OpenRPC JSON or version (e.g. {{LATEST_VERSION}}) to extract from canonical IR.
# Example: just process-openrpc resources/ir/openrpc.json  |  just process-openrpc {{LATEST_VERSION}} out.ir.json
process-openrpc input output="":
    @if [ -z "{{output}}" ]; then \
        cargo run {{RELEASE}} -p ethos-adapters --bin process_bitcoin_openrpc -- {{input}}; \
    else \
        cargo run {{RELEASE}} -p ethos-adapters --bin process_bitcoin_openrpc -- {{input}} {{output}}; \
    fi

# Generate client from IR. Set output_path to write into a repo (e.g. ../ethos-bitcoind); use version for a pinned release.
# Example: just generate-from-ir  |  just generate-from-ir ../ethos-bitcoind {{LATEST_VERSION}}
generate-from-ir input_file="" output_path="" version="" impl="bitcoin_core":
    @set --; \
    [ -n "{{output_path}}" ] && set -- "$@" --output "{{output_path}}"; \
    [ -n "{{version}}" ] && set -- "$@" --version "{{version}}"; \
    [ -n "{{input_file}}" ] && set -- "$@" --input "{{input_file}}"; \
    cargo run {{RELEASE}} --package ethos-cli --bin ethos-compiler -- pipeline --implementation {{impl}} "$@"

# Process OpenRPC → IR → generate client into repo. Default openrpc_file includes hidden RPCs.
process-openrpc-and-generate output_path version="" openrpc_file="resources/ir/openrpc.json" impl="bitcoin_core":
    just process-openrpc {{openrpc_file}} resources/ir/bitcoin.ir.json && just generate-from-ir resources/ir/bitcoin.ir.json {{output_path}} {{version}} {{impl}}


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
  cargo +{{NIGHTLY_VERSION}} udeps --all-targets

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
    @echo "  just generate-from-ir            # Generate client from IR"
    @echo "  just generate-from-ir ../ethos-bitcoind {{LATEST_VERSION}}   # Generate into repo with version"
    @echo "  just process-openrpc resources/ir/openrpc.json resources/ir/bitcoin.ir.json"
    @echo "  just process-openrpc-and-generate ../ethos-bitcoind   # OpenRPC → IR → generate into repo"
    @echo "  just corpus-pull         # Pull all corpus repositories"
