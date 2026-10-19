//! map-core — Rust SDK for the Mixum Agent Protocol (MAP).
//!
//! The specification (`site/src/content/docs/spec`) is authoritative; this
//! crate is a convenience over the wire. Wire JSON is camelCase; types use
//! serde with `rename_all = "camelCase"` and tolerate unknown fields
//! (additive evolution is a protocol rule).
//!
//! Transport (WSS client) and snapshot tooling live in the sibling crates
//! `map-client`, `map-server` and `map-snapshot`.

pub mod error;
pub mod token;
pub mod types;

pub const PROTOCOL_VERSION: u32 = 1;
pub const SUBPROTOCOL: &str = "map.v1";
/// Transitional subprotocol accepted alongside `map.v1` (spec §hello).
pub const LEGACY_SUBPROTOCOL: &str = "mixum.v1";

pub use error::{ErrorCode, MapError};
pub use types::*;
