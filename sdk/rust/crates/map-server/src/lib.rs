//! MAP node-side routing layer.
//!
//! A MAP node (M-ART `martd`, mixumd, a cloud pod) receives envelopes and
//! dispatches on `op`. This crate keeps that dispatch transport-agnostic:
//! implement [`OpHandler`], register it on a [`Router`], and hand the router
//! envelopes from any WSS/Unix/stdio transport. Replies and `.err` envelopes
//! are shaped here so every node agrees on the wire form.

use async_trait::async_trait;
use map_core::{token::new_ulid, Envelope, ErrorCode, MapError};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

/// One operation endpoint. Handlers receive the request payload and return the
/// success payload; the router owns envelope ids and error shaping.
#[async_trait]
pub trait OpHandler: Send + Sync {
    async fn handle(&self, payload: Value) -> Result<Value, MapError>;
}

/// Adapter for async closures.
pub struct FnHandler<F> {
    inner: F,
}

impl<F> FnHandler<F> {
    pub fn new(inner: F) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl<F, Fut> OpHandler for FnHandler<F>
where
    F: Fn(Value) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<Value, MapError>> + Send,
{
    async fn handle(&self, payload: Value) -> Result<Value, MapError> {
        (self.inner)(payload).await
    }
}

#[derive(Default)]
pub struct Router {
    routes: HashMap<String, Arc<dyn OpHandler>>,
}

impl Router {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an exact `op` handler. Re-registering replaces the old one.
    pub fn route<H: OpHandler + 'static>(&mut self, op: &str, handler: H) -> &mut Self {
        self.routes.insert(op.to_owned(), Arc::new(handler));
        self
    }

    /// Register an async closure as a handler.
    pub fn route_fn<F, Fut>(&mut self, op: &str, handler: F) -> &mut Self
    where
        F: Fn(Value) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<Value, MapError>> + Send + 'static,
    {
        self.routes
            .insert(op.to_owned(), Arc::new(FnHandler::new(handler)));
        self
    }

    pub fn ops(&self) -> Vec<String> {
        let mut ops: Vec<String> = self.routes.keys().cloned().collect();
        ops.sort();
        ops
    }

    /// Dispatch one request envelope and produce the reply envelope.
    pub async fn dispatch(&self, request: Envelope) -> Envelope {
        let reply_to = request.id.clone();
        let op = request.op.clone();
        let Some(handler) = self.routes.get(&op) else {
            return error_envelope(
                &reply_to,
                &op,
                ErrorCode::Unsupported,
                &format!("unknown op: {op}"),
            );
        };
        match handler.handle(request.payload).await {
            Ok(payload) => ok_envelope(&reply_to, &op, payload),
            Err(MapError::Wire { code, message }) => error_envelope(&reply_to, &op, code, &message),
            Err(other) => error_envelope(&reply_to, &op, ErrorCode::Internal, &other.to_string()),
        }
    }
}

pub fn ok_envelope(reply_to: &str, op: &str, payload: Value) -> Envelope {
    Envelope {
        v: map_core::PROTOCOL_VERSION,
        id: new_ulid(),
        op: format!("{op}.ok"),
        seq: None,
        reply_to: Some(reply_to.to_owned()),
        ts: map_core::types::now_ms(),
        payload,
    }
}

pub fn error_envelope(reply_to: &str, op: &str, code: ErrorCode, message: &str) -> Envelope {
    Envelope {
        v: map_core::PROTOCOL_VERSION,
        id: new_ulid(),
        op: format!("{op}.err"),
        seq: None,
        reply_to: Some(reply_to.to_owned()),
        ts: map_core::types::now_ms(),
        payload: json!({ "code": code, "message": message }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn routes_and_shapes_replies() {
        let mut router = Router::new();
        router.route_fn("sys.ping", |_payload| async { Ok(json!({ "pong": true })) });
        let request = Envelope::request("sys.ping", json!({}));
        let id = request.id.clone();
        let reply = router.dispatch(request).await;
        assert_eq!(reply.op, "sys.ping.ok");
        assert_eq!(reply.reply_to.as_deref(), Some(id.as_str()));
        assert_eq!(reply.payload, json!({ "pong": true }));
    }

    #[tokio::test]
    async fn unknown_op_is_unsupported() {
        let router = Router::new();
        let reply = router
            .dispatch(Envelope::request("no.such", json!({})))
            .await;
        assert_eq!(reply.op, "no.such.err");
        assert_eq!(reply.payload["code"], json!("unsupported"));
    }

    #[tokio::test]
    async fn handler_error_keeps_code() {
        let mut router = Router::new();
        router.route_fn("task.get", |_payload| async {
            Err(MapError::Wire {
                code: ErrorCode::NotFound,
                message: "no task".into(),
            })
        });
        let reply = router
            .dispatch(Envelope::request("task.get", json!({})))
            .await;
        assert_eq!(reply.op, "task.get.err");
        assert_eq!(reply.payload["code"], json!("not_found"));
        assert_eq!(reply.payload["message"], json!("no task"));
    }
}
