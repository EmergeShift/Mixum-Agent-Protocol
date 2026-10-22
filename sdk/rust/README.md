# MAP Rust SDK & M-ART snapshot tooling

Rust workspace for the [Mixum Agent Protocol (MAP)](../../README.md) and its
reference snapshot format `.mart`.

The [specification](../../site/src/content/docs/spec) is authoritative; this
workspace is a convenience over the wire. Wire JSON is camelCase, unknown fields
are tolerated, and identifiers are 26-char ULIDs.

## Crates

| Crate | What it gives |
|---|---|
| [`map-core`](crates/map-core) | Wire types mirroring `schemas/*.schema.json`, stable `ErrorCode`, `map1.` token parsing/verification, ULID |
| [`map-snapshot`](crates/map-snapshot) | `.mart` format: rkyv zero-copy archive, content-addressed chunks, pack/open/verify/restore/diff |
| [`map-client`](crates/map-client) | Async WSS client: `session.hello` + request/reply correlation with timeout |
| [`map-server`](crates/map-server) | Node-side routing: `OpHandler` trait, `Router`, `.ok`/`.err` envelope shaping |
| [`map-cli`](crates/map-cli) | `mart` binary: pack, inspect, verify, restore, diff |

Bindings and examples:

```text
bindings/map-py      PyO3 bindings for .mart snapshots (opt-in build)
bindings/map-wasm    wasm-bindgen helpers (format probe, path safety)
spec/                .mart binary format specification
crates/*/examples/   instant_roaming.rs, rust_agent_sandbox.rs
```

## Quick start

```bash
cd sdk/rust
cargo test                    # protocol types, tokens, snapshot roundtrip, router
cargo run -p map-cli -- pack /tmp/world -o /tmp/world.mart
cargo run -p map-cli -- inspect /tmp/world.mart --manifest-only
cargo run -p map-cli -- verify /tmp/world.mart
cargo run -p map-cli -- restore /tmp/world.mart /tmp/restored
cargo run -p map-snapshot --example instant_roaming
```

Measured on one dev machine (Windows, debug build): a 4 MiB / 2-file tree packs
in ~60 ms, opens zero-copy in ~0.23 ms, and restores in ~4 ms. Run the example
on your target to get its numbers; release builds and warmer caches move them.

## Snapshot library

```rust
use map_snapshot::{pack, MartArchive, PackOptions};

let stats = pack("/workspace", "world.mart", &PackOptions::default())?;
let archive = MartArchive::open("world.mart")?;   // mmap + digest check
assert!(archive.verify()?.ok);                    // re-hash every chunk
archive.restore("/restored")?;                    // straight from the mapping
```

`PackOptions::base = Some("old.mart".into())` reuses unchanged chunks from a
previous archive, so an incremental snapshot only appends new bytes.
`MartArchive::diff(&other)` reports added/removed/modified files and chunk
deltas. See [spec/mart-binary-spec.md](spec/mart-binary-spec.md) for the exact
layout, alignment and integrity rules.

## Client and server

```rust
use map_client::MapClient;

let (mut client, hello) = MapClient::connect(url, token, "my-agent/0.1").await?;
// Behind a gateway: route every request to a node by name.
let mut client = client.with_node("mac-mini");
let task = client.call("task.submit", serde_json::json!({
    "instruction": "run the test suite",
    "agent": "shell",
})).await?;
```

```rust
use map_server::Router;

let mut router = Router::new();
router.route_fn("sys.ping", |_| async { Ok(serde_json::json!({ "pong": true })) });
let reply = router.dispatch(request).await;   // shapes .ok / .err for you
```

## Token features

`map-core` has one optional feature: `token-verify` pulls `ed25519-dalek` and
enables signature verification against a Node identity key. Parsing, expiry
checks and ULID generation are always available and dependency-free.

## Bindings

```bash
cargo build --manifest-path bindings/map-py/Cargo.toml --release
cargo build --target wasm32-unknown-unknown --manifest-path bindings/map-wasm/Cargo.toml
```

Both are excluded from the workspace so a plain `cargo test` never needs a
Python toolchain or a wasm target.

## Not yet

- Multiplexed client routing with `session.resume` replay and subscription
  streams (the current client skips non-matching events).
- `snap.sync` chunk streaming and cross-node transfer.
- MAP-Bridge runner/agent harnesses.
- Compression/encryption variants of `.mart` (the chunk table reserves
  `raw_len` for it).

Apache-2.0.
