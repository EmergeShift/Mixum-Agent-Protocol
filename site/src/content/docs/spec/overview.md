---
title: What is MAP?
description: The Mixum Agent Protocol — purpose, ecosystem, design principles and terminology.
---

MAP (**Mixum Agent Protocol**) is an open protocol that connects AI agents to **isolated, snapshot-capable computing environments** — the "world" an agent lives in — on macOS, Windows and Linux hosts, from a local laptop to the cloud.

An agent that can only call tools is still trapped in someone else's sandbox. MAP makes the sandbox itself interoperable:

- **Task execution with isolation** — run work inside a VM or container (a MAP *Node*), with process-level sandboxing and a guaranteed pre-task snapshot.
- **Time travel** — every Node exposes a snapshot tree: rollback to any point, branch parallel timelines, fork experiments. Git semantics for machine state.
- **Mobility** — move the entire working state (files, optionally live process memory) between Nodes on different machines or between local and cloud, idempotently and cheaply.
- **Windows and interaction** — GUI applications opened by the agent stream as native windows to any client, with a well-defined takeover state machine.
- **Agent memory** — one normalized record schema with import/export adapters, so memory follows the agent across Nodes and agent platforms.

## The problem

Today, every agent sandbox vendor invents private APIs for the same small set of needs: run a task, show me what happened, save and restore, move my work. Clients get locked to one backend, and agents get locked to one runtime. MAP defines a small, capability-secured, versioned surface so that **any client** can drive **any compliant Node**.

MCP standardized the agent↔tool edge. MAP standardizes the agent↔environment edge. They are complementary: an MCP server can run inside a MAP Node; a MAP Node can host any agent that speaks MAP-Bridge.

## The ecosystem

| Project | Role | MAP conformance |
|---|---|---|
| [mixumOS](https://github.com/ashreo/mixumOS) | The system base: an Arch-based Agent OS distribution. Containers/VMs running `mixum-agentd` + `mixcomp` are the reference **Node** implementation. | Core, Display, Mobility (reference implementation) |
| **Mixum Agent Protocol** (this repo) | The contract: normative specification, JSON Schemas, conformance vectors, SDKs. | — |
| [Mixum Agent Env (MixumART)](https://github.com/ashreo/mixum-Agent-Env) | Desktop GUI and companion environment: VM lifecycle on macOS/Windows/Linux, agent platform connectors. Reference **Host** + **Client**. | Host, Core client |

## Design principles

1. **Capability-based security.** Every connection holds a least-privilege signed token. There is no ambient authority and no "logged-in user" — only capabilities.
2. **Content-addressed everything.** Snapshot chunks and manifests are addressed by the SHA-256 of their plaintext. Compression and encryption are storage details; deduplication is universal and hash verification is total.
3. **Event-sourced state.** Tasks and snapshot trees are observable event streams. Clients resync after reconnect; the server is the source of truth.
4. **Negotiated, additive evolution.** Versions are negotiated at hello. Additive changes (new ops, optional fields, capabilities, codecs) are minor. Breaking changes are major and rare.
5. **Profiles, not a monolith.** Implementations conform to **MAP-Core** plus optional profiles (Display, Mobility, Memory, Host). Clients degrade gracefully when a profile is absent.
6. **Language-neutral wire.** A JSON control plane plus tagged binary frames, locked by JSON Schemas and executable conformance vectors. SDKs are conveniences, never the definition.

## Terminology

| Term | Definition |
|---|---|
| **Node** | The sandbox runtime inside a VM or container (e.g. mixumOS). Serves tasks, snapshots, display, memory. |
| **Host** | A desktop-side daemon (e.g. MixumART) that manages local VMs and hands out bootstrap access to their Nodes. |
| **Client** | A consumer of the protocol: GUI, phone PWA, CLI, or another agent. |
| **Peer** | Another Node exchanging snapshot data with this Node (push/pull). |
| **Gateway** | Cloud-side routing and multi-tenancy layer. Reserved; not part of MAP v1. |
| **Task** | A unit of agent work with a lifecycle, instruction, events, and a pre-task snapshot. |
| **Snapshot (SnapNode)** | A point-in-time state of the workspace and/or task process tree, stored as a node in a snapshot tree. |
| **Branch / Head** | A branch is an attribute of snapshot nodes; the head is the pointer where the next snapshot attaches. |
| **CAS** | Content-addressed store: chunks keyed by plaintext SHA-256. |
| **Manifest** | The file inventory of a snapshot: paths, modes, sizes, chunk lists. |
| **Capability token** | `map1.<payload>.<sig>` — an ed25519-signed set of capabilities with TTL and optional bindings. |
| **Profile** | A named conformance tier: MAP-Core, Display, Mobility, Memory, Host. |
| **Bridge (MAP-Bridge)** | The normative line-delimited JSON sub-protocol between a Node's runner and an external agent process. |
| **Window** | A GUI window of the sandbox exposed for streaming and (guarded) input. |
| **Memory record** | A normalized unit of agent memory with provider, kind, scope and content. |

## Non-goals (v1)

- **Multi-tenancy and public cloud routing** — the Gateway is reserved for a future version; v1 is single-user, multi-device.
- **Agent-to-agent protocols** — use MCP or similar inside a Node; MAP carries the environment, not agent conversations with each other.
- **A specific agent framework** — MAP-Bridge is deliberately tiny so any framework can speak it in an afternoon.
- **Browser-side sandboxing** — Nodes run in VMs/containers on real hosts.
