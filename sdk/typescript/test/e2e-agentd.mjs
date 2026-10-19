// Full-loop smoke test: the published-shape @mixum/map client against a real
// mixum-agentd (plain WS, started by scripts/map-sdk-e2e.sh).
//
// Covers the handoff §2 acceptance surface with ZERO client adaptation:
// session.hello, subprotocols, task events (MAP data shapes), task.reply,
// session.resume, mem.*, snap.*, display.*, token prefix and error codes.
import assert from "node:assert/strict";
import { chmodSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { MapClient } from "../dist/index.js";

const url = process.env.MAP_URL;
const token = process.env.MAP_TOKEN;
if (!url || !token) {
  console.error("MAP_URL and MAP_TOKEN are required (scripts/map-sdk-e2e.sh sets them)");
  process.exit(2);
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
function watch(client) {
  const events = [];
  client.task.onEvent((event) => events.push(event));
  return {
    events,
    async wait(predicate, label, timeoutMs = 15_000) {
      const deadline = Date.now() + timeoutMs;
      while (Date.now() < deadline) {
        const index = events.findIndex(predicate);
        if (index >= 0) return events[index];
        await sleep(20);
      }
      throw new Error(`timeout waiting for ${label}; events=${JSON.stringify(events)}`);
    },
  };
}

async function main() {
  // 1. session.hello / profile / version negotiation.
  const client = await MapClient.connect({ url, token });
  assert.equal(client.sessionId.length, 26, "sessionId must be a ULID");
  assert.deepEqual(client.hello.versions, [1], "envelope.v stays 1");
  assert.ok(client.profiles.includes("core"), `profiles: ${client.profiles}`);
  assert.ok(client.profiles.includes("memory"), `profiles: ${client.profiles}`);
  assert.ok(client.hello.server.startsWith("mixum-agentd/"), client.hello.server);
  console.log("PASS session.hello", client.hello.server, client.profiles.join(","));

  const info = await client.sys.info();
  assert.ok(info.profiles.includes("memory"));
  assert.ok(info.memoryProviders.includes("builtin"));
  assert.equal(info.compositor, false);
  console.log("PASS sys.info");

  // 2. Task lifecycle with MAP event data shapes.
  const watcher = watch(client);
  assert.deepEqual(await client.task.events(), { subscribed: true });
  const task = await client.task.submit({ instruction: "printf e2e-ok" });
  assert.equal(task.id.length, 26, "task id must be a ULID");
  await watcher.wait(
    (event) => event.taskId === task.id && event.type === "state" && event.data.state === "completed",
    "task completion",
  );
  assert.ok(
    watcher.events.some((event) => event.taskId === task.id && event.type === "log" && event.data.line === "e2e-ok"),
    "log data.line",
  );
  assert.ok(
    watcher.events.some((event) => event.taskId === task.id && event.type === "state" && event.data.state === "queued"),
    "state data.state",
  );
  const fetched = await client.task.get(task.id);
  assert.equal(fetched.state, "completed");
  const listed = await client.task.list();
  assert.ok(listed.tasks.some((candidate) => candidate.id === task.id));
  console.log("PASS task lifecycle + MAP event shapes");

  // 3. Human-input channel: bridge input.request -> task.event/input -> task.reply.
  const agentDir = mkdtempSync(join(tmpdir(), "map-agent-"));
  const agentScript = join(agentDir, "agent.sh");
  writeFileSync(
    agentScript,
    `#!/bin/sh
read hello
printf '%s\\n' '{"type":"input.request","id":"q1","prompt":"Proceed?","choices":["yes","no"]}'
read response
case "$response" in
  *'"value":"yes"'*) printf '%s\\n' '{"type":"log","line":"input-delivered"}' ;;
  *) printf '%s\\n' '{"type":"log","line":"input-wrong"}' ;;
esac
printf '%s\\n' '{"type":"done"}'
`,
  );
  chmodSync(agentScript, 0o755);
  const inputTask = await client.task.submit({
    instruction: "ask the human",
    agent: "external",
    params: { agentCmd: agentScript },
  });
  const inputEvent = await watcher.wait(
    (event) => event.taskId === inputTask.id && event.type === "input",
    "input event",
  );
  assert.equal(inputEvent.data.replyId, "q1");
  assert.deepEqual(inputEvent.data.choices, ["yes", "no"]);
  assert.deepEqual(await client.task.reply(inputTask.id, "q1", "yes"), { delivered: true });
  await watcher.wait(
    (event) => event.taskId === inputTask.id && event.type === "state" && event.data.state === "completed",
    "input task completion",
  );
  assert.ok(watcher.events.some((event) => event.type === "log" && event.data.line === "input-delivered"));
  rmSync(agentDir, { recursive: true, force: true });
  console.log("PASS task.reply <-> input.request");

  // 4. Snapshot tree ops (memory backend).
  const treeBefore = await client.snap.tree();
  const node = await client.snap.create({ kind: "disk", label: "sdk-e2e" });
  assert.equal(node.id.length, 26);
  const treeAfter = await client.snap.tree();
  assert.equal(treeAfter.head, node.id);
  assert.ok(treeAfter.nodes.length >= treeBefore.nodes.length);
  console.log("PASS snap.create / snap.tree");

  // 5. Display enumeration (no compositor => empty list, typed reply).
  assert.deepEqual(await client.display.list(), { windows: [] });
  console.log("PASS display.list");

  // 6. MAP-Memory.
  const providers = await client.mem.providers();
  assert.equal(providers.providers[0].id, "builtin");
  const record = await client.mem.put({
    provider: "builtin",
    kind: "preference",
    content: { text: "Prefers concise answers", structured: { locale: "zh-CN" } },
    scope: { level: "global" },
    source: { platform: "chatgpt", origin: "sdk-e2e" },
    tags: ["style"],
    confidence: 0.9,
    opaque: "AAEC",
  });
  assert.equal(record.id.length, 26);
  assert.equal(record.opaque, "AAEC");
  const got = await client.mem.get(record.id);
  assert.equal(got.content.text, "Prefers concise answers");
  const hits = await client.mem.search({ query: "concise" });
  assert.equal(hits.hits[0].record.id, record.id);
  assert.ok(hits.hits[0].score > 0);
  const exported = await client.mem.export({ format: "map-json" });
  assert.ok(exported.records.some((candidate) => candidate.id === record.id));
  assert.deepEqual(await client.mem.delete(record.id), { deleted: true });
  await assert.rejects(client.mem.get(record.id), (error) => error.code === "not_found");
  console.log("PASS mem.* (providers/put/get/search/list/export/delete)");

  // 7. Error mapping for unknown ops.
  await assert.rejects(client.request("does.not_exist"), (error) => error.code === "unsupported");
  console.log("PASS unsupported op mapping");

  // 8. session.resume: no hello, no re-subscribe; replays buffered pushes.
  const sessionId = client.sessionId;
  client.close();
  await sleep(100);
  const resumed = await MapClient.resume({ url, sessionId, lastSeq: 0 });
  assert.equal(resumed.sessionId, sessionId);
  assert.deepEqual(resumed.caps, client.caps);
  const replayed = [];
  resumed.task.onEvent((event) => replayed.push(event));
  // 重放帧可能在 resume 返回后才到达(经 handler 直投)或已早到(缓冲补投),
  // 两种路径都要在有限时间内出现。
  for (let waited = 0; waited < 2000 && replayed.length === 0; waited += 20) await sleep(20);
  assert.ok(replayed.length > 0, "resume must replay buffered task events");
  assert.ok(replayed.every((event) => typeof event.taskId === "string"));
  const resumedList = await resumed.task.list();
  assert.ok(resumedList.tasks.length > 0, "claims restored after resume");
  console.log(`PASS session.resume (replayed ${replayed.length} events, claims restored)`);

  // 9. Legacy dialect option against a MAP node (which also accepts the alias).
  const legacy = await MapClient.connect({ url, token, legacy: "mixum-v1" });
  assert.equal(legacy.sessionId.length, 26);
  const legacyInfo = await legacy.sys.info();
  assert.equal(legacyInfo.server, info.server);
  legacy.close();
  console.log("PASS legacy: mixum-v1 (mixum.v1 + hello alias)");

  resumed.close();
  console.log("\nALL MAP SDK E2E CHECKS PASSED");
}

main().catch((error) => {
  console.error("E2E FAILED:", error);
  process.exit(1);
});
