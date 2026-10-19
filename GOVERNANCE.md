# MAP Governance

## Roles

- **Maintainers** merge RFCs, spec changes and releases. Initial maintainer: [@ashreo](https://github.com/ashreo) (project lead). New maintainers are added by consensus of existing maintainers after sustained, high-quality contributions.
- **RFC authors** drive proposals through the RFC lifecycle; authorship is recorded in the RFC header.
- **Implementers** ship MAP Nodes, Hosts and Clients and report conformance.

## Decision process

1. **Idea** — issue tagged `rfc`.
2. **Draft** — `rfcs/NNNN-title.md`, open PR, discussion window of at least 14 days.
3. **Last Call** — maintainer declares it; at least 7 more days; blocking objections must be resolved or documented.
4. **Accepted** — merged into `rfcs/`, spec PR follows.
5. **Implemented** — vectors + schema + at least one independent implementation exist.

For trivial, non-semantic fixes (typos, clarifications) the RFC step is waived.

## Versioning ownership

- The **major version** of the protocol is owned by the spec, not by any implementation.
- `envelope.v` in the wire format is the single negotiation anchor. Additive changes (new ops, new optional fields, new capability strings, new codecs) are minor. Semantic changes, removals, and binary format changes are major.
- Stability guarantee: ops marked **Stable** in the spec do not change semantics within a major version. Ops marked **Experimental** may change in minors and are excluded from conformance.

## Conformance

An implementation may claim "MAP v1 Core conformant" (or plus profiles: Display, Mobility, Memory, Host) when it passes the conformance vectors in `vectors/` for that scope. Self-certification is documented in the implementation's README; a community conformance runner is planned post-v1.0.
