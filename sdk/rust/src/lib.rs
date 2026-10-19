//! map-core — Rust SDK for the Mixum Agent Protocol (MAP).
//!
//! The specification (`site/src/content/docs/spec`) is authoritative; this
//! crate is a convenience over the wire. Wire JSON is camelCase; types use
//! serde with `rename_all = "camelCase"` and tolerate unknown fields
//! (additive evolution is a protocol rule).

pub mod error;
pub mod token;
pub mod types;

#[cfg(feature = "client")]
pub mod client;

pub const PROTOCOL_VERSION: u32 = 1;
pub const SUBPROTOCOL: &str = "map.v1";

pub use error::MapError;
pub use types::*;
