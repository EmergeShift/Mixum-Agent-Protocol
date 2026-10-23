---
title: VM & Host
description: The Host role — VM lifecycle on macOS/Windows/Linux, images, and bootstrap handoff to Nodes.
---

Stability: **Stable at v1.0** (profile **MAP-Host**).

macOS and Windows cannot run Linux containers natively. The **Host** role closes the gap: a desktop-side daemon (reference: MixumART) that manages local **VMs** — each VM runs a MAP Node, typically a mixumOS image. Agents' work stays isolated inside the VM; the Host is a control-plane convenience, never in the data path.

## Responsibilities

- Create, start, stop, delete VMs; manage VM images.
- Hand clients a **bootstrap**: Node URL, certificate fingerprint, short-TTL token (the Host vouches for the client — the client never stores long-lived Node credentials).
- Forward exactly one port (the Node's WSS) per VM; the Node is otherwise unreachable from the host network by default.

## Vm object

```json
{
  "id": "01JBVD2K…",
  "name": "task-vm-01",
  "state": "running",
  "profile": { "vcpu": 4, "memMiB": 8192, "diskGiB": 64 },
  "image": { "ref": "mixumos/2026.10", "sha256": "<rootfs digest>" },
  "node": { "url": "wss://127.0.0.1:51042/ws", "fingerprint": "sha256/<…>" },
  "createdAt": 1696400000123
}
```

`node` is populated once the VM's Node is reachable; before that it is `null`.

## State machine

```
creating ──▶ stopped ──▶ starting ──▶ running ──▶ stopping ──▶ stopped
                │                        │
                └────────────────────────┴────▶ failed
```

`suspended` (VM-level checkpoint → [Mobility](/map/spec/mobility/) hybrid snapshot of the whole VM) is reserved for v1.1.

## Ops

| Op | Request | Reply |
|---|---|---|
| `vm.create` | `{ name?, profile: {vcpu, memMiB, diskGiB}, image: {uri?\|ref, sha256}, autostart? }` | Vm (`state: "creating"`) |
| `vm.list` | — | `{ vms: [Vm] }` |
| `vm.get` | `{ id }` | Vm |
| `vm.start` | `{ id }` | Vm |
| `vm.stop` | `{ id, force? }` | Vm (`force` skips graceful shutdown) |
| `vm.delete` | `{ id }` | `{ deleted }` |
| `vm.events` | — | `{ subscribed }` — pushes `vm.event { vmId, type: "state"\|"node-ready", … }` |
| `vm.images` | — | `{ images: [{ ref, sha256, label?, sizeBytes }] }` |
| `vm.image.import` | `{ uri, sha256, label? }` | Image entry |
| `vm.attach` | `{ id, ttlSeconds? }` | `{ node: { url, fingerprint, token, expiresAt } }` |

## Bootstrap handoff

`vm.attach` is the standard join flow:

1. Client (GUI/CLI) calls `vm.attach { id }` on the **Host** (which it reaches with its `vm.manage` token).
2. Host returns the Node's `url` (a loopback-forwarded `wss://`), the Node certificate's **SHA-256 fingerprint** (client pins it, TOFU with the Host's vouching replacing the manual confirm), and a **bootstrap token** — default TTL 10 minutes, capabilities `task.run`, `task.observe`, `snap.manage`, `display.view`, `display.input`, `sub` bound to the requesting client.
3. The client opens a MAP session with the Node; the Node verifies the token against the key the Host provisioned into the VM at image setup.

This means: no long-lived secrets on clients, no shared passwords, and revocation is as easy as stopping the VM.

## Images

- An image is a MAP Node root filesystem (e.g. a mixumOS disk image / OCI rootfs). Identified by `ref` (mutable label) + `sha256` (immutable content); the SHA is verified before first boot.
- Image updates are explicit (`vm.image.import` + recreate); running VMs are never mutated underneath.
