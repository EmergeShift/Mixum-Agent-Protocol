---
title: Agents & Bridge
description: How agents plug into a Node — driver registry, the normative MAP-Bridge sub-protocol, sandbox contract and GUI cooperation.
---

Stability: **Stable** (bridge framing and messages). The builtin-driver trait is implementation-specific and **not** part of the wire.

A Node executes tasks through **agents**. MAP defines two integration modes:

1. **Builtin drivers** — compiled into the runner (e.g. a Rust `AgentDriver` trait). Powerful, but implementation-private.
2. **MAP-Bridge** — a tiny line-delimited JSON sub-protocol over the agent process's stdio. **Normative, language-agnostic**, and the only way third-party agents integrate.

## Driver registry

`agent.list` (requires `task.observe`):

```json
{ "agents": [ { "name": "external/claude-code", "kind": "external", "version": "1.2.0" } ] }
```

| Field | Meaning |
|---|---|
| `name` | Referenced by `task.submit { agent }` |
| `kind` | `builtin` \| `external` |
| `version` | Reporting only; clients must not branch on it |

## MAP-Bridge

The runner starts the agent process with the instruction on **stdin**; the agent narrates and requests work on **stdout**. Framing: **line-delimited UTF-8 JSON** (LF separators). Any stdout line that is not valid JSON is treated as a `log` line — a chatty agent can never corrupt the stream.

### Runner → agent

| Message | Payload | Meaning |
|---|---|---|
| `hello` | `{ instruction, params, env: { arch, kernel, workspace } }` | Always the first line |
| `exec.done` | `{ id, code, error? }` | Result of a previous `exec` |
| `input.response` | `{ id, value }` | Answer to `input.request` (human's `task.reply`) |
| `suspend` | `{}` | Agent should pause cleanly (user takeover / snapshot freeze incoming) |
| `resume` | `{}` | Continue |

### Agent → runner

| Message | Payload | Meaning |
|---|---|---|
| `log` | `{ line }` | Progress narration (surfaced as `task.event/log`) |
| `progress` | `{ pct?, step? }` | Progress update |
| `exec` | `{ id, argv, cwd?, env?, timeoutMs? }` | Request command execution — **the only world access** |
| `input.request` | `{ id, prompt, choices?, sensitive?, timeoutMs? }` | Ask the human (see [Tasks](/map/spec/tasks/#the-human-input-channel)) |
| `done` | `{ result? }` | Task succeeded |
| `fail` | `{ error, code? }` | Task failed |

## Sandbox contract

When the agent sends `exec`, the runner executes it inside the sandbox with hard guarantees. These guarantees are **why** agents must never touch process APIs directly:

- The child runs in a dedicated **process group**; the runner can freeze it (SIGSTOP / cgroup freezer) so snapshots are consistent and invisible.
- stdout/stderr stream back as `task.event/log` and, where supported, `stdio` binary frames.
- `timeoutMs` (default 60 000) → `exec.done { code: null, error: "timeout" }`; the process group is killed.
- Filesystem access is confined to the workspace root; network is the sandbox's own.
- `exec` requests are serialized per task; the runner may pipeline but must answer in order.

## Environment

The runner starts the agent with a minimal environment (no inherited shell history, no host secrets):

| Variable | Value |
|---|---|
| `MAP_INSTRUCTION` | The task instruction (also in `hello`) |
| `MAP_TASK_ID` | Task ULID |
| `MAP_AGENT_NAME` | Registry name |
| `MAP_BRIDGE` | `1` |

(Legacy `MIXUM_*` names are accepted by adapters but must not be emitted by conformant runners.)

## Lifecycle

- **Success** = exit code 0 **and** no `fail` sent. Everything else is failure; a `fail` message always wins over the exit code.
- **Cancel** = runner closes stdin, sends SIGTERM to the process group, waits a 5 s grace, then SIGKILL. The task becomes `canceled`.
- **Suspend/resume**: `suspend` is advisory — agents that cannot pause may keep running but must tolerate `exec.done` latency during snapshot freezes. The runner never drops an in-flight `exec` on suspend.

## GUI cooperation

- When the agent opens a GUI application (typically via `exec`), the runner marks the new window `mode: "agent"`: **anyone may watch, no one may inject** (input injection replies `conflict`).
- The agent drives its own GUI through `exec` (CLI automation tools) or a Node's native input API for builtin drivers.
- A user may claim a window (`display.takeover { action: "claim" }`): the window flips to `mode: "user"`, the runner forwards `suspend` to the agent. Releasing returns it to `free` and the task may resume normally. See [Display](/map/spec/display/).

## Conformance

Bridge conformance = the message round-trip vectors in [`vectors/`](https://github.com/EmergeShift/Mixum-Agent-Protocol/tree/main/vectors): a scripted agent + a scripted runner must complete a full session (hello → exec round-trip → input.request/response → done) byte-for-byte modulo ULIDs and timestamps.
