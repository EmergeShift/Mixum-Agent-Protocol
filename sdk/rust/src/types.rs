//! Wire types mirroring `schemas/*.schema.json` and the spec chapters.
//! The spec is authoritative; this file must never invent semantics.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROTOCOL_VERSION: u32 = 1;

/// Canonical 26-char Crockford base32 ULID.
pub type Ulid = String;

/// Control-plane envelope (envelope.schema.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    pub v: u32,
    pub id: Ulid,
    pub op: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seq: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<Ulid>,
    /// Unix milliseconds.
    pub ts: i64,
    #[serde(default = "Value::default")]
    pub payload: Value,
}

impl Envelope {
    pub fn request(op: &str, payload: Value) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            id: crate::token::new_ulid(),
            op: op.to_owned(),
            seq: None,
            reply_to: None,
            ts: now_ms(),
            payload,
        }
    }

    pub fn is_err(&self) -> bool {
        self.op.ends_with(".err")
    }
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// map1. token claims (token.schema.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenClaims {
    pub sub: String,
    pub caps: Vec<Capability>,
    /// Unix seconds.
    pub iat: i64,
    /// Unix seconds.
    pub exp: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jti: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<Ulid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub windows: Option<Vec<u32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mem: Option<MemBinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    TaskRun,
    TaskObserve,
    SnapManage,
    SnapSync,
    DisplayView,
    DisplayInput,
    MemRead,
    MemWrite,
    VmManage,
    Admin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemBinding {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub providers: Option<Vec<String>>,
    #[serde(default)]
    pub write: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloOk {
    pub session_id: Ulid,
    pub server: String,
    pub arch: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kernel: Option<String>,
    pub caps: Vec<Capability>,
    #[serde(default)]
    pub profiles: Vec<String>,
    pub versions: Vec<u32>,
    pub heartbeat: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: Ulid,
    pub instruction: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runner: Option<String>,
    pub state: TaskState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_snap_node: Option<Ulid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Queued,
    Running,
    Suspended,
    Completed,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskEvent {
    pub task_id: Ulid,
    #[serde(rename = "type")]
    pub kind: TaskEventType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
    #[serde(default)]
    pub data: Value,
    pub ts: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskEventType {
    State,
    Log,
    Step,
    Progress,
    Error,
    Window,
    Input,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotKind {
    Disk,
    Memory,
    Hybrid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapNode {
    pub id: Ulid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<Ulid>,
    pub branch: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub kind: SnapshotKind,
    #[serde(default)]
    pub auto: bool,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    pub size_bytes: u64,
    pub chunks: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kernel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk: Option<Manifest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<Manifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub version: u32,
    #[serde(default)]
    pub files: Vec<ManifestFile>,
    #[serde(default)]
    pub dirs: Vec<ManifestDir>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestFile {
    pub path: String,
    /// Octal permission as integer (420 = 0644).
    pub mode: u32,
    pub size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symlink: Option<String>,
    /// Plaintext SHA-256 per chunk.
    #[serde(default)]
    pub chunks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestDir {
    pub path: String,
    pub mode: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Window {
    pub id: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    pub title: String,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub focused: bool,
    pub mode: WindowMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codec: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowMode {
    Agent,
    User,
    Free,
}

/// Window frame header (display.md). Little-endian on the wire; fields after
/// the 1-byte channel tag: windowId u32, width u32, height u32, size u32,
/// flags u16, cursorX i16, cursorY i16, cursorW u16, cursorH u16.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoFrameHeader {
    pub window_id: u32,
    pub width: u32,
    pub height: u32,
    pub size: u32,
    /// bit0 keyframe/IDR, bit1 cursor present.
    pub flags: u16,
    pub cursor_x: i16,
    pub cursor_y: i16,
    pub cursor_w: u16,
    pub cursor_h: u16,
}

pub const VIDEO_HEADER_LEN: usize = 27; // tag byte + fields

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Ulid>,
    pub provider: String,
    pub kind: MemoryKind,
    pub content: MemoryContent,
    pub scope: MemoryScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opaque: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    Fact,
    Preference,
    Procedure,
    Episodic,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryContent {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryScope {
    pub level: MemoryScopeLevel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#ref: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScopeLevel {
    Global,
    Project,
    Task,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vm {
    pub id: Ulid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub state: VmState,
    pub profile: VmProfile,
    pub image: VmImage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<VmNodeEndpoint>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VmState {
    Creating,
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VmProfile {
    pub vcpu: u32,
    pub mem_mib: u32,
    pub disk_gib: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VmImage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#ref: Option<String>,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VmNodeEndpoint {
    pub url: String,
    pub fingerprint: String,
}
