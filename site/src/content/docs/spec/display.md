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
  "codec": "png"
}
```

`mode` is the takeover state: `agent` (task-owned, watch-only for clients), `user` (claimed by a client), `free` (idle).

## Ops

| Op | Request | Reply |
|---|---|---|
| `display.list` | — | `{ windows: [Window] }` |
| `display.attach` | `{ window, fps?, codec?: "jpeg"\|"h264"\|"png", quality?: 1–100 }` | `{ window, codec, width, height, fps }` — **codec is the negotiated one**, video frames follow on the binary channel |
| `display.detach` | `{ window }` | `{ window }` |
| `display.windows` | — | `{ subscribed }` — pushes window add/remove/rename/codec-change events |
| `display.input` | `{ window, type, … }` | `{ ok: true }` |
| `display.takeover` | `{ window, action: "claim"\|"release", force? }` | `{ mode }` |
| `display.clipboard.set` | `{ window, text }` | `{ ok: true }` |
| `display.clipboard.subscribe` | — | `{ subscribed: true }` — opts the connection into `display.clipboard.changed` |
| `display.clipboard` | — | `{ text }` — read-only pull from the B compositor (compatibility) |

**Codec negotiation.** `jpeg` is the mandatory baseline for interoperability; `h264` is optional. A client requesting `h264` from a Node without an encoder silently gets `jpeg` — the attach reply's `codec` field is authoritative, and a codec change is broadcast on the window subscription so all attached clients reconfigure. JPEG frames are byte-identical full frames; H.264 payloads are **Annex-B** with the same frame header (`flags` bit0 = keyframe/IDR): on attach the Node sends an IDR immediately, then at least one IDR every 2 seconds.

`png` is the lossless damage codec used by mixumOS and MixumART for local/LAN viewers. It is implemented end to end by both reference implementations, but it is not yet part of the conformance suite; a third-party Node may fall back to JPEG. Clients must honor the `codec` in the attach reply.

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
[27:]    —    encoded frame (JPEG; H.264 Annex-B when negotiated; PNG or PNG patch when `codec: "png"`)
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

## Clipboard

> **Draft extension (RFC 0004).** Clipboard sync is additive and introduces no
> new capability; clients must tolerate Nodes that do not implement it. The
> rest of this chapter remains Stable.

MAP-Display carries **text** clipboard content per window. Rich MIME (images,
files, `text/html`), history, cross-node sync and multi-seat are out of scope
for v1.

`display.clipboard.set { window, text }` writes text into the selection of the
compositor that owns `window`:

- requires `display.input`; when the token carries a `windows` claim, it must
  cover `window`;
- `text` is non-empty UTF-8, at most **4 MiB counted in UTF-8 bytes**; empty
  text is `bad_request` (clearing the selection is not in v1);
- an `agent`-owned window replies `conflict` (claim it first), and so does a
  compositor that has not declared clipboard support;
- an unknown window is `not_found`; a disconnected owner is `conflict`;
- success means the text reached the owning compositor's control channel, not
  that the application has read it.

`display.clipboard.changed { window, text }` is pushed when the selection owned
by `window` changes. Delivery requires an explicit `display.clipboard.subscribe`
(requires `display.view`); a `display.windows` subscription alone does **not**
opt a client into clipboard text — watching window metadata is not consent for
clipboard content. Subscribe state is not restored by `session.resume` and
events are never replayed. Events are still filtered per window by the token's
`windows` claim. v1 synchronizes **non-empty text only**: clearing a selection
does not emit a changed event. Content over 4 MiB of UTF-8 bytes is truncated
and discarded as a whole; the byte limit is enforced at dispatch level
(`vectors/clipboard/byte-limit.json` carries a multi-byte over-limit case that
passes the schema and must be rejected). The event carries no compositor id.
Clients **SHOULD** suppress echo: a changed event equal to the last text they
set for that window should not be written back to the host clipboard.

The legacy `display.clipboard` pull is unchanged: read-only, best-effort from
the B compositor, no window routing or ACL, no change notification. Clients
should prefer the window-scoped operations and fall back to the pull only when
the window's compositor does not implement Route A.

The Node↔compositor capability handshake and `clipboard.set` / `clipboard`
records are private, but follow the same forward-compatibility rule as the
private control channel: unknown record types are ignored and counted.

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


## Damage patches (PNG)

`codec: "png"` negotiates a lossless, damage-based window stream for local/LAN
viewers. The binary `video` channel keeps the 27-byte frame header; when
`flags` bit2 is set the payload starts with a 20-byte little-endian patch
header, followed by the encoded patch:

```
[27:31]   u32 x
[31:35]   u32 y
[35:39]   u32 w
[39:43]   u32 h
[43:47]   u32 format      0 = cursor-only (no image), 2 = PNG
[47:...]  PNG bytes (w*h)
```

Semantics:

- The first frame after attach, and any frame after a size change, is a full
  frame (`flags` bit2 clear) with the window's complete PNG.
- Subsequent frames carry only the bounding box of changed pixels; a frame
  with no changes is omitted. `format: 0` updates the cursor overlay without
  touching the image.
- The host composites patches onto its last full frame; it must not clear or
  resize the canvas for a patch. Patches are ordered and must not be dropped.
- `quality` applies to lossy codecs (`jpeg`); PNG ignores it. A Node may
  clamp or ignore it.
- Conformance status: **implemented and exercised by both reference
  implementations, but not yet part of the conformance suite**. A future RFC
  may formalize the patch header and add damage-region lists.
