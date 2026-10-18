# Changelog

All notable changes to the MAP specification and repository artifacts are documented here.
Format based on [Keep a Changelog](https://keepachangelog.com/); the protocol itself follows
[Versioning & Governance](site/src/content/docs/spec/versioning-governance).

## [Unreleased]

### Added

- v1 Draft specification: overview, architecture & profiles, transport & security,
  envelope & framing, session & system, tasks, agents & bridge, snapshots & time travel,
  mobility, display, memory, VM & host, versioning & governance.
- JSON Schemas for the control plane (envelope, token, task, snapshot node/manifest,
  memory record, bridge messages).
- Structural conformance vectors (envelope, snapshot tree, bridge session).
- SDK skeletons: `sdk/typescript`, `sdk/rust`.
- Documentation site (Astro Starlight) with English normative text and Chinese
  translations of overview and core concept pages.
- RFC 0001: MAP v1 scope and relation to the mixumOS wire protocol.
