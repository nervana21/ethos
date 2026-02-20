[![License: CC0-1.0](https://img.shields.io/badge/license-CC0--1.0-blue)](LICENSE)

# Ethos

A formal RPC description for type-safe Rust clients

## Why Ethos?

Recent [discussion](https://delvingbitcoin.org/t/the-future-of-the-bitcoin-core-gui/2253/17) has suggested a renewed interest in a formal description of the [RPC API](https://github.com/bitcoin/bitcoin/issues/29912) surface. The Bitcoin Core RPC surface is the predominant means through which external clients query the blockchain. As tooling continues to depend on RPC behavior, the need for and benefits from a behavioral specification are likely to increase.

Ethos is a PoC for the capabilities of such a [specification](resources/ir/schema.json). [Other](https://github.com/willcl-ark/bitcoin-rpc-web/blob/master/assets/openrpc.json) comprehensive specifications may be adapted to.

## Architecture

[Schema](resources/ir/schema.json) → [IR](resources/ir/bitcoin.ir.json) → [codegen](https://crates.io/crates/ethos-bitcoind)

## Getting Started

### Prerequisites

1. **Rust** (edition 2021, rust-version 1.70+)
2. **just** command runner (install with `cargo install just`)
3. Protocol executable (for integration tests): `bitcoind` (Bitcoin Core)

### Quick Start

Run the complete code generation workflow:
```bash
just ethos
```

This will:
1. Generate a Bitcoin Core client library from the IR file (`resources/ir/bitcoin.ir.json`)
2. Run end-to-end tests against a spawned node

## Contributing

Contributors are warmly welcome, see [CONTRIBUTING.md](CONTRIBUTING.md).

## License

CC0-1.0

## Security

This is experimental software in active development. Please use appropriate caution.
