# Ethos workspace justfile

set positional-arguments

NIGHTLY_VERSION := trim(read(justfile_directory() / "nightly-version"))

_default:
    @just --list

# Generate + e2e tests
ethos input_file="":
    @just generate {{input_file}}
    @just e2e

# Generate client artifacts from IR files
generate input_file="":
    @if [ -n "{{input_file}}" ]; then \
        cargo run --package ethos-cli --bin ethos-compiler -- pipeline --input {{input_file}} --implementation bitcoin_core; \
    else \
        cargo run --package ethos-cli --bin ethos-compiler -- pipeline --implementation bitcoin_core; \
    fi

# Usage: just generate-into-repo /path/to/ethos-bitcoind [version] [impl]
# First time: mkdir ../ethos-bitcoind && cd ../ethos-bitcoind && git init
#   or with version override: just generate-into-repo ../ethos-bitcoind v30.2.4
generate-into-repo output_path version="" impl="bitcoin_core":
    @if [ -n "{{version}}" ]; then \
        cargo run --package ethos-cli --bin ethos-compiler -- pipeline --implementation {{impl}} --version {{version}} --output {{output_path}}; \
    else \
        cargo run --package ethos-cli --bin ethos-compiler -- pipeline --implementation {{impl}} --output {{output_path}}; \
    fi

# Alias for process-schema
create-version-ir schema_file output_file="":
    @just process-schema {{schema_file}} {{output_file}}

# Process Bitcoin Core schema.json to IR
process-schema schema_file output_file="":
    @if [ -z "{{output_file}}" ]; then \
        cargo run -p ethos-adapters --bin process_bitcoin_schema -- {{schema_file}}; \
    else \
        cargo run -p ethos-adapters --bin process_bitcoin_schema -- {{schema_file}} {{output_file}}; \
    fi

# Extract version-specific IR from canonical bitcoin.ir.json
extract-version-ir version output_file="":
    @if [ -z "{{output_file}}" ]; then \
        cargo run -p ethos-adapters --bin process_bitcoin_schema -- {{version}}; \
    else \
        cargo run -p ethos-adapters --bin process_bitcoin_schema -- {{version}} {{output_file}}; \
    fi

e2e:
    cd tests/e2e && cargo run

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
# Quick sanity check.
[group('ci')]
@sane: lint
  cargo test --quiet --all-targets --no-default-features
  cargo test --quiet --all-targets --all-features

# Examples
examples:
    @echo "Examples:"
    @echo "  just generate           # Generate from IR files"
    @echo "  just generate-into-repo ../ethos-bitcoind   # Generate into a separate git repo for diff review"
    @echo "  just e2e                # Run e2e tests"
    @echo "  just ethos               # Complete code generation workflow"
    @echo "  just corpus-pull         # Pull all corpus repositories"
