# RFC 0002 — Snapshot tree: branch rename constraint and delete-head handoff

* RFC: 0002
* Title: Snapshot tree — branch rename constraint and delete-head handoff semantics
* Status: Accepted
* Type: Standards Track
* Author: ashreo
* Created: 2026-10-19

## Summary

Two amendments to [Snapshots & Time Travel](../site/src/content/docs/spec/snapshots.md),
both originating from a review of the reference implementation's PR #6
(EmergeShift/mixumOS):

1. **Branch rename constraint** — branch rename is not a v1 wire op, but
   implementations expose it (mixumOS's tree layer does). When exposed, the
   rename MUST rewrite the `branch` attribute of every member node. The
   pre-fix reference implementation only updated branch metadata: an in-place
   restore of an old node wrote back the node's recorded old branch name and
   **resurrected the renamed-away branch**.
2. **Delete-head handoff** — when `snap.delete` removes a leaf that is a
   branch's head, the head moves to the leaf's parent (a root leaf takes the
   branch entry with it). Previously the heads entry was simply removed,
   leaving the branch headless: new snapshots on that branch had no defined
   attach point.

## Decision

Both semantics are folded into the v1 Draft spec (`snapshots.md` Notes) and
locked by a new case in `vectors/snapshot-tree/tree-scenario.json`.

- `snap.rename` is **not** registered as a wire op in v1 — the constraint
  above governs implementations that expose rename as an extension. If it
  graduates to a wire op, that is an additive (minor) change with its own RFC.
- Delete-head handoff defines existing `snap.delete` behavior; it is
  backward-compatible with trees that never hit the case and fixes headless
  branches for all that do.

## Consequences

- Conformance: the new vector case is part of MAP-Core tree semantics.
- Reference implementation (mixumOS) already implements both via PR #6.
