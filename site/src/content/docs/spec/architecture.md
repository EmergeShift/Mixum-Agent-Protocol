---
title: Architecture & Profiles
description: Roles, topology, conformance profiles and the capability model.
---

## Roles

MAP separates four roles. One process may play several (a Host is usually also a Client of its Nodes; a Node is a Peer of other Nodes).

```
        ┌──────────────┐   ┌──────────────┐   ┌──────────────┐
        │ Client (GUI, │   │ Client (CLI, │   │ Client (PWA) │
        │  MixumART)   │   │  SDK, agent) │   │              │
        └──────┬───────┘   └──────┬───────┘   └──────┬───────┘
               │   MAP (WSS)      │                  │
        ┌──────▼──────────────────▼──────────────────▼───────┐
        │                Host (MAP-Host role)                │
        │      vm.*: create/start/stop VMs, bootstrap        │
        └──────┬──────────────────────────┬──────────────────┘
               │ MAP (WSS, forwarded)     │
        ┌──────▼───────┐           ┌──────▼───────┐
        │  Node (VM)   │   ⇄ CAS   │  Node (cloud │
        │  mixumOS     │     sync  │  pod, VM)    │
        │  task/snap/  │           │  same MAP    │
        │  display/mem │           │  surface     │
        └──────────────┘           └──────────────┘
```

| Role | Runs on | Serves | Must implement |
|---|---|---|---|
| **Node** | Inside a VM or container (Linux today) | `session`, `sys`, `task`, `snap`, `agent` (bridge), optional `display`, `mem` | MAP-Core |
| **Host** | Desktop: macOS / Windows / Linux | `vm.*` to its clients; proxies or forwards to its Nodes | MAP-Host |
| **Client** | Anywhere | Consumes; initiates connections | MAP-Core client side |
| **Peer** | Same as Node | `snap` push/pull counterpart | MAP-Mobility |
| **Gateway** | Cloud | Routing, multi-tenant auth | Reserved (not in v1) |

## Topology rules

- Clients connect to a Node **directly** over `wss://` (TLS 1.3, TOFU pinning). The Host's `vm.attach` is the standard way to discover a Node's URL, certificate fingerprint and a bootstrap token.
- Node-to-Node snapshot sync is peer-to-peer: the pushing Node is a MAP **client** of the receiving Node, using a `snap.sync` capability token.
- The Host never sits in the data path of window frames or chunks; it is a control-plane convenience. Direct client↔Node paths are the norm.

## Conformance profiles

An implementation declares the profiles it implements; `hello.ok` echoes the set. Clients must degrade gracefully when an optional profile is missing.

| Profile | Contents | Status in v1 |
|---|---|---|
| **MAP-Core** | Envelope & framing, session & sys, tasks, snapshot tree basics (create/restore/fork/delete/gc), MAP-Bridge presence | Stable |
| **MAP-Display** | `display.*`, window streaming, input, takeover | Stable |
| **MAP-Mobility** | `snap.push`/pull, CAS sync protocol, manifest interchange, restore guards | Stable |
| **MAP-Memory** | `mem.*`, memory records, provider import/export | Stable at v1.0 |
| **MAP-Host** | `vm.*`, images, bootstrap handoff | Stable at v1.0 |
| MAP-Cluster | Multi-node scheduling via a Gateway | Reserved |

Conformance is scoped: an implementation claims exactly what it passed vectors for, e.g. *"MAP v1 Core + Display conformant"*. See [Versioning & Governance](/spec/versioning-governance/).

## The capability model

Authorization is fully capability-based. A connection holds exactly the capabilities its token grants — nothing else. Servers must treat the absence of a token capability as `forbidden`, never as a crash or silent ignore.

| Capability string | Grants |
|---|---|
| `task.run` | Submit and cancel tasks |
| `task.observe` | Subscribe to task events, list/get tasks, snapshots and system info |
| `snap.manage` | Create, restore, fork, delete snapshots; run GC |
| `snap.sync` | Receive pushed snapshots (act as sync target) |
| `display.view` | List windows, attach to streams |
| `display.input` | Inject input, takeover |
| `mem.read` | Search, list, get, export memory records |
| `mem.write` | Put, delete, import memory records |
| `vm.manage` | Create, start/stop, delete VMs and images (Host role) |
| `admin` | Everything; used for bootstrap and derivation of scoped tokens |

Token claims may **bind** a capability further: `taskId` (operate only that task; also forbids submitting new tasks), `windows` (only those window ids), `mem.providers` (only those memory providers). See [Transport & Security](/spec/transport-security/).

Typical grants:

| Caller | Token capabilities |
|---|---|
| Desktop GUI / phone PWA | `task.run`, `task.observe`, `display.view`, `display.input` |
| Snapshot sync peer | `snap.sync` (short TTL, per-remote) |
| Headless automation client | `task.run`, `task.observe`, `snap.manage` |
| Agent process (via runner) | None — the agent talks to the runner over MAP-Bridge, never directly to the wire |
