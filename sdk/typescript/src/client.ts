import { MapError } from "./errors.js";
import type { ErrorCode } from "./types.js";
import { ulid, ulidFromBytes } from "./ulid.js";
import {
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
  readonly hello: HelloOk;

  private constructor(ws: WebSocket, hello: HelloOk, requestTimeoutMs: number) {
    this.#ws = ws;
    this.hello = hello;
    this.#requestTimeoutMs = requestTimeoutMs;
    ws.binaryType = "arraybuffer";
    ws.addEventListener("message", (ev) => this.#onMessage(ev.data));
    ws.addEventListener("close", () => this.#onClose());
  }

  /** Connect, handshake, and negotiate. Rejects on hello.err or timeout. */
  static async connect(opts: MapConnectOptions): Promise<MapClient> {
    const WS = opts.WebSocketImpl ?? WebSocket;
    const ws = new WS(opts.url, SUBPROTOCOL);
    await new Promise<void>((resolve, reject) => {
      if (ws.readyState === ws.OPEN) return resolve();
      ws.addEventListener("open", () => resolve(), { once: true });
      ws.addEventListener("error", () => reject(new MapError("unavailable", "connection failed")), { once: true });
    });

    const timeoutMs = opts.requestTimeoutMs ?? 30_000;
    const hello = await new Promise<HelloOk>((resolve, reject) => {
      const timer = setTimeout(() => reject(new MapError("timeout", "handshake timed out")), timeoutMs);
      const onMsg = (ev: MessageEvent): void => {
        const env = parseEnvelope(ev.data);
        if (!env) return;
        const isOk = env.op === "session.hello.ok" || env.op === "hello.ok";
        const isErr = env.op.endsWith(".err");
        if (!isOk && !isErr) return;
        clearTimeout(timer);
        ws.removeEventListener("message", onMsg);
        if (isOk) resolve(envelopePayload(env) as unknown as HelloOk);
        else {
          const p = envelopePayload(env) as { code?: string; message?: string };
          reject(new MapError(errCode(p.code), p.message ?? "handshake failed"));
        }
      };
      ws.addEventListener("message", onMsg);
      const env: Envelope = {
        v: PROTOCOL_VERSION,
        id: ulid(),
        op: "session.hello",
        ts: Date.now(),
        payload: { token: opts.token, client: "@mixum/map/0.1.0" },
      };
      ws.send(JSON.stringify(env));
    });

    return new MapClient(ws, hello, timeoutMs);
  }

  get sessionId(): Ulid {
    return this.hello.sessionId;
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
      for (const h of [...this.#pushHandlers]) h(env);
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
