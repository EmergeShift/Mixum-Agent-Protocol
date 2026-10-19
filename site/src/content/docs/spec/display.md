---
title: Display & Interaction
description: Window enumeration, codec-negotiated streaming, the frame header, input events and the takeover state machine.
---

Stability: **Stable**. The window frame header layout is frozen; changing it is a major version.

MAP streams the GUI of the sandbox to any client. Each window is captured independently by the Node's compositor — clients attach per window, never to a whole screen.

## Windows

```json
{
  "id": 7,
  "appId": "org.chromium.Chromium",
  "title": "bilibili — mixumOS",
  "width": 1280,
  "height": 800,
  "focused": false,
  "mode": "agent",
  "codec": "h264"
}
```

`mode` is the takeover state: `agent` (task-owned, watch-only for clients), `user` (claimed by a client), `free` (idle).

## Ops

| Op | Request | Reply |
|---|---|---|
| `display.list` | — | `{ windows: [Window] }` |
| `display.attach` | `{ window, fps?, codec?: "jpeg"\|"h264" }` | `{ window, codec, width, height, fps }` — **codec is the negotiated one**, video frames follow on the binary channel |
| `display.detach` | `{ window }` | `{ window }` |
| `display.windows` | — | `{ subscribed }` — pushes window add/remove/rename/codec-change events |
| `display.input` | `{ window, type, … }` | `{ ok: true }` |
| `display.takeover` | `{ window, action: "claim"\|"release", force? }` | `{ mode }` |

**Codec negotiation.** `jpeg` is mandatory for every MAP-Display implementation; `h264` is optional. A client requesting `h264` from a Node without an encoder silently gets `jpeg` — the attach reply's `codec` field is authoritative, and a codec change is broadcast on the window subscription so all attached clients reconfigure. JPEG frames are byte-identical full frames; H.264 payloads are **Annex-B** with the same frame header (`flags` bit0 = keyframe/IDR): on attach the Node sends an IDR immediately, then at least one IDR every 2 seconds.

## Window frame header

Little-endian, prefixed to every video record:

```
[0]      u8   0x01                channel tag (repeated as a check)
[1:5]    u32  windowId
[5:9]    u32  width
[9:13]   u32  height
[13:17]  u32  size                encoded data length
[17:19]  u16  flags               bit0 keyframe/IDR, bit1 cursor-present
[19:21]  i16  cursorX             physical px, relative to window top-left
[21:23]  i16  cursorY
[23:25]  u16  cursorW             cursor draw area
[25:27]  u16  cursorH
[27:]    —    encoded frame (v1: JPEG; or H.264 Annex-B when negotiated)
```

Design notes: every JPEG frame is a keyframe, so clients can join mid-stream at any frame. The cursor position rides in the header — clients draw it over the canvas, so the pointer is always consistent with the frame it came from, never extrapolated.

## Input events

`display.input { window, type, … }`:

| `type` | Fields | Notes |
|---|---|---|
| `pointer_motion` | `x, y` | Physical px inside the window |
| `pointer_button` | `button: "left"\|"right"\|"middle"`, `action: "press"\|"release"\|"click"` | |
| `pointer_axis` | `action: "scroll_up"\|"scroll_down"` | |
| `key` | `key` (evdev code) or `keysym` (XKB name, e.g. `"a"`, `"Return"`), `action`, `mods?: ["ctrl"\|"alt"\|"shift"\|"logo"]` | |

Injection into a window in `mode: "agent"` replies `conflict` — claim it first.

## Takeover state machine

```
            claim (client)                claim
  free ────────────────▶ user        agent ──────▶ user
   ▲                      │            ▲             │
   │        release       │            │  release    │
   └──────────────────────┘            └─────────────┘
```

- `claim` on an agent window: allowed for clients with `display.input`. The runner forwards `suspend` to the owning agent (via the bridge). The window becomes `user`.
- `release`: returns the window to `free`; the agent is told to resume.
- `force: true` re-claims from another client (politeness is the client's problem; the protocol records who holds the claim).
- Task end (any terminal state) returns all of the task's windows to `free`.

## Private control channel

The record-framed channel between the Node and its compositor (`mixumd` ↔ `mixcomp` in mixumOS) is **private**, but its evolution follows the same additive rule as the public protocol: record types evolve incrementally, and a receiver **must ignore unknown record types** (logging/counting them is encouraged) instead of closing the connection. Framing or payload validation failures are a separate, explicit error path and may terminate the channel. This rule is a minor clarification (2026-10); it does not change any wire layout.

