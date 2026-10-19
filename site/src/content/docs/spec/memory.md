---
title: Memory
description: Portable agent memory — records, providers, search, import/export and cross-node sync.
---

Stability: **Stable at v1.0** (profile **MAP-Memory**; live cross-node sync is Experimental).

Agents accumulate context — preferences, facts, procedures, episode history. Today that memory is locked inside each agent platform. MAP makes it a **first-class, portable artifact** with one normalized schema, so memory follows the agent across Nodes, and platform memories can be imported ("forced compatibility") rather than abandoned.

## Goals

1. **Portability** — records move between Nodes (via [Mobility](/spec/mobility/) or export/import).
2. **Platform interop** — major platforms' memories are converted into MAP records by importers; the protocol itself stays neutral.
3. **User ownership** — memory lives in the user's Node, not in a vendor's cloud. Privacy is architectural.

## Model

- A **provider** is a named source/store of records: `builtin` (the Node's local store), `platform-import:chatgpt`, `platform-import:claude`, `remote:*`, or an implementation's own.
- A **record** is one normalized unit of memory:

```json
{
  "id": "01JBVB6Q…",
  "provider": "builtin",
  "kind": "preference",
  "content": {
    "text": "Prefers concise answers; skip preamble.",
    "structured": { "locale": "zh-CN", "verbosity": "low" }
  },
  "scope": { "level": "global" },
  "source": { "platform": "chatgpt", "origin": "import 2026-10-18" },
  "tags": ["style"],
  "createdAt": 1696400000123,
  "updatedAt": 1696400000123,
  "confidence": 0.9,
  "expiresAt": null,
  "opaque": null
}
```

| Field | Rules |
|---|---|
| `id` | ULID; server-assigned when omitted from `mem.put` |
| `provider` | Must exist on this Node |
| `kind` | `fact` \| `preference` \| `procedure` \| `episodic` \| `custom` |
| `content.text` | Required human-readable text (what gets injected into prompts) |
| `content.structured` | Optional key/value refinement |
| `scope.level` | `global` \| `project` \| `task`; `scope.ref` = project root or task ULID |
| `source` | Provenance; `platform` names the origin platform for imported records |
| `opaque` | Provider-specific blob (base64) preserved on round-trip so **no importer's data is lossy** |

## Ops

| Op | Request | Reply |
|---|---|---|
| `mem.providers` | — | `{ providers: [{ id, kind, status, capabilities }] }` — capabilities ⊆ `read,write,search,import,export` |
| `mem.search` | `{ query, kinds?, providers?, scope?, limit? }` | `{ hits: [{ record, score }] }` |
| `mem.get` | `{ id }` | Record |
| `mem.put` | `{ record }` | Record (server-assigned `id`/`updatedAt`) |
| `mem.delete` | `{ id }` | `{ deleted }` |
| `mem.list` | `{ provider?, kind?, cursor?, limit? }` | `{ records, nextCursor? }` |
| `mem.import` | `{ provider, format: "map-json", records: [Record] }` | `{ imported, skipped }` |
| `mem.export` | `{ provider?, format: "map-json" }` | `{ records: [Record] }` |

Notes:

- **Search ranking is implementation-defined** in v1 (substring/FTS/embeddings all conformant); `score` is normalized 0–1, order is relevance-descending. A Node with semantic search sets the provider capability `search` and advertises `searchSemantic: true` in `mem.providers`.
- `mem.put` with an existing `id` **updates** (source of truth: the last write); clients doing offline edits should read-modify-write.
- `mem.import` is upsert-by-`id`; records without `id` are always inserted. `skipped` counts records rejected by validation.
- `format` is `"map-json"` in v1; the registry may add named formats (e.g. `"chatgpt-export"`) — but platform-specific conversion belongs in **SDKs/Hosts** ([below](#platform-compatibility)), not in Nodes.

## Storage & mobility

- The memory store is node-local (`dataDir/memory`), **outside the workspace**: it is not part of `disk` snapshots by default.
- It travels by explicit choice: `snap.push { includeMemory: true }` carries the store's chunks ([Mobility](/spec/mobility/#memory-with-mobility)), or via `mem.export` / `mem.import` round-trips.
- Per-record ACLs and encrypted-at-rest records are reserved (v1.1); the store inherits the Node's at-rest encryption posture.

## Platform compatibility

Early MAP targets **forced compatibility** with the major agent platforms. The division of labor:

| Layer | Responsibility |
|---|---|
| **Importers** (in SDKs / MixumART) | Convert platform exports (ChatGPT memory, Claude memory, Cursor rules, …) → `map-json` records; keep originals in `opaque` |
| **Protocol** | Move and store normalized records; guarantee lossless round-trips via `opaque` |
| **Nodes** | Expose providers; apply records where the local agent runner can consume them |

Importer coverage is tracked in the SDK documentation, not in the spec — the spec only requires that an importer be expressible, which `opaque` + `source.platform` guarantee.

## Experimental: live sync

`mem.sync { remote, since? }` (continuous record replication between Nodes) is **Experimental** in v1: it may change in minor versions and is excluded from conformance. Nodes must not enable it by default.
