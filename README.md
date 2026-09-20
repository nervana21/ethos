[![License: CC0-1.0](https://img.shields.io/badge/license-CC0--1.0-blue)](LICENSE)
[![crates.io](https://img.shields.io/crates/v/ethos-bitcoind)](https://crates.io/crates/ethos-bitcoind)
[![Docs.rs](https://img.shields.io/docsrs/ethos-bitcoind)](https://docs.rs/ethos-bitcoind)


## What's Ethos?

Bitcoin Core ships a machine-readable [OpenRPC](resources/ir/openrpc.json) dump of its JSON-RPC surface and Ethos converts that dump into a strongly-typed Rust client, [`ethos-bitcoind`](https://crates.io/crates/ethos-bitcoind).

## Example

[Here's](https://github.com/nervana21/Floresta/tree/ethos) a concrete example, using Floresta, for instance.

## Contributing

Contributors are warmly welcome, see [CONTRIBUTING.md](CONTRIBUTING.md).

The highest leverage help is usually reviewing open Bitcoin Core PRs [here](https://github.com/bitcoin/bitcoin/pulls?q=is%3Apr+state%3Aopen+label%3ARPC%2FREST%2FZMQ). Consider opening OpenRPC fidelity patches against Bitcoin Core if the dump can be improved.

## License

CC0-1.0

## Security

This is experimental software in active development. Please use appropriate caution.
