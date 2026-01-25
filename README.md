[![License: CC0-1.0](https://img.shields.io/badge/license-CC0--1.0-blue)](LICENSE)

# Ethos: A Meta-Compiler for The Bitcoin Protocol

Ethos is a meta-compiler for The Bitcoin Protocol.

## Architecture

See [docs/architecture.mmd](docs/architecture.mmd) for a full system diagram.

The compiler pipeline:
1. **Schema Input**: Projects produce `schema.json` files in the expected format
2. **IR Generation**: Convert `schema.json` to `XXX_XXX.ir.json`
3. **Analysis** normalizes IR and validates consistency
4. **Codegen** generates Rust client libraries, traits, and types

Deep Dive: [docs/semantic-convergence.md](docs/semantic-convergence.md)

## Getting Started

### Prerequisites

1. **Rust** (edition 2021, rust-version 1.70+)
2. **just** command runner (install with `cargo install just`)
3. Protocol executables (for integration tests):
   - `bitcoind` (Bitcoin Core)
   - `lightningd` (Core Lightning)

### Quick Start

Run the complete code generation workflow:
```bash
just ethos
```

This will:
1. Generate client libraries from IR files (`resources/ir/`)
2. Run end-to-end tests against spawned protocol nodes

## Contributing

Contributors are warmly welcome, see [CONTRIBUTING.md](CONTRIBUTING.md).

## License

CC0-1.0

## Security

This is experimental software in active development. Please use appropriate caution.
