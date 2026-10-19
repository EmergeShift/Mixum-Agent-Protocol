//! WSS MAP client (feature `client`). Skeleton: handshake + request/reply
//! correlation over tokio-tungstenite. Full chunk streaming lands post-v1.

use crate::error::MapError;
use crate::types::{Envelope, HelloOk};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

pub struct MapClient<S> {
    sink: S,
    next_id: std::collections::VecDeque<String>,
}

impl<S> MapClient<S>
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
        + SinkExt<Message>
        + Unpin,
{
    /// Perform session.hello over an established (TLS) WebSocket.
    pub async fn hello(mut sink: S, token: &str) -> Result<(Self, HelloOk), MapError> {
        let env = Envelope::request(
            "session.hello",
            serde_json::json!({ "token": token, "client": "map-core/0.1.0" }),
        );
        let text = serde_json::to_string(&env).map_err(|e| MapError::Transport(e.to_string()))?;
        sink.send(Message::text(text)).await.map_err(|e| MapError::Transport(e.to_string()))?;

        let hello = loop {
            let msg = sink.next().await.ok_or(MapError::Transport("closed".into()))?;
            let msg = msg.map_err(|e| MapError::Transport(e.to_string()))?;
            let data = msg.into_data();
            let env: Envelope = serde_json::from_slice(&data)
                .map_err(|e| MapError::Transport(format!("non-envelope frame: {e}")))?;
            if env.op == "session.hello.ok" {
                let ok: HelloOk = serde_json::from_value(env.payload)
                    .map_err(|e| MapError::Transport(format!("bad hello payload: {e}")))?;
                break ok;
            }
            if env.is_err() {
                return Err(MapError::Wire {
                    code: crate::error::ErrorCode::Unauthorized,
                    message: "handshake rejected".into(),
                });
            }
        };

        Ok((Self { sink, next_id: Default::default() }, hello))
    }
}
