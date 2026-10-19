/**
 * Wire types mirroring schemas/*.schema.json and the spec chapters.
 * The spec (site/src/content/docs/spec) is authoritative; this file must
 * never invent semantics.
 */

export const PROTOCOL_VERSION = 1 as const;
export const SUBPROTOCOL = "map.v1" as const;
/** Pre-MAP 0.1.0 nodes only negotiate this subprotocol (transition alias). */
export const LEGACY_SUBPROTOCOL = "mixum.v1" as const;

export type Ulid = string; // 26-char Crockford base32

/** Control-plane envelope (envelope.schema.json). */
export interface Envelope<P = Record<string, unknown>> {
  v: typeof PROTOCOL_VERSION;
  id: Ulid;
  op: string;
  seq?: number;
  replyTo?: Ulid;
  /** Optional gateway routing target; direct node connections ignore it. */
  node?: string;
  ts: number; // unix ms
  payload: P;
}

/** map1. capability token claims (token.schema.json). */
export interface TokenClaims {
  sub: string;
  caps: Capability[];
  iat: number; // unix s
  exp: number; // unix s
  jti?: string;
  taskId?: Ulid;
  windows?: number[];
  mem?: { providers?: string[]; write?: boolean };
}

export type Capability =
  | "task.run"
  | "task.observe"
  | "snap.manage"
  | "snap.sync"
  | "display.view"
  | "display.input"
  | "mem.read"
  | "mem.write"
  | "vm.manage"
  | "admin";

export const ERROR_CODES = [
  "bad_request",
  "unauthorized",
  "forbidden",
  "not_found",
  "conflict",
  "already_exists",
  "replay_detected",
  "busy",
  "internal",
  "unsupported",
  "rate_limited",
  "timeout",
  "unavailable",
  "snapshot_invalid",
] as const;
export type ErrorCode = (typeof ERROR_CODES)[number];

export interface HelloOk {
  sessionId: Ulid;
  server: string;
  arch: string;
  kernel?: string;
  caps: Capability[];
  profiles: string[];
  versions: number[];
  heartbeat: number;
}

export interface SysInfo {
  server: string;
  arch: string;
  kernel?: string;
  os?: string;
  hostname?: string;
  caps?: Capability[];
  profiles?: string[];
  snapshotBackend?: string;
  criuAvailable?: boolean;
  compositor?: string;
  windows?: number;
  memoryProviders?: string[];
  uptimeSeconds?: number;
}

export type TaskState = "queued" | "running" | "suspended" | "completed" | "failed" | "canceled";

export interface Task {
  id: Ulid;
  instruction: string;
  agent?: string;
  runner?: string;
  state: TaskState;
  preSnapNode?: Ulid;
  params?: Record<string, unknown>;
  error?: string | null;
  createdAt: number;
  startedAt?: number | null;
  endedAt?: number | null;
}

export interface SnapshotPolicy {
  pre?: "always" | "never";
  kind?: "disk" | "memory" | "hybrid";
  onFailure?: "rollback" | "keep";
}

export interface TaskSubmitRequest {
  instruction: string;
  agent?: string;
  runner?: string;
  params?: Record<string, unknown>;
  snapshotPolicy?: SnapshotPolicy;
}

export type TaskEventType = "state" | "log" | "step" | "progress" | "error" | "window" | "input";

export interface TaskEvent {
  taskId: Ulid;
  type: TaskEventType;
  step?: string;
  data: Record<string, unknown>;
  ts: number;
}

export type SnapshotKind = "disk" | "memory" | "hybrid";

export interface SnapNode {
  id: Ulid;
  parent?: Ulid | null;
  branch: string;
  label?: string | null;
  kind: SnapshotKind;
  auto?: boolean;
  createdAt: number;
  origin?: string | null;
  sizeBytes: number;
  chunks: number;
  kernel?: string | null;
  arch?: string | null;
  disk?: Manifest;
  memory?: Manifest;
}

export interface Manifest {
  version: 1;
  files: ManifestFile[];
  dirs: ManifestDir[];
}

export interface ManifestFile {
  path: string;
  mode: number;
  size: number;
  symlink?: string;
  chunks: string[]; // plaintext sha256 per chunk
}

export interface ManifestDir {
  path: string;
  mode: number;
}

export interface Window {
  id: number;
  appId?: string;
  title: string;
  width: number;
  height: number;
  focused?: boolean;
  mode: "agent" | "user" | "free";
  codec?: "jpeg" | "h264";
}

export type VideoCodec = "jpeg" | "h264";

/** Video record: frame header (27 bytes) + encoded payload (display.md). */
export interface VideoFrameHeader {
  windowId: number;
  width: number;
  height: number;
  size: number;
  flags: number; // bit0 keyframe/IDR, bit1 cursor present
  cursorX: number;
  cursorY: number;
  cursorW: number;
  cursorH: number;
}

export interface MemoryRecord {
  id?: Ulid;
  provider: string;
  kind: "fact" | "preference" | "procedure" | "episodic" | "custom";
  content: { text: string; structured?: Record<string, unknown> };
  scope: { level: "global" | "project" | "task"; ref?: string };
  source?: { platform?: string; origin?: string };
  tags?: string[];
  createdAt?: number;
  updatedAt?: number;
  confidence?: number;
  expiresAt?: number | null;
  opaque?: string | null;
}

export interface MemoryProvider {
  id: string;
  kind: string;
  status: string;
  capabilities: string[];
  searchSemantic?: boolean;
}

export interface Vm {
  id: Ulid;
  name?: string;
  state: "creating" | "stopped" | "starting" | "running" | "stopping" | "failed";
  profile: { vcpu: number; memMiB: number; diskGiB: number };
  image: { ref?: string; uri?: string; sha256: string };
  node?: { url: string; fingerprint: string } | null;
  createdAt: number;
}

export interface VmAttach {
  node: {
    url: string;
    fingerprint: string;
    token: string;
    expiresAt: number;
  };
}

/** MAP-Bridge message (bridge-message.schema.json). */
export type BridgeMessage =
  | { type: "hello"; instruction: string; params: Record<string, unknown>; env?: Record<string, unknown> }
  | { type: "exec.done"; id: string; code: number | null; error?: string }
  | { type: "input.response"; id: string; value: string }
  | { type: "suspend" }
  | { type: "resume" }
  | { type: "log"; line: string }
  | { type: "progress"; pct?: number; step?: string }
  | { type: "exec"; id: string; argv: string[]; cwd?: string; env?: Record<string, string>; timeoutMs?: number }
  | { type: "input.request"; id: string; prompt: string; choices?: string[]; sensitive?: boolean; timeoutMs?: number }
  | { type: "done"; result?: unknown }
  | { type: "fail"; error: string; code?: string };
