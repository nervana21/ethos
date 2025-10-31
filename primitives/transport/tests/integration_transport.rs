//! Integration tests for the shared transport API.
//!
//! These are forward-looking tests meant to exercise consumer usage
//! patterns without requiring a running Bitcoin Core instance.
//! They are ignored by default in CI.

use transport::{Transport, TransportError};

struct DummyTransport;

#[async_trait::async_trait]
impl transport::Transport for DummyTransport {
    async fn send(
        &self,
        method: &str,
        _params: &[serde_json::Value],
    ) -> Result<serde_json::Value, TransportError> {
        if method == "fail" {
            Err(TransportError::Rpc("dummy error".to_string()))
        } else {
            Ok(serde_json::json!({"ok": true}))
        }
    }

    fn endpoint(&self) -> &str { "dummy://" }
}

#[tokio::test]
async fn consumer_can_call_transport() {
    let t = DummyTransport;
    let v = t.send("ping", &[]).await.expect("ok");
    assert_eq!(v["ok"], true);
}

#[tokio::test]
async fn consumer_sees_rpc_error() {
    let t = DummyTransport;
    let err = t.send("fail", &[]).await.expect_err("should err");
    match err {
        TransportError::Rpc(msg) => assert!(msg.contains("dummy")),
        _ => panic!("unexpected error variant"),
    }
}
