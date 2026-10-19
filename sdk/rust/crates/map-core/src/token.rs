//! `map1.` capability tokens: parsing (and verification behind
//! `token-verify`). Format: `map1.<b64url(payload)>.<b64url(ed25519 sig)>`,
//! signed by the Node identity key (transport-security.md).

use crate::error::MapError;
use crate::types::TokenClaims;

/// Base64url (no padding) decode — std-only implementation.
pub fn b64url_decode(input: &str) -> Result<Vec<u8>, MapError> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    for ch in input.bytes() {
        let val = TABLE
            .iter()
            .position(|&t| t == ch)
            .ok_or_else(|| MapError::Wire {
                code: crate::error::ErrorCode::BadRequest,
                message: "token: invalid base64url character".into(),
            })? as u32;
        acc = (acc << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Ok(out)
}

/// Base64url (no padding) encode — used by token issuers and tooling.
pub fn b64url_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b = [
            chunk.first().copied().unwrap_or(0),
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(n >> 6) as usize & 63] as char);
        }
        if chunk.len() > 2 {
            out.push(TABLE[n as usize & 63] as char);
        }
    }
    out
}

/// A parsed, not-yet-verified token.
pub struct MapToken {
    pub claims: TokenClaims,
    pub payload: Vec<u8>,
    pub signature: Vec<u8>,
}

impl MapToken {
    /// Parse `map1.<payload>.<sig>`; verifies structural expiry only.
    pub fn parse(token: &str) -> Result<Self, MapError> {
        let rest = token.strip_prefix("map1.").ok_or_else(|| MapError::Wire {
            code: crate::error::ErrorCode::Unauthorized,
            message: "token: missing map1. prefix".into(),
        })?;
        let (payload_s, sig_s) = rest.split_once('.').ok_or_else(|| MapError::Wire {
            code: crate::error::ErrorCode::Unauthorized,
            message: "token: missing signature segment".into(),
        })?;
        let payload = b64url_decode(payload_s)?;
        let signature = b64url_decode(sig_s)?;
        let claims: TokenClaims = serde_json::from_slice(&payload).map_err(|e| MapError::Wire {
            code: crate::error::ErrorCode::Unauthorized,
            message: format!("token: payload is not valid claims: {e}"),
        })?;
        let token = Self {
            claims,
            payload,
            signature,
        };
        token.check_expiry()?;
        Ok(token)
    }

    pub fn check_expiry(&self) -> Result<(), MapError> {
        let now = crate::types::now_ms() / 1000;
        if now >= self.claims.exp {
            return Err(MapError::Wire {
                code: crate::error::ErrorCode::Unauthorized,
                message: "token expired".into(),
            });
        }
        Ok(())
    }

    /// Verify the ed25519 signature against the Node identity public key.
    #[cfg(feature = "token-verify")]
    pub fn verify(&self, public_key: &ed25519_dalek::VerifyingKey) -> Result<(), MapError> {
        use ed25519_dalek::{Signature, Verifier};
        let sig = Signature::from_slice(&self.signature).map_err(|_| MapError::Wire {
            code: crate::error::ErrorCode::Unauthorized,
            message: "token: malformed signature".into(),
        })?;
        public_key
            .verify(&self.payload, &sig)
            .map_err(|_| MapError::Wire {
                code: crate::error::ErrorCode::Unauthorized,
                message: "token: signature mismatch".into(),
            })
    }
}

/// Minimal canonical ULID generator (time + crypto randomness when the
/// `token-verify` feature pulls in no RNG by itself; falls back to a
/// process-global counter seeded by time, adequate for envelope ids).
pub fn new_ulid() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    const ENC: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let time = crate::types::now_ms() as u64;
    let n = time
        .wrapping_mul(1_000_003)
        .wrapping_add(COUNTER.fetch_add(1, Ordering::Relaxed));
    let mut out = String::with_capacity(26);
    let mut t = time;
    for _ in 0..10 {
        out.push(ENC[(t % 32) as usize] as char);
        t /= 32;
    }
    let mut r = n;
    for _ in 0..16 {
        out.push(ENC[(r % 32) as usize] as char);
        r /= 32;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn b64url_roundtrip() {
        let data = b"hello MAP payload \xF0\x9F\x8C\x8D";
        let enc = b64url_encode(data);
        assert_eq!(b64url_decode(&enc).unwrap(), data);
        assert!(!enc.contains('='));
    }

    #[test]
    fn ulid_shape() {
        let id = new_ulid();
        assert_eq!(id.len(), 26);
        assert!(id
            .chars()
            .all(|c| "0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(c)));
    }
}
