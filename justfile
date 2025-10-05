# Ethos workspace justfile

# Run all fuzz targets
fuzz-all:
    just -f compiler/fuzz/justfile fuzz-all

# Generate client artifacts from IR files
generate:
    cargo run --package ethos-cli --bin ethos-compiler -- pipeline --input resources/ir/bitcoin.ir.json --implementation bitcoin_core
    cargo run --package ethos-cli --bin ethos-compiler -- pipeline --input resources/ir/lightning.ir.json --implementation core_lightning

# Run e2e tests (requires 'just generate' to be run first)
e2e:
    cd tests/e2e && cargo run

# Complete workflow: code generation → E2E tests
ethos:
    @just generate
    @just e2e

check:
    cargo +$(cat nightly-version) fmt --all
    cargo clippy --all-targets -- -D warnings

# Examples
examples:
    @echo "Examples:"
    @echo "  just generate           # Generate from IR files"
    @echo "  just e2e                # Run e2e tests"
    @echo "  just ethos               # Complete code generation workflow"
