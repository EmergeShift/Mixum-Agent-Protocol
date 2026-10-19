import { MapError } from "./errors.js";
import type { ErrorCode } from "./types.js";
import { ulid, ulidFromBytes } from "./ulid.js";
import {
  LEGACY_SUBPROTOCOL,
  PROTOCOL_VERSION,
  SUBPROTOCOL,
  type Envelope,
  type HelloOk,
  type MemoryProvider,
  type MemoryRecord,
  type SnapNode,
  type SnapshotKind,
  type SysInfo,
  type Task,
  type TaskEvent,
  type TaskSubmitRequest,
  type Ulid,
  type VideoCodec,
  type VideoFrameHeader,
  type Vm,
  type VmAttach,
  type Window,
} from "./types.js";

export interface MapConnectOptions {
  url: string;
  token: string;
  /** TOFU pin (SHA-256 fingerprint of the leaf certificate). Enforced by the
   *  runtime: pass a WebSocketImpl wrapper that rejects non-matching certs. */
  pinTLS?: string;
  /** Per-request timeout (ms). Default 30 000. */
  requestTimeoutMs?: number;
  /** WebSocket constructor override (tests, TLS-pinning wrappers). */
  WebSocketImpl?: typeof WebSocket;
  /**
   * Speak the pre-MAP wire dialect of 0.1.0 nodes: subprotocol `mixum.v1` and
   * handshake op `hello` (alias kept by MAP v1). New nodes need no option.
   */
  legacy?: "mixum-v1";
}

/** Reconnect a previously negotiated session (session.md §Resume). */
export interface MapResumeOptions {
  url: string;
  /** The `sessionId` from the original `session.hello.ok`. */
  sessionId: Ulid;
  /** Highest push `seq` the client has processed; the node replays newer ones. */
  lastSeq?: number;
  /** TOFU pin, see MapConnectOptions. */
  pinTLS?: string;
  /** Per-request timeout (ms). Default 30 000. */
  requestTimeoutMs?: number;
  /** WebSocket constructor override (tests, TLS-pinning wrappers). */
  WebSocketImpl?: typeof WebSocket;
}

type ReplyResolver = {
  resolve: (payload: Record<string, unknown>) => void;
  reject: (err: MapError) => void;
  timer: ReturnType<typeof setTimeout>;
};

function errCode(value: unknown): ErrorCode {
  return typeof value === "string" && MapError.isCode(value) ? value : "internal";
}

type PushHandler = (envelope: Envelope) => void;

/** Binary channel tags (envelope.md). */
const CHANNEL_VIDEO = 0x01;
const CHANNEL_CHUNK = 0x03;
const CHANNEL_STDIO = 0x04;

/**
 * MAP client (MAP-Core). Connects over WSS, performs session.hello,
 * correlates requests/replies by ULID, dispatches push envelopes and
 * binary frames, and exposes typed domain helpers.
 */
export class MapClient {
  #ws: WebSocket;
  #pending = new Map<Ulid, ReplyResolver>();
  #pushHandlers = new Set<PushHandler>();
  #videoCodecs = new Map<number, VideoCodec>();
  #requestTimeoutMs: number;
  #lastSeq = 0;
  /** Push envelopes buffered until a push handler registers (resume replay). */
  #earlyPush: Envelope[] = [];
  /** Replayed envelopes still expected from session.resume. */
  #replayRemaining = 0;
  readonly hello: HelloOk;

  private constructor(
    ws: WebSocket,
    hello: HelloOk,
    requestTimeoutMs: number,
    buffered: unknown[] = [],
    replayCount = 0,
  ) {
    this.#ws = ws;
    this.hello = hello;
    this.#requestTimeoutMs = requestTimeoutMs;
    this.#replayRemaining = replayCount;
    ws.binaryType = "arraybuffer";
    ws.addEventListener("message", (ev) => this.#onMessage(ev.data));
    ws.addEventListener("close", () => this.#onClose());
    // 握手/恢复期间已到达的推送(含 resume 重放)先入缓冲,等 onPush 注册后
    // 按顺序补投递,避免「resume 返回前重放已丢」。
    for (const data of buffered) {
      const env = typeof data === "string" ? parseEnvelope(data) : undefined;
      if (env && !env.replyTo) {
        if (typeof env.seq === "number" && env.seq > this.#lastSeq) this.#lastSeq = env.seq;
        this.#ingestPush(env);
      } else {
        this.#onMessage(data);
      }
    }
  }

  /**
   * Deliver a push envelope: with registered handlers immediately, otherwise
   * retain it while resume replay is still outstanding. The cap keeps a
   * client that never subscribes from growing without bound.
   */
  #ingestPush(envelope: Envelope): void {
    if (this.#pushHandlers.size > 0) {
      for (const handler of [...this.#pushHandlers]) handler(envelope);
      if (this.#replayRemaining > 0) this.#replayRemaining -= 1;
      return;
    }
    if (this.#replayRemaining > 0) {
      this.#replayRemaining -= 1;
      this.#earlyPush.push(envelope);
      if (this.#earlyPush.length > 1024) this.#earlyPush.shift();
    }
  }

  /** Connect, handshake, and negotiate. Rejects on hello.err or timeout. */
  static async connect(opts: MapConnectOptions): Promise<MapClient> {
    const timeoutMs = opts.requestTimeoutMs ?? 30_000;
    const WS = opts.WebSocketImpl ?? WebSocket;
    const legacy = opts.legacy === "mixum-v1";
    const ws = await openSocket(WS, opts.url, legacy ? LEGACY_SUBPROTOCOL : SUBPROTOCOL);

    const env: Envelope = {
      v: PROTOCOL_VERSION,
      id: ulid(),
      // 旧节点只认 hello;MAP 节点按别名继续接受。
      op: legacy ? "hello" : "session.hello",
      ts: Date.now(),
      payload: { token: opts.token, client: "@mixum/map/0.1.0" },
    };
    const { reply, buffered, detach } = await awaitReply(ws, env, timeoutMs, "handshake timed out");
    if (reply.op.endsWith(".err")) {
      const p = envelopePayload(reply) as { code?: string; message?: string };
      detach();
      rejectSocket(ws);
      throw new MapError(errCode(p.code), p.message ?? "handshake failed");
    }
    const hello = envelopePayload(reply) as unknown as HelloOk;
    const client = new MapClient(ws, hello, timeoutMs, buffered);
    detach();
    return client;
  }

  /**
   * Reconnect an existing session without a new handshake. The node replays
   * every buffered push newer than `lastSeq` (session.md §Resume); those
   * envelopes are dispatched to push handlers registered right after resume.
   */
  static async resume(opts: MapResumeOptions): Promise<MapClient> {
    const timeoutMs = opts.requestTimeoutMs ?? 30_000;
    const WS = opts.WebSocketImpl ?? WebSocket;
    const ws = await openSocket(WS, opts.url, SUBPROTOCOL);
    const env: Envelope = {
      v: PROTOCOL_VERSION,
      id: ulid(),
      op: "session.resume",
      ts: Date.now(),
      payload: { sessionId: opts.sessionId, lastSeq: opts.lastSeq ?? 0 },
    };
    const { reply, buffered, detach } = await awaitReply(ws, env, timeoutMs, "resume timed out");
    if (reply.op.endsWith(".err")) {
      const p = envelopePayload(reply) as { code?: string; message?: string };
      detach();
      rejectSocket(ws);
      throw new MapError(errCode(p.code), p.message ?? "resume failed");
    }
    const p = envelopePayload(reply) as Partial<HelloOk> & { replayed?: number };
    const hello: HelloOk = {
      sessionId: opts.sessionId,
      server: p.server ?? "unknown",
      arch: p.arch ?? "unknown",
      kernel: p.kernel,
      caps: p.caps ?? [],
      profiles: p.profiles ?? [],
      versions: p.versions ?? [PROTOCOL_VERSION],
      heartbeat: p.heartbeat ?? 30,
    };
    const client = new MapClient(ws, hello, timeoutMs, buffered, p.replayed ?? 0);
    detach();
    return client;
  }

  get sessionId(): Ulid {
    return this.hello.sessionId;
  }

  /** Highest push `seq` seen on this connection (for `MapClient.resume`). */
  get lastSeq(): number {
    return this.#lastSeq;
  }

  get caps(): string[] {
    return [...(this.hello.caps ?? [])];
  }

  get profiles(): string[] {
    return [...(this.hello.profiles ?? [])];
  }

  /** Send a request op and await its .ok payload; .err rejects with MapError. */
  async request<P = Record<string, unknown>>(op: string, payload: Record<string, unknown> = {}): Promise<P> {
    const id = ulid();
    const env: Envelope = { v: PROTOCOL_VERSION, id, op, ts: Date.now(), payload };
    return new Promise<P>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.#pending.delete(id);
        reject(new MapError("timeout", `${op} timed out`, id));
      }, this.#requestTimeoutMs);
      this.#pending.set(id, { resolve: (p) => resolve(p as P), reject, timer });
      this.#send(env);
    });
  }

  /** Subscribe to every push-direction envelope (task.event, snap.event, …). */
  onPush(handler: PushHandler): () => void {
    // 先补投递连接/恢复期间早到的推送(resume 重放),再接后续推送。
    if (this.#earlyPush.length > 0) {
      const early = this.#earlyPush.splice(0);
      for (const envelope of early) handler(envelope);
    }
    this.#pushHandlers.add(handler);
    return () => {
      this.#pushHandlers.delete(handler);
    };
  }

  close(): void {
    this.#ws.close();
  }

  // ---- domains -----------------------------------------------------------

  readonly sys = {
    ping: () => this.request<{ pong: string }>("sys.ping"),
    info: () => this.request<SysInfo>("sys.info"),
  };

  readonly task = {
    submit: (req: TaskSubmitRequest) => this.request<Task>("task.submit", { ...req }),
    list: (req: { state?: string; limit?: number; cursor?: string } = {}) =>
      this.request<{ tasks: Task[]; nextCursor?: string }>("task.list", req),
    get: (id: Ulid) => this.request<Task>("task.get", { id }),
    cancel: (id: Ulid) => this.request<{ id: Ulid }>("task.cancel", { id }),
    events: (taskId?: Ulid) => this.request<{ subscribed: boolean }>("task.events", taskId ? { taskId } : {}),
    reply: (taskId: Ulid, replyId: string, value: string) =>
      this.request<{ delivered: boolean }>("task.reply", { taskId, replyId, value }),
    onEvent: (fn: (e: TaskEvent) => void) =>
      this.onPush((env) => {
        if (env.op === "task.event") fn(envelopePayload(env) as unknown as TaskEvent);
      }),
  };

  readonly snap = {
    create: (req: { kind: SnapshotKind; label?: string; parent?: Ulid; branch?: string; auto?: boolean }) =>
      this.request<SnapNode>("snap.create", { ...req }),
    tree: () => this.request<{ head: Ulid; branch: string; nodes: SnapNode[] }>("snap.tree"),
    restore: (req: { node: Ulid; strategy: "inplace" | "fork"; label?: string; force?: boolean }) =>
      this.request<{ restoredTo: Ulid; newBranch?: string; newNode?: Ulid }>("snap.restore", { ...req }),
    fork: (req: { node: Ulid; branch: string }) => this.request<{ branch: string }>("snap.fork", req),
    delete: (node: Ulid) => this.request<{ deleted: boolean }>("snap.delete", { node }),
    gc: () => this.request<{ removed: number }>("snap.gc"),
    events: () => this.request<{ subscribed: boolean }>("snap.events"),
    push: (req: { node: Ulid; remote: string; token?: string; includeMemory?: boolean }) =>
      this.request<{ node: Ulid; chunksSent: number; bytesSent: number; dedupRatio: number }>("snap.push", { ...req }),
  };

  readonly display = {
    list: () => this.request<{ windows: Window[] }>("display.list"),
    attach: (req: { window: number; fps?: number; codec?: VideoCodec }) => {
      const p = this.request<{ window: number; codec: VideoCodec; width: number; height: number; fps: number }>(
        "display.attach",
        { ...req },
      );
      void p.then((r) => this.#videoCodecs.set(r.window, r.codec)).catch(() => undefined);
      return p;
    },
    detach: (window: number) => this.request<{ window: number }>("display.detach", { window }),
    input: (req: { window: number; type: string } & Record<string, unknown>) => this.request<{ ok: true }>("display.input", { ...req }),
    takeover: (req: { window: number; action: "claim" | "release"; force?: boolean }) =>
      this.request<{ mode: string }>("display.takeover", { ...req }),
    windows: () => this.request<{ subscribed: boolean }>("display.windows"),
    /** Listen to video records: frame header (display.md) + encoded payload. */
    onVideo: (fn: (header: VideoFrameHeader, codec: VideoCodec, payload: Uint8Array) => void): (() => void) => {
      this.#videoHandlers.add(fn);
      return () => {
        this.#videoHandlers.delete(fn);
      };
    },
  };

  readonly mem = {
    providers: () => this.request<{ providers: MemoryProvider[] }>("mem.providers"),
    search: (req: { query: string; kinds?: string[]; providers?: string[]; limit?: number }) =>
      this.request<{ hits: { record: MemoryRecord; score: number }[] }>("mem.search", { ...req }),
    get: (id: Ulid) => this.request<MemoryRecord>("mem.get", { id }),
    put: (record: MemoryRecord) => this.request<MemoryRecord>("mem.put", { record }),
    delete: (id: Ulid) => this.request<{ deleted: boolean }>("mem.delete", { id }),
    list: (req: { provider?: string; kind?: string; cursor?: string; limit?: number } = {}) =>
      this.request<{ records: MemoryRecord[]; nextCursor?: string }>("mem.list", req),
    import: (req: { provider: string; format: "map-json"; records: MemoryRecord[] }) =>
      this.request<{ imported: number; skipped: number }>("mem.import", req),
    export: (req: { provider?: string; format: "map-json" }) => this.request<{ records: MemoryRecord[] }>("mem.export", req),
  };

  readonly vm = {
    list: () => this.request<{ vms: Vm[] }>("vm.list"),
    get: (id: Ulid) => this.request<Vm>("vm.get", { id }),
    create: (req: {
      name?: string;
      profile: { vcpu: number; memMiB: number; diskGiB: number };
      image: { uri?: string; ref?: string; sha256: string };
      autostart?: boolean;
    }) => this.request<Vm>("vm.create", { ...req }),
    start: (id: Ulid) => this.request<Vm>("vm.start", { id }),
    stop: (id: Ulid, force?: boolean) => this.request<Vm>("vm.stop", { id, force }),
    delete: (id: Ulid) => this.request<{ deleted: boolean }>("vm.delete", { id }),
    attach: (id: Ulid, ttlSeconds?: number) => this.request<VmAttach>("vm.attach", ttlSeconds ? { id, ttlSeconds } : { id }),
  };

  // ---- internals ---------------------------------------------------------

  #videoHandlers = new Set<(header: VideoFrameHeader, codec: VideoCodec, payload: Uint8Array) => void>();

  #send(env: Envelope): void {
    this.#ws.send(JSON.stringify(env));
  }

  #onMessage(data: unknown): void {
    if (typeof data === "string") {
      const env = parseEnvelope(data);
      if (!env) return;
      if (typeof env.seq === "number" && env.seq > this.#lastSeq) this.#lastSeq = env.seq;
      if (env.replyTo) {
        const pending = this.#pending.get(env.replyTo);
        if (pending) {
          this.#pending.delete(env.replyTo);
          clearTimeout(pending.timer);
          if (env.op.endsWith(".err")) {
            const p = envelopePayload(env) as { code?: string; message?: string };
            pending.reject(new MapError(errCode(p.code), p.message ?? "error", env.replyTo));
          } else {
            pending.resolve(envelopePayload(env));
          }
        }
        return;
      }
      this.#ingestPush(env);
      return;
    }
    if (data instanceof ArrayBuffer) this.#onBinary(data);
  }

  #onBinary(buf: ArrayBuffer): void {
    const view = new DataView(buf);
    if (buf.byteLength < 1) return;
    switch (view.getUint8(0)) {
      case CHANNEL_VIDEO: {
        if (buf.byteLength < 27) return;
        const header: VideoFrameHeader = {
          windowId: view.getUint32(1, true),
          width: view.getUint32(5, true),
          height: view.getUint32(9, true),
          size: view.getUint32(13, true),
          flags: view.getUint16(17, true),
          cursorX: view.getInt16(19, true),
          cursorY: view.getInt16(21, true),
          cursorW: view.getUint16(23, true),
          cursorH: view.getUint16(25, true),
        };
        const payload = new Uint8Array(buf, 27);
        const codec = this.#videoCodecs.get(header.windowId) ?? "jpeg";
        for (const h of [...this.#videoHandlers]) h(header, codec, payload);
        return;
      }
      case CHANNEL_CHUNK:
        return; // sync peers use dedicated tooling; exposed via lower-level API later
      case CHANNEL_STDIO:
        return; // stdio streaming: mirror of task.event/log; consumers use events today
      default:
        return; // unknown channel: ignore (additive channels are a minor change)
    }
  }

  #onClose(): void {
    const err = new MapError("unavailable", "connection closed");
    for (const pending of this.#pending.values()) {
      clearTimeout(pending.timer);
      pending.reject(err);
    }
    this.#pending.clear();
  }
}

type HandshakeResult = {
  reply: Envelope;
  buffered: unknown[];
  /** Detach the temporary listener after MapClient took over the socket. */
  detach: () => void;
};

async function openSocket(
  WS: typeof WebSocket,
  url: string,
  protocol: string,
): Promise<WebSocket> {
  const ws = new WS(url, protocol);
  await new Promise<void>((resolve, reject) => {
    if (ws.readyState === ws.OPEN) return resolve();
    ws.addEventListener("open", () => resolve(), { once: true });
    ws.addEventListener("error", () => reject(new MapError("unavailable", "connection failed")), { once: true });
  });
  return ws;
}

/**
 * Send one envelope and wait for its reply. Frames that arrive before the
 * reply (push events, resume replay) are buffered so the MapClient can
 * dispatch them once its listeners are attached.
 */
function awaitReply(
  ws: WebSocket,
  env: Envelope,
  timeoutMs: number,
  timeoutMessage: string,
): Promise<HandshakeResult> {
  return new Promise((resolve, reject) => {
    const buffered: unknown[] = [];
    let timer: ReturnType<typeof setTimeout> | undefined;
    let settled = false;
    const detach = (): void => {
      if (timer !== undefined) clearTimeout(timer);
      ws.removeEventListener("message", onMsg);
    };
    const onMsg = (ev: MessageEvent): void => {
      // 答复之后的帧仍继续缓冲,直到 MapClient 构造完成并调用 detach:
      // resume 的 replayed 推送可能紧跟在 reply 之后同一个宏任务里到达。
      buffered.push(ev.data);
      if (settled) return;
      const parsed = parseEnvelope(ev.data);
      if (!parsed || parsed.replyTo !== env.id) return;
      settled = true;
      if (timer !== undefined) clearTimeout(timer);
      resolve({ reply: parsed, buffered, detach });
    };
    timer = setTimeout(() => {
      if (settled) return;
      settled = true;
      detach();
      reject(new MapError("timeout", timeoutMessage));
    }, timeoutMs);
    ws.addEventListener("message", onMsg);
    ws.send(JSON.stringify(env));
  });
}

function rejectSocket(ws: WebSocket): void {
  try {
    ws.close();
  } catch {
    // 关闭失败不影响已经确定的握手错误。
  }
}

function parseEnvelope(text: unknown): Envelope | undefined {
  try {
    const env = JSON.parse(String(text)) as Envelope;
    if (env && typeof env === "object" && env.v === PROTOCOL_VERSION && typeof env.op === "string") return env;
  } catch {
    // non-envelope text on the control plane: ignore (forward compatibility)
  }
  return undefined;
}

function envelopePayload(env: Envelope): Record<string, unknown> {
  return env.payload ?? {};
}

// re-exported for advanced users (binary stdio frames carry 16-byte ULIDs)
export { ulidFromBytes };
