---
title: Versioning & Governance
description: Version negotiation, stability guarantees, deprecation policy, RFC process and conformance.
---

Stability: **Stable** (this chapter defines how stability itself works).

## Version model

- The protocol version is **MAJOR.MINOR** in spirit (semver), carried on the wire by `envelope.v` (the major only) and `session.hello.ok.server` (full implementation string).
- **Minor (additive, backward-compatible):** new ops; new optional request/reply fields; new capability strings; new codecs; new error codes; new profile ids; new memory kinds. Receivers must ignore unknown fields and reject unknown ops with `unsupported` (not crash).
- **Major (breaking):** changing the semantics of anything; removing or renaming fields/ops/codes; changing any binary layout (frame header, chunk framing, bridge messages).

## Negotiation

1. Client connects with subprotocol `map.v1` and sends envelopes with `"v": 1`.
2. `session.hello.ok.versions` lists the majors the Node supports.
3. The connection's protocol = intersection. Empty intersection ⇒ `unsupported`, connection closes.
4. Within an accepted major, implementations signal finer capabilities via `caps` and `profiles` — clients must feature-probe, never version-probe, for optional features.

## Stability markers

Every spec chapter carries a **Stability** line:

| Marker | Guarantee |
|---|---|
| **Stable** | Frozen semantics within the major. Conformance vectors exist. Breaking change = major version. |
| **Stable at v1.0** | Draft today; frozen when v1.0 ships. Feedback window is now. |
| **Experimental** | May change in minors. Excluded from conformance. Nodes must not enable by default. |

## Deprecation

1. The spec marks an item *Deprecated* and names its replacement (minimum one minor version of notice).
2. Implementations may emit a warning (control message or log) when deprecated items are used.
3. Removal happens **only** in a major version.

## Governance

See [GOVERNANCE.md](https://github.com/ashreo/Mixum-Agent-Protocol/blob/main/GOVERNANCE.md) for the full text. In short:

- **RFC-first**: semantic changes go through `rfcs/` (Idea → Draft → Last Call → Accepted → Implemented); typos and clarifications don't.
- **English is normative**; translations must never invent semantics.
- **Maintainers** merge RFCs and cut spec releases; the registry of capability strings, codecs, error codes and profiles is owned by the spec (see the schemas and this site; a `REGISTRY.md` consolidates additions per release).

## Conformance

An implementation claims conformance **scoped by profile** (e.g. *"MAP v1 Core + Display conformant"*) when it:

1. Implements every op of the claimed profiles with the exact request/reply shapes of the JSON Schemas in [`schemas/`](https://github.com/ashreo/Mixum-Agent-Protocol/tree/main/schemas);
2. Passes the vectors for those profiles in [`vectors/`](https://github.com/ashreo/Mixum-Agent-Protocol/tree/main/vectors);
3. Rejects everything it does not implement with `unsupported` instead of guessing.

Self-certification lives in the implementation's README; a community conformance runner is planned post-v1.0. Vendors claiming conformance without passing vectors are asked to correct or remove the claim — the community treats false claims as a governance matter.
