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

## What's in the skeleton

- `types.ts` — wire types mirroring `schemas/*.schema.json` (envelope, token claims, task, snapshot node/manifest, memory record, bridge messages, window, VM).
- `errors.ts` — `MapError` with the stable error-code set.
- `ulid.ts` — canonical ULID generation (spec: envelope identifiers).
- `client.ts` — `MapClient`: WSS connect + `session.hello`, request/reply correlation by ULID with timeouts, push-envelope dispatch, binary video-frame header parsing, typed domain helpers (`sys`, `task`, `snap`, `display`, `mem`, `vm`).

## Not yet (post-skeleton)

- TLS TOFU pinning wrapper for Node (pass a custom `WebSocketImpl` today; the option is already plumbed).
- Snapshot chunk push/pull streaming (`snap.sync` peer role).
- MAP-Bridge runner/agent harnesses.
- Reactivity helpers for GUIs.

## Status

Experimental skeleton tracking MAP v1 Draft. Wire types follow the spec's stability markers; Experimental surfaces are not included.

Apache-2.0.
