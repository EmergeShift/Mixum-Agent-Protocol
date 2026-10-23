# Changelog

All notable changes to the MAP specification and repository artifacts are documented here.
Format based on [Keep a Changelog](https://keepachangelog.com/); the protocol itself follows
[Versioning & Governance](site/src/content/docs/spec/versioning-governance.md).

## [Unreleased]

### Added

- v1 Draft specification: overview, architecture & profiles, transport & security,
  envelope & framing, session & system, tasks, agents & bridge, snapshots & time travel,
  mobility, display, memory, VM & host, versioning & governance.
- JSON Schemas for the control plane (envelope, token, task, snapshot node/manifest,
  memory record, bridge messages).
- Structural conformance vectors (envelope, snapshot tree, bridge session).
- SDK skeletons: `sdk/typescript`, `sdk/rust`.
- Rust workspace for M-ART tooling (`sdk/rust`): `map-snapshot` implements the
  `.mart` single-file snapshot (rkyv zero-copy archive, content-addressed
  chunks, pack/open/verify/restore/diff, base-archive chunk reuse);
  `map-client` adds WSS `session.hello` + request/reply; `map-server` adds the
  `OpHandler`/`Router` node-side dispatch; `map-cli` ships the `mart` binary.
  PyO3 / wasm-bindgen bindings and `spec/mart-binary-spec.md` document the
  format. Layout note: `map-core` moved to `sdk/rust/crates/map-core`, and its
  old `client` feature is now the separate `map-client` crate.
- Documentation site (Astro Starlight) with English normative text and Chinese
  translations of overview and core concept pages.
- RFC 0001: MAP v1 scope and relation to the mixumOS wire protocol.
- Privilege extension (`spec/privilege`): one-shot root requests, approval and
  the audit boundary. Added `schemas/privilege.schema.json` and
  `vectors/privilege/approval-flow.json` covering approve/deny/timeout/expiry,
  executable pinning and audit records; the docs sidebar now links the page.
- CI: four jobs (schema/vector validation, Rust SDK, TypeScript SDK, docs site)
  and `scripts/validate_artifacts.py`, the repository schema/vector checker that
  runs locally and in CI.

### Changed

- Privilege spec aligned with the hardened reference Node: pinned `resolved`
  path + `sha256`, `peerUid`, approver provenance, late decisions recorded as
  `expired`, audit record shape and request limits. Fixed the example expiry
  (`createdAt + 120000` ms, previously +120 ms).
- Display: documented the private Node↔compositor control channel's
  forward-compatibility rule — unknown record types are ignored and counted;
  framing/payload validation errors are an explicit error path (minor
  clarification, no wire change). Closes the spec side of EmergeShift/mixumOS#8.
