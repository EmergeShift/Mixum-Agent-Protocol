# RFC 0001 — MAP v1 scope and relation to the mixumOS wire protocol

* RFC: 0001
* Title: MAP v1 scope and relation to the mixumOS wire protocol
* Status: Accepted
* Type: Standards Track
* Author: ashreo
* Created: 2026-10-18

## Summary

MAP v1 formalizes the wire protocol that mixumOS has operated since 2026-10-04
(documented as "mixumOS 协议规范 v1"), generalizes it from a single
implementation's spec into a vendor-neutral open protocol, and extends it with
the domains mixumOS v1 did not cover: agent memory (`mem.*`), VM/host lifecycle
(`vm.*`), a normative agent bridge, session resume, and the human-input channel.

## Motivation

The mixumOS protocol doc is implementation-anchored: it names mixumOS crates and
internal constraints, uses the `mixum1.` token prefix, and evolves with the code.
A protocol that other runtimes (MixumART hosts, third-party nodes) and clients
can adopt needs an independent home, an English normative text, conformance
tiers, and a governance process.

## Decision

1. **MAP v1 = mixumOS wire v1 semantics + additive extensions.** The envelope
   shape, capability model, task state machine, snapshot tree semantics, CAS
   sync flow, takeover state machine and video frame header are carried over
   unchanged — they are field-tested (double-node e2e, 30fps soak, H.264 e2e).
2. **Renaming, minimally:** token prefix `mixum1.` → `map1.`; subprotocol
   `mixum.v1` → `map.v1`; env vars `MIXUM_*` → `MAP_*`. All structurally
   identical; adapters are trivial. mixumOS tracks MAP v1 as its wire spec.
3. **New in v1 (additive, so mixumOS can adopt incrementally):**
   * `mem.*` domain and the memory record schema (forced platform compatibility
     via importers in SDKs/Hosts; `opaque` preserves round-trip fidelity).
   * `vm.*` domain for the Host role (VM lifecycle, images, bootstrap handoff).
   * MAP-Bridge as a normative sub-protocol (was implementation-described).
   * `session.resume` with a ≥256-event replay ring; `task.reply` human-input
     channel; `snap.events`/`vm.events` subscriptions; `session.*`/`agent.*`
     op namespacing (legacy unnamespaced `hello` accepted in v1).
   * Error codes `timeout`, `unavailable`, `already_exists` added.
4. **Profiles instead of a monolith:** Core / Display / Mobility / Memory /
   Host; conformance is scoped per profile. This is what makes the protocol
   adoptable by small implementations.
5. **Governance:** RFC process, English-normative docs, schema + vector locking.

## Alternatives considered

* **Fresh protocol, clean slate.** Rejected: discards field-tested semantics
  and would orphan the reference implementation during rework.
* **Keep everything implementation-private, add adapters later.** Rejected:
  the user-facing lock-in problem MAP exists to solve.
* **Adopt an existing sandbox protocol.** No existing protocol covers snapshot
  trees, mobility and GUI takeover; wrapping one would inherit its release
  governance and still require this surface.

## Consequences

* mixumOS migrates its docs to reference MAP; `mixum1.` tokens remain accepted
  by mixumOS builds through a compatibility window (its own release policy).
* Vectors and schemas become the executable contract; crypto vectors follow
  the first SDK release.
* Gateway / multi-tenancy (MAP-Cluster) stays reserved and out of v1.
