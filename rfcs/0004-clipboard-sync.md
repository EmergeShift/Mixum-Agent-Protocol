# RFC 0004 — Clipboard sync (Route A): window-scoped bidirectional text

* RFC: 0004
* Title: Clipboard sync (Route A) — window-scoped bidirectional text
* Status: Draft
* Type: Standards Track
* Author: yubing0721
* Created: 2026-10-25

## Summary

Add a window-scoped, bidirectional text clipboard to MAP-Display:

- `display.clipboard.set` — a client writes text into the selection of the
  compositor that owns a window;
- `display.clipboard.changed` — the Node pushes the new text when the selection
  owned by that window changes;
- `display.clipboard` — the existing read-only pull from the B compositor stays
  as a compatibility path.

No new capability is introduced: `set` reuses `display.input`, and `changed`
is delivered only to sessions that subscribed to `display.windows` **and**
whose token window ACL covers the window. The change is additive and does not
bump the wire major version.

## Motivation

MAP currently exposes only a read-only pull:

- it is hard-wired to the B compositor, so windows captured by `mixcomp`
  (Route A) cannot provide their selection;
- there is no write path, so an operator UI cannot paste text into the sandbox;
- there is no change push, so a client has to poll;
- the selection is global to the Node, not bound to a window, so it cannot
  apply window ACLs or the `agent`/`user` takeover rules.

The accepted mixumOS design
([`docs/clipboard.md`](https://github.com/EmergeShift/mixumOS/blob/main/docs/clipboard.md))
defines the compositor-native route. This RFC lifts that design into the
vendor-neutral MAP contract.

## Specification detail

### Ops

| Op | Request | Reply |
|---|---|---|
| `display.clipboard.set` | `{ window, text }` | `{ ok: true }` |
| `display.clipboard` | — | `{ text }` |
| `display.clipboard.changed` | server push, payload `{ window, text }` | — |

### `display.clipboard.set`

- Requires the `display.input` capability and a token whose `windows` claim
  (when present) covers `window`.
- `window` is a server-scoped u32 window id. A nonexistent window replies
  `not_found`.
- `text` MUST be a non-empty UTF-8 string of at most **4 MiB, counted in
  UTF-8 bytes**. Empty text replies `bad_request`; clearing the selection is
  not part of v1.
- A window in `mode: "agent"` replies `conflict`; the same takeover rules as
  `display.input` apply. The client must claim the window first.
- If the owning compositor has not declared clipboard support, the Node
  replies `conflict`. A success reply means the text reached the compositor's
  control channel; it does not guarantee that the application has read it.
- The Node routes the request only to the compositor that owns the window.
  With multiple compositors, a failed owner lookup is `not_found` and a
  disconnected owner is `conflict`.
- A gateway forwards the envelope by `node`; it does not parse or rewrite the
  payload.

### `display.clipboard.changed`

- Pushed when the selection of a window changes. Payload:

  ```json
  { "window": 7, "text": "hello" }
  ```

- Delivery requires a `display.windows` subscription **and**, when the token
  carries a `windows` claim, a claim that covers `window`. `display.view` alone
  is not sufficient: clipboard content is more sensitive than window imagery.
- The event is not delivered to a compositor or a client that has not passed
  the window ACL check.
- `text` is UTF-8 and at most 4 MiB. Content over the limit is truncated and
  discarded as a whole; no partial text is pushed.
- The event carries no compositor id. `window` is enough for window-level
  synchronization and echo suppression.
- Implementations **MUST NOT** replay `display.clipboard.changed` in the
  session resume ring (see Session & System). Clipboard text is sensitive and
  may be stale after a reconnect.
- Clients **SHOULD** suppress echo: a changed event whose text equals the
  last text the client set for that window should not be written back to the
  host clipboard.

### `display.clipboard` (compatibility)

The existing pull keeps its current semantics:

- no parameters, reply `{ text }`;
- read-only, best-effort, from the B compositor only;
- no window routing, no window ACL, no change push;
- retained for older clients and for windows whose compositor does not
  implement Route A.

New clients should prefer the window-scoped operations when the Node
advertises the capability at the compositor level; otherwise they may fall
back to the pull.

### Errors

| Code | Condition |
|---|---|
| `bad_request` | empty or oversized `text`, malformed window id |
| `forbidden` | token lacks `display.input`, or window ACL rejects the window |
| `not_found` | window does not exist |
| `conflict` | agent-owned window, missing compositor clipboard capability, or disconnected owner |

### Security and privacy

- `set` adds no capability bit; it is gated by `display.input` and the window
  ACL, exactly like input injection.
- `changed` is ACL-filtered and is deliberately excluded from the session
  replay ring.
- Implementations **MUST NOT** log clipboard contents. Logs may record the
  window id, byte length, and outcome.
- The gateway routes by `node` only; device tokens do not gain clipboard
  permissions by traversing it.

### Compositor contract (private)

The Node↔compositor channel is private, but the required behavior is:

- after connecting, a compositor reports a `capabilities` message with
  `clipboard: true`; without it, `set` is rejected with `conflict`;
- the Node sends `clipboard.set { id, text }` to the owner only;
- the compositor reports `clipboard { id, text }` after the selection changes;
  the Node drops reports from a compositor that does not own the window and
  counts them as protocol errors.

Unknown private record types are ignored, per the existing forward
compatibility rule.

## Alternatives considered

- **B-only proxy.** Spawn a clipboard client per selection and poll. It cannot
  cover `mixcomp` windows, cannot apply window ACLs, and cannot push changes.
- **Host clipboard polling.** Poll the host clipboard and inject text on every
  focus change. This loses window ownership, creates feedback loops, and does
  not work for remote Nodes.
- **Rich MIME in v1.** Images and `text/html` would require a different
  payload model, size accounting, and security review. Deferred to v2.

## Drawbacks

- Clipboard text is sensitive; the spec must exclude it from replay and require
  ACL filtering, which is easy to get wrong.
- Bidirectional sync needs echo suppression on the client; a naive client can
  create an infinite loop.
- Compositors must implement the private clipboard record and capability
  handshake. A compositor that does not will return `conflict`, so clients
  must handle partial support.
- v1 is text-only; rich MIME, history, cross-node sync, and multi-seat are out
  of scope.

## Unresolved questions

- When should the B compositor implement Route A? Until it does, `set` on its
  windows returns `conflict`; the pull remains the only option there.
- Explicit `display.clipboard.clear` semantics (v1 rejects empty text).
- Whether a future revision should expose a per-window read for resync after a
  missed event; v1 has no such op.
- Rich MIME, history and cross-node clipboard sync are v2 topics.

## Prior art

- Wayland `wlr-data-control` / data-device selection ownership.
- SPICE and VNC clipboard channels: bidirectional text with size limits and
  host-side permission prompts.
- Remote desktop clipboard bridges generally separate "selection changed" from
  "set selection" and use a client-side echo guard, as proposed here.
