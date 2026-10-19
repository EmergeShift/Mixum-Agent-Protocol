---
title: Envelope & Framing
description: The control-plane envelope, error codes, binary frame channels and identifier rules.
---

Stability: **Stable**. The envelope and the binary channel tags are frozen for MAP v1; changes require a major version.

## Control plane (text frames)

Every text frame is exactly one JSON envelope, UTF-8:

```json
{
  "v": 1,
  "id": "01JBV7W8X5Q3M2N4P6R8T0V2W4",
  "op": "task.submit",
  "seq": 42,
  "replyTo": "01JBV7W8X5Q3M2N4P6R8T0V2W3",
  "node": "home-mini",
  "ts": 1696400000123,
  "payload": { "instruction": "…" }
}
```

| Field | Type | Rules |
|---|---|---|
| `v` | int | Protocol major version. Mismatch ⇒ reject every post-handshake message with `unsupported` |
| `id` | string | **ULID** (26-char canonical). Replies reference the request via `replyTo` |
| `op` | string | Operation name, `domain.verb`. Replies use `<op>.ok` / `<op>.err` |
| `seq` | int? | Per-connection monotonic counter (optional but recommended; used by `session.resume`) |
| `replyTo` | string? | ULID of the request being answered |
| `node` | string? | Optional gateway routing target. Direct node connections ignore it; gateway clients set it to select an online node |
| `ts` | int | Unix milliseconds |
| `payload` | object | Op-specific; never `null`, may be `{}` |

Unknown fields must be **ignored by receivers** (additive evolution). Unknown `op` values reply `unsupported`.

## Error replies

`.err` payload:

```json
{ "code": "forbidden", "message": "token lacks capability: display.input" }
```

`code` is a stable, programmatic identifier; `message` is for humans and may change.

| Code | Meaning |
|---|---|
| `bad_request` | Malformed payload, failed validation |
| `unauthorized` | Missing/expired/invalid token |
| `forbidden` | Authenticated but capability or binding missing |
| `not_found` | Referenced task/snapshot/window/record does not exist |
| `conflict` | State preconditions failed (e.g. injecting into an agent-mode window) |
| `already_exists` | Creating something that exists (branch name, image ref) |
| `replay_detected` | Duplicate/already-used message or token where uniqueness is required |
| `busy` | Resource temporarily busy; safe to retry |
| `internal` | Server-side failure |
| `unsupported` | Op/codec/profile/version not supported by this implementation |
| `rate_limited` | Rate bucket exhausted |
| `timeout` | Deadline exceeded (e.g. input.request unanswered) |
| `unavailable` | Capability present but backing subsystem down (e.g. compositor offline) |
| `snapshot_invalid` | Snapshot node/manifest/chunks failed validation or restore guards |

## Binary plane (binary frames)

A binary frame carries **one record**: `[channel tag: u8][record]`.

| Tag | Channel | Record layout |
|---|---|---|
| `0x01` | video | [Window frame header](/spec/display/#window-frame-header) + encoded frame |
| `0x02` | audio | Reserved (no layout in v1) |
| `0x03` | chunk | `[32 bytes: plaintext SHA-256][chunk bytes]` — see [Mobility](/spec/mobility/) |
| `0x04` | stdio | `[stream: u8 (1=stdout, 2=stderr)][task ULID: 16 bytes binary][bytes]` |

Delivery semantics per channel:

| Channel | Ordering | Reliability |
|---|---|---|
| video | Not guaranteed (latest-wins is fine) | At-most-once; loss tolerable |
| chunk | Ordered per push session | Reliable — completeness is enforced by `snap.push.commit` |
| stdio | Ordered per stream | At-most-once; events also mirror notable output |

Control-plane messages are always reliable and ordered per connection.

## Identifiers

- **ULID** (universally unique lexicographically sortable identifier) is mandated for: envelope ids, task ids, snapshot node ids, memory record ids. ULIDs sort by creation time — free causality in listings and logs.
- Binary contexts use the 16-byte ULID form; JSON uses the 26-character canonical Crockford base32 form.
- Window ids are **server-scoped u32** integers, reused only after window destruction.
- Branch names: user-visible strings; uniqueness enforced per tree; collisions auto-suffix `-2`, `-3`, …
