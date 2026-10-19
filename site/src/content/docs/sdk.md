---
title: SDKs
description: TypeScript and Rust SDK skeletons for MAP clients and tooling.
---

SDKs are conveniences over the wire; the [specification](/spec/overview/) is always the definition. Both SDKs ship as **skeletons** with the core types and client structure; the wire behavior they must satisfy is exactly what the [schemas](/artifacts/) and vectors lock.

## TypeScript — `@mixum/map`

 [`sdk/typescript/`](https://github.com/ashreo/Mixum-Agent-Protocol/tree/main/sdk/typescript)

```ts
import { MapClient } from "@mixum/map";

const client = await MapClient.connect({
  url: "wss://127.0.0.1:7601/ws",
  token: process.env.MAP_TOKEN!,
});

await client.task.submit({ instruction: "…" });
client.task.onEvent((e) => console.log(e.type, e.data));
```

Structure: `src/types.ts` (wire types mirroring the schemas), `src/errors.ts` (`MapError` with stable codes), `src/client.ts` (connect/hello, request-reply correlation by ULID, timeouts, binary frame dispatch, event subscriptions, domain helpers). Zero runtime dependencies beyond the standard library — WebSocket is global in Node ≥ 22 and browsers.

## Rust — `map-core`

 [`sdk/rust/`](https://github.com/ashreo/Mixum-Agent-Protocol/tree/main/sdk/rust)

```rust
use map_core::{Envelope, Task, SnapNode};

let env = Envelope::request("task.submit", serde_json::json!({
    "instruction": "…"
}));
```

Structure: `types.rs` (serde types mirroring the schemas, `deny_unknown_fields` off, `camelCase` on the wire), `error.rs`, `token.rs` (map1 parsing/verification — ed25519 behind a feature flag), `client.rs` (tokio-tungstenite client skeleton behind `client` feature). The types crate is dependency-light on purpose: embedding targets (the mixumOS runner) compile it everywhere.

## Status

| SDK | State | First target |
|---|---|---|
| TypeScript | Skeleton (types + client structure) | Browser/PWA + Node clients (MixumART console) |
| Rust | Skeleton (types + token) | mixumOS crates, CLIs, sync peers |

Both follow the spec's stability markers: code for Experimental features lands behind explicit feature flags, never in the default path.
