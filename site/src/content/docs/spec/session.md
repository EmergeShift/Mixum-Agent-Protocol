---
title: Session & System
description: Handshake, version/profile negotiation, heartbeat, resume, and system introspection ops.
---

Stability: **Stable**.

## Handshake

After the WebSocket upgrade, the client must send `hello` before anything except `sys.ping`. Servers must reject every other op with `unauthorized` until the handshake completes.

**Request** `session.hello` — legacy alias `hello` accepted in v1:

```json
{
  "token": "map1.eyJ…",
  "client": "mixumart/0.3.1",
  "device": "studio-mac",
  "profiles": ["core", "display", "memory"]
}
```

**Reply** `session.hello.ok`:

```json
{
  "sessionId": "01JBV8…",
  "server": "mixum-agentd/0.2.0",
  "arch": "aarch64",
  "kernel": "6.12.8-arch1-1",
  "caps": ["task.run", "task.observe", "display.view", "display.input"],
  "profiles": ["core", "display"],
  "versions": [1],
  "heartbeat": 30
}
```

| Field | Meaning |
|---|---|
| `sessionId` | ULID; used by `session.resume` |
| `caps` | The capabilities granted by the token (intersection with server support) |
| `profiles` | Profiles this Node implements (the client's `profiles` hint is advisory only) |
| `versions` | Wire major versions supported; **the intersection with the client's `v: 1` envelope is the protocol for this connection**. Empty intersection ⇒ `unsupported` |
| `heartbeat` | Seconds; client must call `sys.ping` within 1.5× this interval or the server drops the connection |

## Heartbeat

- Client → `sys.ping` → `{ "pong": "<request id>" }`.
- The server may push `session.ping` at any time; clients should reply `session.ping.ok` (payload ignored). A server MAY use this to probe half-open connections.

## Resume

For GUI clients that hop networks (laptop sleep, Wi-Fi → hotspot), `session.resume` restores the subscription state without re-subscribing to every stream.

**Request** `session.resume`:

```json
{ "sessionId": "01JBV8…", "lastSeq": 418 }
```

**Reply** `session.resume.ok`:

```json
{ "resumed": true, "replayed": 7 }
```

Semantics:

- The server keeps a per-session ring buffer of at least **256** envelopes pushed to that session (task events, window events, tree events — all push-direction control messages **except clipboard events**).
- On resume with `lastSeq`, the server replays every buffered envelope newer than `lastSeq` before any new pushes, preserving order.
- `display.clipboard.changed` is deliberately **not replayed**: clipboard text is sensitive and may be stale. A resumed client must not treat a missing event as an empty clipboard; v1 has no per-window read, so it should wait for the next change event (or use the legacy `display.clipboard` pull where that compositor supports it).
- Unknown/expired `sessionId` → `not_found`; the client must fall back to a full `session.hello` and re-attach.
- Binary channels are **not** replayed. Clients re-issue `display.attach` and rely on snapshot-tree events (not chunk replay) for sync state.

## System ops

| Op | Request | Reply |
|---|---|---|
| `sys.ping` | — | `{ "pong": "<request id>" }` |
| `sys.info` | — | SysInfo object |

`sys.info` reply (fields absent when the backing feature is unavailable):

```json
{
  "server": "mixum-agentd/0.2.0",
  "arch": "aarch64",
  "kernel": "6.12.8-arch1-1",
  "os": "Arch Linux",
  "hostname": "task-vm-01",
  "caps": ["admin"],
  "profiles": ["core", "display", "mobility", "memory"],
  "snapshotBackend": "directory",
  "criuAvailable": true,
  "compositor": "mixcomp/0.4.0",
  "windows": 2,
  "memoryProviders": ["builtin"],
  "uptimeSeconds": 86200
}
```

`sys.info` is allowed pre-handshake **only** to report `profiles` and `server` so clients can render meaningful connection errors; all other pre-handshake ops are `unauthorized`.
