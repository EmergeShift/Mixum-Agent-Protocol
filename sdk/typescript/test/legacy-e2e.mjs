// Legacy-dialect smoke: the SDK's `legacy: "mixum-v1"` option against a real
// pre-MAP 0.1.0 node (subprotocol mixum.v1 + handshake op hello).
//
// The 0.1.0 wire differs in three places only (see HANDOFF §3):
//   - subprotocol mixum.v1 (vs map.v1)
//   - handshake op hello (vs session.hello)
//   - task.event data strings (vs objects) — this script reads both shapes.
//
// Usage:
//   NODE_TLS_REJECT_UNAUTHORIZED=0 MAP_URL=wss://host:7601/ws \
//     MAP_TOKEN=mixum1.… node test/legacy-e2e.mjs
import assert from "node:assert/strict";
import { MapClient } from "../dist/index.js";

const url = process.env.MAP_URL;
const token = process.env.MAP_TOKEN;
if (!url || !token) {
  console.error("MAP_URL and MAP_TOKEN are required");
  process.exit(2);
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const stateOf = (event) => (typeof event.data === "string" ? event.data : event.data?.state);
const lineOf = (event) => (typeof event.data === "string" ? event.data : event.data?.line);

async function main() {
  const client = await MapClient.connect({
    url,
    token,
    legacy: "mixum-v1",
    requestTimeoutMs: 20_000,
  });
  assert.equal(client.sessionId.length > 0, true);
  // 0.1.0 nodes omit caps/profiles in hello.ok; the SDK defaults them.
  assert.ok(Array.isArray(client.caps));
  assert.ok(Array.isArray(client.profiles));
  console.log("PASS legacy handshake (mixum.v1 + hello, thin hello.ok)");

  const info = await client.sys.info();
  assert.equal(info.os, "Arch Linux");
  // 0.1.0 镜像的实现串还是 mixumd/;新节点为 mixum-agentd/。
  assert.match(info.server, /^mixum(?:d|-agentd)\//);
  console.log("PASS legacy sys.info", info.server, info.snapshotBackend);

  const events = [];
  client.task.onEvent((event) => events.push(event));
  await client.task.events();
  const task = await client.task.submit({ instruction: "printf legacy-ok" });
  const deadline = Date.now() + 15_000;
  while (Date.now() < deadline) {
    if (events.some((event) => event.taskId === task.id && event.type === "state" && stateOf(event) === "completed")) {
      break;
    }
    await sleep(20);
  }
  assert.ok(
    events.some((event) => event.taskId === task.id && event.type === "state" && stateOf(event) === "completed"),
    `legacy state string not observed: ${JSON.stringify(events)}`,
  );
  assert.ok(
    events.some((event) => event.taskId === task.id && event.type === "log" && lineOf(event) === "legacy-ok"),
    "legacy log string not observed",
  );
  const fetched = await client.task.get(task.id);
  assert.equal(fetched.state, "completed");
  console.log("PASS legacy task lifecycle (string-shaped events)");

  const tree = await client.snap.tree();
  assert.equal(typeof tree.head, "string");
  assert.ok(Array.isArray(tree.nodes));
  console.log("PASS legacy snap.tree", tree.nodes.length, "nodes");

  const display = await client.display.list();
  assert.ok(Array.isArray(display.windows));
  console.log("PASS legacy display.list", display.windows.length, "windows");

  client.close();
  console.log("\nLEGACY 0.1.0 E2E PASSED");
}

main().catch((error) => {
  console.error("LEGACY E2E FAILED:", error);
  process.exit(1);
});
