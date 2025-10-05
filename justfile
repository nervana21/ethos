# Ethos workspace justfile

set positional-arguments

NIGHTLY_VERSION := trim(read(justfile_directory() / "nightly-version"))

_default:
    @just --list

# Primary workflows
# Complete workflow: code generation → E2E tests
ethos:
    @just generate
    @just e2e

# Generate client artifacts from IR files
generate:
    cargo run --package ethos-cli --bin ethos-compiler -- pipeline --input resources/ir/bitcoin.ir.json --implementation bitcoin_core
    cargo run --package ethos-cli --bin ethos-compiler -- pipeline --input resources/ir/lightning.ir.json --implementation core_lightning

# Run e2e tests (requires 'just generate' to be run first)
e2e:
    cd tests/e2e && cargo run

# Testing
# Run tests.
test:
  cargo test

# Code quality
# Format workspace.
fmt:
  cargo +{{NIGHTLY_VERSION}} fmt --all

# Run all linting checks (clippy, whitespace, links).
lint:
  cargo +{{NIGHTLY_VERSION}} clippy --quiet --all-targets --all-features -- --deny warnings
  ./contrib/check-whitespace.sh
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


# Examples
examples:
    @echo "Examples:"
    @echo "  just generate           # Generate from IR files"
    @echo "  just e2e                # Run e2e tests"
    @echo "  just ethos               # Complete code generation workflow"
