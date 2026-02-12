# Contributing to Ethos

We warmly welcome contributions and are committed to free and open source software.

## How to Contribute

### Adding a New Protocol Adapter

The best way to contribute is by adding support for a new Bitcoin protocol implementation. Adapters translate protocol-specific schemas into a standardized Protocol IR. The registry and Implementation pattern allow adding new protocols; the pipeline is implementation-agnostic.

**Current adapters:** Bitcoin Core

**Quick start:**

1. Create `adapters/src/<your_protocol>/` with `types.rs` and `rpc_client.rs`

2. Implement the `ProtocolBackend` trait (see existing traits in `adapters/src/rpc_adapter.rs`):
   ```rust
   #[async_trait::async_trait]
   impl ProtocolBackend for YourProtocolRpcClient {
       fn name(&self) -> &'static str { "your_protocol" }
       fn capabilities(&self) -> Vec<&'static str> { vec![CAP_RPC] }
       fn extract_protocol_ir(&self, path: &Path) -> ProtocolAdapterResult<ProtocolIR> { ... }
       fn normalize_output(&self, value: &Value) -> Value { ... }
       async fn call(&self, method: &str, params: Value) -> Result<Value, Box<dyn Error + Send + Sync>> { ... }
   }
   ```

3. Implement `BackendProvider` and register in `adapters/src/rpc_adapter.rs`:
   ```rust
   impl BackendProvider for YourProtocolRpcClient {
       fn implementation() -> Implementation { Implementation::YourProtocol }
       fn build() -> ProtocolAdapterResult<Box<dyn ProtocolBackend + Send + Sync>> { ... }
   }
   ```
   Add to `REGISTERED_BACKENDS` and add an `Implementation` variant in `primitives/types/src/implementation.rs`.

4. Add module to `adapters/src/lib.rs`.

5. Update `resources/adapters/registry.json` under the appropriate protocol section (e.g. `bitcoin`):
   ```json
   "your_protocol": {
       "name": "Your Protocol",
       "versions": ["v1.0.0"],
       "default_version": "v1.0.0",
       "adapter_class": "RpcAdapter",
       "implementation": "your_protocol"
   }
   ```

6. (Optional) Add templates in `adapters/templates/your_protocol/` and normalization rules in `resources/adapters/normalization/`

**Need help?** Check out `adapters/src/bitcoin_core/` for the reference implementation.

## Questions?

Open an issue on GitHub to discuss adapter ideas, implementation questions, or improvements to the Protocol IR.

Thank you for contributing!
