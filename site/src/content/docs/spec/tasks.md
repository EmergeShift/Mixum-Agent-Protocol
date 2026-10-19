---
title: Tasks
description: Task lifecycle, ops, events, the pre-task snapshot guarantee, and the human-input channel.
---

Stability: **Stable**.

A **task** is one unit of agent work: an instruction, an agent (driver) that executes it, a lifecycle, an event stream, and a guaranteed pre-task snapshot.

## Task object

```json
{
  "id": "01JBV9C2…",
  "instruction": "打开 b 站搜 mixumOS",
  "agent": "external/claude-code",
  "runner": "container",
  "state": "running",
  "preSnapNode": "01JBV9BZ…",
  "params": {},
  "error": null,
  "createdAt": 1696400000123,
  "startedAt": 1696400001200,
  "endedAt": null
}
```

## State machine

```
queued ──▶ running ──▶ completed
   │          │  │
   │          │  └──▶ suspended ──▶ running
   │          └────────▶ failed
   └────────────▶ canceled
```

- `queued → running`: the runner starts the agent **after** taking the pre-task snapshot.
- `suspended`: transient, caused by user takeover of all agent windows or a runner-driven freeze (e.g. memory snapshot of the task's process tree). It ends when the cause ends.
- `canceled`: client requested cancellation, or the runner killed the agent.
- `failed`: agent reported failure (`fail` / nonzero exit) or a runner-internal error. Terminal.
- A task is **immutable** once terminal; results live in the workspace and snapshot tree.

## Ops

| Op | Request payload | Reply payload |
|---|---|---|
| `task.submit` | `{ instruction, agent?, runner?, params?, snapshotPolicy? }` | Task (with `preSnapNode` once taken; the reply is sent after the snapshot completes) |
| `task.list` | `{ state?, limit?, cursor? }` | `{ tasks: [Task], nextCursor? }` |
| `task.get` | `{ id }` | Task |
| `task.cancel` | `{ id }` | `{ id }` |
| `task.events` | `{ taskId? }` (absent = all tasks) | `{ subscribed }` — pushes follow |
| `task.reply` | `{ taskId, replyId, value }` | `{ delivered }` — answers an agent `input.request` |

`snapshotPolicy` (all optional, server defaults apply):

| Field | Values | Default | Meaning |
|---|---|---|---|
| `pre` | `"always" \| "never"` | `"always"` | Take a pre-task snapshot. `never` requires a token with `admin` |
| `kind` | `disk \| memory \| hybrid` | `disk` | Snapshot kind for the pre-task point |
| `onFailure` | `"rollback" \| "keep"` | `"keep"` | Automatic restore to the pre-task node on `failed` |

Binding rule: a token with a bound `taskId` **cannot** call `task.submit` (`forbidden`) — it may only observe or act on that task.

## Events

Pushed as `task.event` (a server-initiated envelope, `replyTo` unset):

```json
{
  "taskId": "01JBV9C2…",
  "type": "progress",
  "step": "browse",
  "data": { "pct": 30 },
  "ts": 1696400003400
}
```

| `type` | `data` | Meaning |
|---|---|---|
| `state` | `{ state }` | State-machine transition |
| `log` | `{ line }` | A line of agent/process output |
| `step` | `{ name }` | Agent-declared step began |
| `progress` | `{ pct?, step? }` | Progress update, `pct` 0–100 |
| `error` | `{ code?, message }` | Non-fatal error report |
| `window` | Window object | Task opened/closed a GUI window (clients may auto-attach) |
| `input` | `{ replyId, prompt, choices?, sensitive? }` | The agent asks the human (see below) |

## The human-input channel

Agents often need a decision (a permission, a choice). MAP standardizes the round trip:

1. Agent (via the bridge) sends `input.request { id, prompt, choices?, sensitive?, timeoutMs? }`.
2. Runner surfaces it to all observing clients as `task.event { type: "input" }` with `replyId = id`.
3. A client answers with `task.reply { taskId, replyId, value }` — `value` is the chosen string, or free text.
4. Runner delivers it to the agent as `input.response { id, value }` over the bridge.

Rules: one `input.request` may be answered once (`conflict` afterwards); unanswered requests expire after `timeoutMs` (default 120 s) and surface to the agent as a `timeout` error; `sensitive: true` tells clients to mask the reply (e.g. passwords).
