# map-core

Rust SDK for the [Mixum Agent Protocol (MAP)](https://github.com/ashreo/Mixum-Agent-Protocol) — skeleton release.

The [specification](../../site/src/content/docs/spec) is authoritative; this crate is a convenience over the wire.

## Usage

```toml
[dependencies]
map-core = "0.1"
```

```rust
use map_core::{Envelope, Task, SnapNode, TokenClaims};

let env = Envelope::request("task.submit", serde_json::json!({
    "instruction": "Hello, sandbox"
}));
let token = map_core::token::MapToken::parse(&raw_token)?;
```

## Features

| Feature | Pulls | Gives |
|---|---|---|
| *(default)* | serde only | wire types (`types.rs`), error codes, `map1.` token parsing, ULID |
| `token-verify` | ed25519-dalek | signature verification against the Node identity key |
| `client` | tokio + tokio-tungstenite | WSS client skeleton (`client.rs`) |

## What's in the skeleton

- `types.rs` — serde wire types mirroring `schemas/*.schema.json` (camelCase on the wire, unknown fields tolerated).
- `error.rs` — stable `ErrorCode` enum + client-side `MapError`.
- `token.rs` — std-only base64url, `map1.` parsing, expiry check, ed25519 verification behind `token-verify`, minimal ULID generation, unit tests.
- `client.rs` — handshake skeleton (feature `client`).

## Not yet (post-skeleton)

- Full request/reply multiplexing with timeouts and reconnection + `session.resume`.
- `snap.sync` chunk streaming (push/pull peer role).
- MAP-Bridge runner/agent harnesses.

```bash
cargo test   # token + ulid unit tests
```

Apache-2.0.
