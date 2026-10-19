//! MAP client over WSS.
//!
//! Scope: one connection, `session.hello`, and sequential request/reply
//! correlation with a timeout. Interleaved subscriptions (`task.event`,
//! `display.frame`) are read and skipped — the full multiplexed router with
//! `session.resume` lands in the next milestone; the wire types are already
//! the same.

use futures_util::{SinkExt, StreamExt};
use map_core::{Envelope, ErrorCode, HelloOk, MapError};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

pub struct MapClient {
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl MapClient {
    /// Dial the node and perform `session.hello` with a `map1.` token.
    pub async fn connect(
        url: &str,
        token: &str,
        client_name: &str,
    ) -> Result<(Self, HelloOk), MapError> {
        let (mut ws, _response) = connect_async(url)
            .await
            .map_err(|e| MapError::Transport(e.to_string()))?;
        let hello = Envelope::request(
            "session.hello",
            serde_json::json!({
                "token": token,
                "client": client_name,
                "profiles": ["map-core", "snapshot"],
                "versions": [map_core::PROTOCOL_VERSION],
            }),
        );
        send_envelope(&mut ws, &hello).await?;
        let reply = wait_for(&mut ws, &hello.id, DEFAULT_TIMEOUT).await?;
        let ok: HelloOk = serde_json::from_value(reply.payload)
            .map_err(|e| MapError::Transport(format!("bad hello payload: {e}")))?;
        Ok((Self { ws }, ok))
    }

    /// Send one request and wait for the matching reply.
    pub async fn call(
        &mut self,
        op: &str,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, MapError> {
        self.call_with_timeout(op, payload, DEFAULT_TIMEOUT).await
    }

    pub async fn call_with_timeout(
        &mut self,
        op: &str,
        payload: serde_json::Value,
        timeout: Duration,
    ) -> Result<serde_json::Value, MapError> {
        let request = Envelope::request(op, payload);
        let id = request.id.clone();
        send_envelope(&mut self.ws, &request).await?;
        let reply = wait_for(&mut self.ws, &id, timeout).await?;
        if reply.is_err() {
            let code = reply
                .payload
                .get("code")
                .and_then(|value| serde_json::from_value(value.clone()).ok())
                .unwrap_or(ErrorCode::Internal);
            let message = reply
                .payload
                .get("message")
                .and_then(|value| value.as_str())
                .unwrap_or("request failed")
                .to_owned();
            return Err(MapError::Wire { code, message });
        }
        Ok(reply.payload)
    }

    /// Graceful close (best effort).
    pub async fn close(mut self) {
        let _ = self.ws.close(None).await;
    }
}

async fn send_envelope(
    ws: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
    envelope: &Envelope,
) -> Result<(), MapError> {
    let text = serde_json::to_string(envelope).map_err(|e| MapError::Transport(e.to_string()))?;
    ws.send(Message::text(text))
        .await
        .map_err(|e| MapError::Transport(e.to_string()))
}

async fn wait_for(
    ws: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
    id: &str,
    timeout: Duration,
) -> Result<Envelope, MapError> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(MapError::Timeout);
        }
        let message = tokio::time::timeout(remaining, ws.next())
            .await
            .map_err(|_| MapError::Timeout)?;
        let message = match message {
            Some(Ok(message)) => message,
            Some(Err(e)) => return Err(MapError::Transport(e.to_string())),
            None => return Err(MapError::Transport("connection closed".into())),
        };
        // Only text frames carry envelopes; binary frames are display streams.
        let data = match message {
            Message::Text(text) => text,
            Message::Close(_) => return Err(MapError::Transport("closed by peer".into())),
            _ => continue,
        };
        let envelope: Envelope = match serde_json::from_str(&data) {
            Ok(envelope) => envelope,
            Err(_) => continue,
        };
        if envelope.reply_to.as_deref() == Some(id) {
            return Ok(envelope);
        }
        // Not our reply: an event or a reply to another request. Skipped.
    }
}
