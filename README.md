# Mixum Agent Protocol (MAP)

[![ci](https://github.com/EmergeShift/Mixum-Agent-Protocol/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/EmergeShift/Mixum-Agent-Protocol/actions/workflows/ci.yml)

**MAP** is an open protocol that connects AI agents to isolated, snapshot-capable computing environments — the "world" an agent lives in — across macOS, Windows and Linux hosts, from a laptop to the cloud.

MCP standardized how agents talk to *tools*. MAP standardizes how agents and their operators talk to *environments*: running tasks in a VM or container, traveling through time with snapshot trees, moving a whole working state between machines, watching and steering GUI windows, and carrying agent memory with them.

> Status: **v1 Draft**. The wire surface is implemented and battle-tested by [mixumOS](https://github.com/ashreo/mixumOS); this repository formalizes it as a vendor-neutral specification.

## Why MAP

Agent runtimes today are closed: every sandbox vendor invents private APIs for tasks, screenshots, filesystems and persistence. That locks agents and operator UIs to one backend. MAP defines a small, capability-secured, versioned surface so that:

- **Any client** (desktop GUI, phone PWA, CLI, another agent) can drive **any compliant node** (container, VM, cloud pod).
- **Any host OS** can run the runtime, and a working state can **move between hosts** (laptop → cloud → desktop) at any moment, including live process state.
- **Time travel is a primitive**: every environment exposes a snapshot tree with branches, rollback and fork — git semantics for machine state.
- **Agent memory is portable**: records follow one normalized schema and move between nodes and agent platforms.

## The ecosystem

| Project | Role |
|---|---|
| [mixumOS](https://github.com/ashreo/mixumOS) | The system base: Arch-based agent OS distribution with container/VM runtime (`mixum-agentd`, `mixcomp`). The reference MAP **Node** implementation. |
| **Mixum Agent Protocol** (this repo) | The contract: normative spec, JSON Schemas, conformance vectors, SDKs (TypeScript / Rust). |
| [Mixum Agent Env (MixumART)](https://github.com/ashreo/mixum-Agent-Env) | Desktop GUI and companion environment for macOS / Windows / Linux: VM lifecycle management, agent platform connectors. A reference MAP **Host** + **Client** implementation. |

## Design principles

1. **Capability-based security** — every connection holds a least-privilege, signed token; there is no ambient authority.
2. **Content-addressed everything** — snapshot chunks and manifests are addressed by plaintext SHA-256; compression and encryption are orthogonal, deduplication is universal.
3. **Event-sourced state** — tasks and snapshot trees are observable event streams; clients resync after reconnect.
4. **Negotiated, additive evolution** — versions are negotiated at hello; additive changes are minor; breaking changes are major and rare.
5. **Profiles, not a monolith** — implementations conform to MAP-Core plus optional profiles (Display, Mobility, Memory, Host). Clients degrade gracefully.
6. **Language-neutral wire** — JSON control plane + tagged binary frames, locked by JSON Schemas and executable test vectors.

## Repository layout

```
├── site/            Documentation site (Astro Starlight) — the normative spec lives here
├── schemas/         JSON Schemas (normative machine-readable forms)
├── vectors/         Conformance vectors (structural, language-neutral)
├── sdk/             SDKs: sdk/typescript, sdk/rust (map-core/client/server/snapshot/cli + .mart tooling)
├── scripts/         Repository checks (JSON Schema + vector validation)
└── rfcs/            Design process: RFCs and template
```

## Read the spec

- [TRUST.md](TRUST.md) — our verifiable commitments: no phone-home, credential custody, telemetry policy, no lock-in
- [What is MAP?](https://docs.emergeshift.com/map/) — **under construction** (tracking: [#11](https://github.com/EmergeShift/Mixum-Agent-Protocol/issues/11))
- Local: `cd site && npm install && npm run dev`

## License

Apache-2.0 — see [LICENSE](LICENSE).
