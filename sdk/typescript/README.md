# @mixum/map

TypeScript SDK for the [Mixum Agent Protocol (MAP)](https://github.com/ashreo/Mixum-Agent-Protocol) — skeleton release.

The [specification](../../site/src/content/docs/spec) is authoritative; this SDK is a typed convenience over the wire.

## Install

```bash
npm install @mixum/map
```

(Not yet published to npm during skeleton phase — `npm install ../sdk/typescript` or `npm link` from a checkout.)

## Usage

```ts
import { MapClient } from "@mixum/map";

const client = await MapClient.connect({
  url: "wss://127.0.0.1:7601/ws",
  token: process.env.MAP_TOKEN!, // map1.… token from the Node / Host
});

const info = await client.sys.info();
const task = await client.task.submit({ instruction: "Hello, sandbox" });
client.task.onEvent((e) => console.log(e.type, e.data));

// time travel
const tree = await client.snap.tree();
await client.snap.restore({ node: tree.head, strategy: "inplace" });
```

### Routing through a gateway

Pass `node` to route every request to a registered node; `request()` accepts a
one-off override as its third argument:

```ts
const client = await MapClient.connect({ url, token, node: "mac-mini" });
await client.sys.info();                       // envelope.node = "mac-mini"
await client.request("sys.info", {}, "studio"); // one-off override
```

### Reconnecting a session

`session.resume` restores claims and subscriptions without a new handshake and
replays buffered pushes newer than `lastSeq`; handlers registered right after
resume receive the replay:

```ts
const client = await MapClient.connect({ url, token });
const { sessionId, lastSeq } = client;

// ... network hiccup / laptop sleep ...
const resumed = await MapClient.resume({ url, sessionId, lastSeq });
resumed.task.onEvent((event) => console.log("replayed or live", event));
```

### Pre-MAP 0.1.0 nodes

Nodes built before MAP adopted `session.hello`/`map.v1` speak `hello`/`mixum.v1`.
Opt into the transition dialect — no custom WebSocket shim needed:

```ts
const legacy = await MapClient.connect({ url, token, legacy: "mixum-v1" });
```

New nodes accept both dialects; the option only matters when the server is old.

### Tests

```bash
npm run build
npm run test:unit   # mock-socket wire tests (subprotocols, legacy, resume replay)
npm run test:e2e    # needs a MAP node:   MAP_URL=… MAP_TOKEN=… npm run test:e2e
                    # full image node:    MAP_FULL_NODE=1 (allows a live compositor,
                    #   skips the host-path agent test)
npm run test:legacy # needs a 0.1.0 node: NODE_TLS_REJECT_UNAUTHORIZED=0 \
                    #   MAP_URL=wss://… MAP_TOKEN=mixum1.… npm run test:legacy
```

`test/e2e-agentd.mjs` is the full-loop check; from a mixumOS checkout run
`scripts/map-sdk-e2e.sh`, which builds a plain-WS agentd harness and executes it.

## What's in the skeleton

- `types.ts` — wire types mirroring `schemas/*.schema.json` (envelope, token claims, task, snapshot node/manifest, memory record, bridge messages, window, VM).
- `errors.ts` — `MapError` with the stable error-code set.
- `ulid.ts` — canonical ULID generation (spec: envelope identifiers).
- `client.ts` — `MapClient`: WSS connect + `session.hello`, request/reply correlation by ULID with timeouts, push-envelope dispatch, binary video-frame header parsing, typed domain helpers (`sys`, `task`, `snap`, `display`, `mem`, `vm`), `MapClient.resume` + `lastSeq`, and the `legacy: "mixum-v1"` transition dialect.

## Not yet (post-skeleton)

- TLS TOFU pinning wrapper for Node (pass a custom `WebSocketImpl` today; the option is already plumbed).
- Snapshot chunk push/pull streaming (`snap.sync` peer role).
- MAP-Bridge runner/agent harnesses.
- Reactivity helpers for GUIs.

## Status

Experimental skeleton tracking MAP v1 Draft. Wire types follow the spec's stability markers; Experimental surfaces are not included.

Apache-2.0.
