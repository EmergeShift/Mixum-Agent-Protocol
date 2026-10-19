// SDK wire-behaviour tests: no network, a mock WebSocket exercises the exact
// frames the client emits (subprotocol + op + payload) and the reply handling.
import assert from "node:assert/strict";
import test from "node:test";
import { MapClient } from "../dist/index.js";

const SESSION_ID = "01JBV8V8V8V8V8V8V8V8V8V8V8";

class MockSocket extends EventTarget {
  static OPEN = 1;
  static instances = [];
  static handler = null;

  constructor(url, protocols) {
    super();
    this.url = url;
    this.protocols = protocols;
    this.readyState = MockSocket.OPEN;
    this.OPEN = MockSocket.OPEN;
    this.binaryType = "blob";
    this.sent = [];
    MockSocket.instances.push(this);
  }

  send(raw) {
    const envelope = JSON.parse(raw);
    this.sent.push(envelope);
    MockSocket.handler?.(this, envelope);
  }

  close() {
    this.readyState = 3;
    this.dispatchEvent(new Event("close"));
  }

  emit(envelope) {
    this.dispatchEvent(new MessageEvent("message", { data: JSON.stringify(envelope) }));
  }
}

function reset(handler) {
  MockSocket.instances = [];
  MockSocket.handler = handler;
}

function replyTo(envelope, op, payload) {
  return {
    v: 1,
    id: `01REPLY${Math.random().toString(36).slice(2, 12).toUpperCase()}`,
    op,
    replyTo: envelope.id,
    ts: Date.now(),
    payload,
  };
}

const HELLO_PAYLOAD = {
  sessionId: SESSION_ID,
  server: "mixum-agentd/0.1.0",
  arch: "aarch64",
  kernel: "6.18.35",
  caps: ["task.observe", "mem.read"],
  profiles: ["core", "memory"],
  versions: [1],
  heartbeat: 30,
};

test("connect sends map.v1 + session.hello and parses hello.ok", async () => {
  reset((socket, envelope) => {
    if (envelope.op === "session.hello") {
      setTimeout(() => socket.emit(replyTo(envelope, "session.hello.ok", HELLO_PAYLOAD)), 0);
    }
  });
  const client = await MapClient.connect({
    url: "ws://node.test/ws",
    token: "map1.token",
    WebSocketImpl: MockSocket,
  });
  const socket = MockSocket.instances[0];
  assert.equal(socket.protocols, "map.v1");
  assert.equal(socket.sent[0].op, "session.hello");
  assert.deepEqual(socket.sent[0].payload, { token: "map1.token", client: "@mixum/map/0.1.0" });
  assert.equal(client.sessionId, SESSION_ID);
  assert.deepEqual(client.caps, ["task.observe", "mem.read"]);
  assert.deepEqual(client.profiles, ["core", "memory"]);
  assert.equal(client.lastSeq, 0);
  client.close();
});

test("legacy: mixum-v1 speaks mixum.v1 + hello and tolerates thin hello.ok", async () => {
  reset((socket, envelope) => {
    if (envelope.op === "hello") {
      // 0.1.0 nodes omit caps/profiles entirely.
      setTimeout(
        () =>
          socket.emit(
            replyTo(envelope, "hello.ok", {
              sessionId: SESSION_ID,
              server: "mixumd/0.1.0",
              arch: "aarch64",
              kernel: "6.12.8",
              versions: [1],
              heartbeat: 30,
            }),
          ),
        0,
      );
    }
  });
  const client = await MapClient.connect({
    url: "ws://node.test/ws",
    token: "mixum1.old",
    WebSocketImpl: MockSocket,
    legacy: "mixum-v1",
  });
  const socket = MockSocket.instances[0];
  assert.equal(socket.protocols, "mixum.v1");
  assert.equal(socket.sent[0].op, "hello");
  assert.deepEqual(client.caps, []);
  assert.deepEqual(client.profiles, []);
  client.close();
});

test("request/reply correlation and push seq tracking", async () => {
  reset((socket, envelope) => {
    if (envelope.op === "session.hello") {
      setTimeout(() => socket.emit(replyTo(envelope, "session.hello.ok", HELLO_PAYLOAD)), 0);
    } else {
      setTimeout(
        () => socket.emit(replyTo(envelope, `${envelope.op}.ok`, { server: "mixum-agentd/0.1.0" })),
        0,
      );
    }
  });
  const client = await MapClient.connect({
    url: "ws://node.test/ws",
    token: "map1.token",
    WebSocketImpl: MockSocket,
  });
  const socket = MockSocket.instances[0];
  const info = await client.sys.info();
  assert.equal(info.server, "mixum-agentd/0.1.0");
  assert.equal(socket.sent.at(-1).op, "sys.info");

  const seen = [];
  client.task.onEvent((event) => seen.push(event));
  socket.emit({
    v: 1,
    id: "01PUSH00000000000000000000",
    op: "task.event",
    seq: 7,
    ts: Date.now(),
    payload: { taskId: "01TASK00000000000000000000", type: "state", data: { state: "queued" }, ts: 1 },
  });
  assert.equal(client.lastSeq, 7);
  assert.equal(seen.length, 1);
  assert.deepEqual(seen[0].data, { state: "queued" });
  client.close();
});

test("resume restores session state and delivers buffered replay to onPush", async () => {
  reset((socket, envelope) => {
    if (envelope.op === "session.resume") {
      setTimeout(() => {
        socket.emit(
          replyTo(envelope, "session.resume.ok", {
            resumed: true,
            replayed: 2,
            sessionId: SESSION_ID,
            server: "mixum-agentd/0.1.0",
            arch: "aarch64",
            kernel: "6.18.35",
            caps: ["task.observe"],
            profiles: ["core", "memory"],
            versions: [1],
            heartbeat: 30,
          }),
        );
        // 同一任务里紧跟着重放:此时 MapClient 尚未构造,SDK 必须缓存。
        for (const seq of [3, 4]) {
          socket.emit({
            v: 1,
            id: `01REPLAY0000000000000000${seq}`,
            op: "task.event",
            seq,
            ts: Date.now(),
            payload: {
              taskId: "01TASK00000000000000000000",
              type: "state",
              data: { state: seq === 3 ? "queued" : "running" },
              ts: seq,
            },
          });
        }
      }, 0);
    }
  });
  const client = await MapClient.resume({
    url: "ws://node.test/ws",
    sessionId: SESSION_ID,
    lastSeq: 2,
    WebSocketImpl: MockSocket,
  });
  const socket = MockSocket.instances[0];
  assert.equal(socket.protocols, "map.v1");
  assert.equal(socket.sent[0].op, "session.resume");
  assert.deepEqual(socket.sent[0].payload, { sessionId: SESSION_ID, lastSeq: 2 });
  assert.equal(client.sessionId, SESSION_ID);
  assert.deepEqual(client.caps, ["task.observe"]);
  assert.equal(client.lastSeq, 4);

  const seen = [];
  client.task.onEvent((event) => seen.push(event));
  assert.deepEqual(
    seen.map((event) => event.data.state),
    ["queued", "running"],
  );
  client.close();
});

test("resume surfaces session.resume.err as MapError", async () => {
  reset((socket, envelope) => {
    if (envelope.op === "session.resume") {
      setTimeout(
        () =>
          socket.emit(
            replyTo(envelope, "session.resume.err", {
              code: "not_found",
              message: "会话不存在或已过期",
            }),
          ),
        0,
      );
    }
  });
  await assert.rejects(
    MapClient.resume({
      url: "ws://node.test/ws",
      sessionId: SESSION_ID,
      WebSocketImpl: MockSocket,
    }),
    (error) => error.code === "not_found",
  );
});
