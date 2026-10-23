---
title: Quickstart
description: Get a MAP client talking to a Node in ten minutes.
---

Two roles, ten minutes: run a **Node** (the mixumOS reference runtime), then drive it with a **client** (the TypeScript SDK).

## 1. Run a Node

The fastest Node is a mixumOS container (Linux, or a VM on macOS/Windows via a MAP Host):

```bash
# Build the rootfs in the mixumOS repo (`make dist ARCH=aarch64`), then import it.
# The 0.1.5 release ships an OCI/rootfs tarball; a GHCR image is not published yet.
docker import --platform linux/arm64 dist/out/rootfs-aarch64.tar mixumos:0.1.5-dev
docker run -d --name map-node -p 7601:7601 mixumos:0.1.5-dev /entrypoint.sh
```

First start generates a Node identity and a **bootstrap admin token** — it is printed once (or mounted from a secret). Keep it for step 3.

> macOS/Windows without Docker: MixumART (the MAP Host) creates and manages a VM for you; `vm.attach` returns the same URL + bootstrap token shape as below.

## 2. Install the client SDK

The npm package is not published before v1.0. Build it from this repository:

```bash
git clone https://github.com/EmergeShift/Mixum-Agent-Protocol
cd Mixum-Agent-Protocol/sdk/typescript
npm ci && npm run build && npm link
# in your own project:
npm link @mixum/map
```

## 3. Hello, Node

```ts
import { MapClient } from "@mixum/map";

const client = await MapClient.connect({
  url: "wss://127.0.0.1:7601/ws",
  token: process.env.MAP_TOKEN!,          // map1.… bootstrap token
  // pinTLS: "sha256/…"                   // TOFU: pass the fingerprint after first confirm
});

const info = await client.sys.info();
console.log("connected to", info.server, info.profiles);

// submit a task and watch it run
const task = await client.task.submit({
  instruction: "Open a browser and search MAP on bilibili",
  agent: "external/claude-code",
});
client.task.onEvent((e) => {
  if (e.taskId === task.id) console.log(`[${e.type}]`, e.data);
});

// time travel: every task starts on a snapshot
const tree = await client.snap.tree();
console.log("head:", tree.head, "nodes:", tree.nodes.length);
```

## 4. What you get for free

- **Pre-task snapshot**: the task's `preSnapNode` exists before the agent runs; on failure, `snapshotPolicy.onFailure: "rollback"` puts the workspace back.
- **Live windows**: if the agent opens a GUI app, `task.event { type: "window" }` tells you; `display.attach { window }` starts the stream (JPEG always; H.264 when both sides support it).
- **Mobility**: `snap.push { node, remote: "wss://cloud…/ws" }` moves the whole tree — re-running it costs zero bytes.

## Next steps

- [What is MAP?](/map/spec/overview/) — the 5-minute mental model
- [Tasks](/map/spec/tasks/) — lifecycle, events, the human-input channel
- [Snapshots & Time Travel](/map/spec/snapshots/) — tree semantics
- [SDKs](/map/sdk/) — TypeScript & Rust skeletons
