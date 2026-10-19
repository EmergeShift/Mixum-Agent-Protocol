---
title: Mobility
description: Moving snapshots between Nodes — the CAS sync protocol, idempotency, compatibility and migration playbooks.
---

Stability: **Stable**.

Mobility is the headline capability: **a working state — files, optionally live process memory — moves between Nodes** on different machines, between a laptop and the cloud, or between clouds. Sync is peer-to-peer: the pushing Node acts as a MAP client of the receiving Node with a `snap.sync` token.

## Sync protocol (normative)

Push node **A** → node **B**, for snapshot node `N` (and its ancestors as needed):

1. **A → B `snap.push.import`**

   ```json
   {
     "node": { "id", "parent?", "branch", "label?", "kind", "createdAt",
               "kernel?", "arch?", "origin" },
     "manifests": { "disk": {…}?, "memory": {…}? },
     "chunks": ["<hash>", "…"]
   }
   ```

2. **B → A** replies with the hashes it does **not** have:

   ```json
   { "missing": ["<hash>", "…"] }
   ```

   B checks its CAS (and may consult other stores) per hash. Everything else is deduplicated — re-pushing a node costs **zero bytes**.

3. **A → B** streams each missing chunk as a binary frame:

   ```
   [0x03][32 bytes: plaintext SHA-256][chunk bytes (plaintext)]
   ```

   Order is irrelevant; B verifies each chunk's hash on arrival. Chunks land in B's CAS as they arrive — a retry after a cut connection re-runs steps 1–2 and finds most chunks present.

4. **A → B `snap.push.commit`** `{ count }`. B verifies all `missing` chunks have arrived, then attaches the node to its tree.
   - If `N.parent` does not exist in B's tree, `N` becomes the **root of a new remote branch** and gets its `origin` recorded; B appends `"(remote root)"` to the label. No cross-tree dangling references, ever.
   - On success B → A `{ node: "<id>", stored: true }`.

Error handling: any validation failure (manifest path traversal, hash mismatch, oversized chunk) aborts the import with `snapshot_invalid` / `bad_request`; already-landed chunks stay in the CAS (they are content-addressed and will dedup on retry).

## Memory with mobility

With **MAP-Memory**, a push may carry the Node's memory store: `snap.push { …, includeMemory: true }` pushes additional chunks for the memory store and sets `node.memoryStore: { chunks }`. On the target, restoring the node restores its memory records with it. Without MAP-Memory, mobility moves workspace and process state only.

## Compatibility

The manifest records `kernel` and `arch` of the originating Node. The matrix:

| From \ To | same arch, same kernel family | different arch | different kernel |
|---|---|---|---|
| `disk` snapshot | restore directly | restore directly (files are portable; modes preserved) | restore directly |
| `memory` snapshot | restore | **reject** (`snapshot_invalid`) | **reject** |
| `hybrid` | restore | disk part only, with `force` | disk part only, with `force` |

Pushing is always allowed (the target just records metadata); **restore** is where guards apply. This keeps sync trivially future-proof while never pretending a CRIU image is portable across kernels.

## Playbooks

**Laptop lid close → cloud, and back.**

1. The Host watches power events; before sleep it triggers `snap.create { kind: "hybrid" }` (process tree freezes for a moment, invisible).
2. `snap.push { node, remote: "cloud-node", token }` — only missing chunks travel.
3. The cloud Node restores the node (`snap.restore { strategy: "inplace" }`): tasks resume where they were, windows re-open.
4. Back at the desk: pull = the cloud pushes the (now newer) head back, or the laptop restores from the cloud head. Both directions are the same protocol.

**Machine → machine (decommissioning).** Push the whole tree (`snap.push` per node), or export/import the CAS store out-of-band (the store directory is portable by construction: content-addressed, no database to reconcile).

**Idempotency.** Every step is safe to re-run: import is a pure diff, chunks are content-addressed, commit is idempotent (committing an already-present node replies ok with `stored: true, alreadyPresent: true`).
