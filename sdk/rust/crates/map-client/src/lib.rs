//! MAP client over WSS.
//!
//! Scope: one connection, `session.hello`, and sequential request/reply
//! correlation with a timeout. Interleaved subscriptions (`task.event`,
//! `display.frame`) are read and skipped — the full multiplexed router with
//! `session.resume` lands in the next milestone; the wire types are already
//! the same.

use futures_util::{SinkExt, StreamExt};
use map_core::{Envelope, ErrorCode, HelloOk, MapError};
use rustls::crypto::{
    verify_tls12_signature as verify_tls12, verify_tls13_signature as verify_tls13,
    WebPkiSupportedAlgorithms,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{
    connect_async_tls_with_config, Connector, MaybeTlsStream, WebSocketStream,
};

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

pub struct MapClient {
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
    /// Gateway routing target applied to every request; `None` for direct nodes.
    node: Option<String>,
}

impl MapClient {
    /// Dial the node and perform `session.hello` with a `map1.` token.
    pub async fn connect(
        url: &str,
        token: &str,
        client_name: &str,
    ) -> Result<(Self, HelloOk), MapError> {
        Self::connect_with_fingerprint(url, token, client_name, None).await
    }

    /// 自签节点:传 `fingerprint`(证书 SHA-256,大小写/冒号不敏感)固定身份。
    /// `None` 时走系统 WebPKI 根(仅适用于受信证书)。
    pub async fn connect_with_fingerprint(
        url: &str,
        token: &str,
        client_name: &str,
        fingerprint: Option<&str>,
    ) -> Result<(Self, HelloOk), MapError> {
        install_crypto_provider();
        let request = url
            .into_client_request()
            .map_err(|e| MapError::Transport(e.to_string()))?;
        let connector = fingerprint.map(|expected| {
            let provider = rustls::crypto::ring::default_provider();
            let algorithms = provider.signature_verification_algorithms;
            let config = rustls::ClientConfig::builder_with_provider(Arc::new(provider))
                .with_protocol_versions(&[&rustls::version::TLS13])
                .expect("TLS13 由 ring provider 支持")
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(PinnedVerifier {
                    expected: normalize_fingerprint(expected),
                    algorithms,
                }))
                .with_no_client_auth();
            Connector::Rustls(Arc::new(config))
        });
        let (mut ws, _response) = connect_async_tls_with_config(request, None, false, connector)
            .await
            .map_err(|e| MapError::Transport(e.to_string()))?;
        let hello = Envelope::request(
            "session.hello",
            serde_json::json!({
                "token": token,
                "client": client_name,
                "profiles": ["map-core", "snapshot"],
            }),
        );
        send_envelope(&mut ws, &hello).await?;
        let reply = wait_for(&mut ws, &hello.id, DEFAULT_TIMEOUT).await?;
        if reply.is_err() {
            let code = reply
                .payload
                .get("code")
                .and_then(|value| serde_json::from_value(value.clone()).ok())
                .unwrap_or(ErrorCode::Unauthorized);
            let message = reply
                .payload
                .get("message")
                .and_then(|value| value.as_str())
                .unwrap_or("handshake rejected")
                .to_owned();
            return Err(MapError::Wire { code, message });
        }
        let ok: HelloOk = serde_json::from_value(reply.payload)
            .map_err(|e| MapError::Transport(format!("bad hello payload: {e}")))?;
        Ok((Self { ws, node: None }, ok))
    }

    /// Route subsequent requests through a gateway to `node`.
    pub fn with_node(mut self, node: impl Into<String>) -> Self {
        self.node = Some(node.into());
        self
    }

    /// Change (or clear) the gateway routing target.
    pub fn set_node(&mut self, node: Option<String>) {
        self.node = node;
    }

    pub fn node(&self) -> Option<&str> {
        self.node.as_deref()
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
        let request = routed_envelope(self.node.as_deref(), op, payload);
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

/// Build a request envelope, applying the client's gateway routing target.
fn routed_envelope(node: Option<&str>, op: &str, payload: serde_json::Value) -> Envelope {
    let mut envelope = Envelope::request(op, payload);
    envelope.node = node.map(str::to_owned);
    envelope
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

fn install_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

fn normalize_fingerprint(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect::<String>()
        .to_ascii_uppercase()
}

#[derive(Debug)]
struct PinnedVerifier {
    expected: String,
    algorithms: WebPkiSupportedAlgorithms,
}

impl rustls::client::danger::ServerCertVerifier for PinnedVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        use sha2::{Digest, Sha256};
        let fingerprint = Sha256::digest(end_entity.as_ref())
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<String>();
        if fingerprint == self.expected {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General(format!(
                "node certificate fingerprint mismatch: got {fingerprint}"
            )))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        verify_tls12(message, cert, dss, &self.algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        verify_tls13(message, cert, dss, &self.algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ED25519,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_normalization_is_colon_and_case_insensitive() {
        assert_eq!(
            normalize_fingerprint("aa:BB:01"),
            normalize_fingerprint("AA:bb:01")
        );
    }

    #[test]
    fn routed_envelope_applies_node_target() {
        let direct = routed_envelope(None, "sys.ping", serde_json::json!({}));
        assert!(direct.node.is_none());

        let routed = routed_envelope(Some("mac-mini"), "sys.ping", serde_json::json!({}));
        assert_eq!(routed.node.as_deref(), Some("mac-mini"));
    }

    #[tokio::test]
    #[ignore = "需要运行中的 MAP 节点：MAP_URL/MAP_TOKEN/MAP_NODE_FINGERPRINT"]
    async fn live_connect_with_fingerprint() {
        let url = std::env::var("MAP_URL").expect("MAP_URL");
        let token = std::env::var("MAP_TOKEN").expect("MAP_TOKEN");
        let fingerprint = std::env::var("MAP_NODE_FINGERPRINT").ok();
        let (mut client, hello) = MapClient::connect_with_fingerprint(
            &url,
            &token,
            "map-client-test",
            fingerprint.as_deref(),
        )
        .await
        .expect("connect");
        assert!(!hello.session_id.is_empty());
        let info = client
            .call("sys.info", serde_json::json!({}))
            .await
            .unwrap();
        assert!(info.get("server").is_some());
        client.close().await;
    }
}
