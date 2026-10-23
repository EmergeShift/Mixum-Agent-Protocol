---
title: Snapshots & Time Travel
description: Content-addressed storage, manifests, the snapshot tree DAG, restore/fork semantics and GC.
---

Stability: **Stable** (tree semantics and manifest format are frozen for v1).

Every Node exposes a **snapshot tree** over its workspace (and optionally the task process tree). Time travel is a primitive, not a feature bolt-on: tasks snapshot themselves before running, users roll back and fork at will, and nothing is ever lost by rolling back.

## Snapshot kinds

| Kind | Captures | Typical cost | Use |
|---|---|---|---|
| `disk` | Workspace filesystem (files, symlinks, dirs) | Milliseconds when hot (chunk-deduped) | Pre-task snapshots, save points |
| `memory` | Task process tree via CRIU | Seconds | Process migration/resume |
| `hybrid` | Both | Sum | Device handoff (lid close → cloud), full-state travel |

v1 restriction: a pure `memory` snapshot may not be restored on its own — it must restore onto a matching `disk` state.

## Content-addressed store (CAS)

- Files are split into chunks of **1 MiB nominal** (tail chunk may be shorter). The chunk id is the **SHA-256 of the plaintext chunk** — always, regardless of on-disk compression or encryption.
- On disk: `objects/<h[:2]>/<h[2:]>`, zstd-compressed, written to a temp file then atomically renamed.
- Optional at-rest encryption: AES-256-GCM per [Transport & Security](/map/spec/transport-security/#at-rest-encryption-optional-normative-when-enabled).
- Reads verify: recomputed hash must equal the address; mismatch = corruption, surfaced as `snapshot_invalid`.

## Manifest

```json
{
  "version": 1,
  "files": [
    { "path": "src/main.rs", "mode": 420, "size": 18432, "chunks": ["<hash>", "<hash>"] },
    { "path": "link-to-notes", "mode": 420, "size": 11, "symlink": "notes.md", "chunks": [] }
  ],
  "dirs": [ { "path": "src", "mode": 493 } ]
}
```

- `mode` is the octal permission as integer (420 = 0644, 493 = 0755).
- Paths from untrusted sources (remote pushes) must be rejected when absolute or containing `..`.
- Sockets/fifos/devices are recorded by type only and not materialized.
- Manifest diff (added/removed/changed) is how Nodes compute push sets and how clients render "what changed between n2 and n3".

## Tree semantics

```
SnapNode {
  id, parent?, branch, label?, kind, auto?, createdAt,
  origin?, sizeBytes, chunks, kernel?, arch?,
  disk?: Manifest, memory?: Manifest
}
```

- **A branch is an attribute of nodes; the head is a pointer.** A new snapshot always attaches under the current head and becomes that branch's new head.
- **In-place restore** (`snap.restore { strategy: "inplace" }`): the head moves back to an older node; the workspace is materialized to it. Continued work then forks a **parallel timeline** naturally — the old future is not deleted.
- **Explicit fork** (`snap.fork { node, branch }`): point a (new) branch's head at any node without touching the workspace. Name collisions auto-suffix `-2`, `-3`, …
- **Any past node can be restored directly**, including "future" nodes on other timelines — nothing is lost by rolling back.
- `origin` records where a node was imported from (host name / node id) — trees show their migration history.
- `tags` is optional extension metadata (for example `base`, `baseDigest`). Restoring a
  node whose tags name a different base should be refused unless the operator
  explicitly overrides it.
- Container/VM initialization creates the **`genesis`** snapshot (empty workspace): the root of every tree.

### A user story

```
20:00  snapshot n2 (pre-task)          main: genesis ── n1 ── n2 ── n3
22:00  workspace changed, snapshot n3
22:30  restore(n2, inplace), keep working → n4
                                                 └─ n4   ← parallel timeline
23:00  restore(n3, fork, "aggressive-tuning") → new branch from n3
```

## Ops

| Op | Request | Reply |
|---|---|---|
| `snap.create` | `{ kind: "disk"\|"memory"\|"hybrid", label?, parent?, branch?, auto? }` | SnapNode |
| `snap.tree` | — | `{ head, branch, nodes: [SnapNode] }` |
| `snap.restore` | `{ node, strategy: "inplace"\|"fork", label?, force? }` | `{ restoredTo, newBranch?, newNode? }` |
| `snap.fork` | `{ node, branch }` | `{ branch }` |
| `snap.delete` | `{ node }` | `{ deleted }` |
| `snap.gc` | — | `{ removed }` |
| `snap.events` | — | `{ subscribed }` — pushes `snap.event` on tree changes |

Notes:

- `snap.create { parent }` may target any node (not just head) — used by runners for mid-task checkpoints on a specific timeline.
- `snap.delete` only deletes **leaf** nodes; deleting an interior node is `conflict` (no dangling references, ever). When the deleted leaf is a branch's **head**, the head moves to the leaf's parent; if the leaf was also the branch root (no parent), the branch entry is removed with it. **A branch never loses its head through deletion.**
- Branch **rename** is not a v1 wire op. Implementations MAY expose it as an extension; when they do, the rename MUST rewrite the `branch` attribute of every member node — otherwise an in-place restore of an old node resurrects the old branch name.
- `snap.restore` guards: a `memory`/`hybrid` snapshot whose `kernel` or `arch` differs from the local node is rejected with `snapshot_invalid` unless `force: true`, and `force` may only bypass for the `disk` portion. Cross-kernel CRIU restore is deliberately unsupported in v1.

## Garbage collection

- Reachable set = all chunks referenced by all **existing** tree nodes. Because only leaves can be deleted, there is no "deleted parent, alive child" intermediate state.
- Sweep removes unreferenced chunks; chunks written in the last **10 minutes** (mtime window) are protected so a concurrent push/import can never have its chunks swept mid-transfer.
