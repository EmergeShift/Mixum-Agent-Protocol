---
title: Transport & Security
description: WebSocket transport, TLS and TOFU, capability tokens (map1), limits and the threat model.
---

Stability: **Stable**.

## Transport

- The only transport is **WebSocket over TLS 1.3** at `wss://<host>:<port>/ws`. Plaintext `ws://` exists in test code only and must never be enabled by a conformant build.
- Clients request the subprotocol **`map.v1`**; servers must select it or reject the connection.
- Frame size limits: control (text) frames ≤ **32 MiB**; binary records ≤ **16 MiB** per record (a frame may carry one record; see [Envelope & Framing](/spec/envelope/)).
- Rate limiting: token bucket per connection, default **50 messages/s, burst 100**. Exceeding it yields `session.err/rate_limited`; sustained flooding is a disconnect.
- Slow consumers: a subscriber that stops reading a frame channel is disconnected after a server-defined grace period (default 2 s) to protect brokers.

## Identity & certificates

- A Node's identity is an **ed25519** key pair generated on first start and stored at `dataDir/identity.key` (mode 0600).
- The TLS certificate is **self-signed ECDSA P-256** (10-year validity) tied to that identity.
- Clients use **TOFU pinning**: on first connection the SHA-256 fingerprint of the leaf certificate is shown and, once confirmed, stored. Every later connection must match exactly; a mismatch is a hard disconnect (suspected man-in-the-middle), not a warning.

## Capability tokens

A token is the only way to do anything beyond `hello`. Format (dot-joined, base64url without padding):

```
map1.<base64url(payload JSON)>.<base64url(ed25519 signature)>
```

The signing key is the Node identity key. Payload claims:

| Claim | Type | Meaning |
|---|---|---|
| `sub` | string | Subject: device or client identity, e.g. `"iphone-15"`, `"mixumart-desktop"` |
| `caps` | string[] | Capability strings (see [Architecture](/spec/architecture/#the-capability-model)) |
| `iat`, `exp` | int | Issued-at / expiry, unix seconds. Expired tokens fail as `unauthorized` |
| `jti` | string? | Unique token id, reserved for revocation lists (not required in v1) |
| `taskId` | string? | Bind all task operations to this task; also **forbids `task.submit`** |
| `windows` | int[]? | Bind window operations to these window ids (absent = all) |
| `mem.providers` | string[]? | Bind memory operations to these providers (absent = all granted by caps) |

Binding rules:

- `admin` implies every capability but does **not** override bindings: a bound `taskId` or `windows` list still constrains an `admin` token.
- Tokens are signed, not encrypted; claims are readable by the holder. Never put secrets in `sub`.
- Long-lived tokens may be reused across connections. Transport replay is prevented by TLS 1.3; `jti` + a revocation list is the reserved mechanism against token theft.

### Issuance

- On first start the Node generates a **bootstrap admin token** (10-year TTL by default) and stores it 0600 alongside the identity key. Operators derive scoped tokens from it; derivation is an implementation concern (CLI or Host UI), the wire only consumes tokens.
- On the **Host** path, `vm.attach` returns a short-TTL bootstrap token for the VM's Node — the Host vouches for the client. See [VM & Host](/spec/vm-host/).
- Sync peers receive a dedicated `snap.sync` token with an independent TTL.

## Limits & defaults

| Parameter | Default | Notes |
|---|---|---|
| Control frame limit | 32 MiB | Text frames |
| Binary record limit | 16 MiB | Per record; a snapshot chunk ≤ 1 MiB nominal |
| Rate bucket | 50/s, burst 100 | Per connection |
| Heartbeat | 30 s | Client must `sys.ping` within 1.5× interval |
| Chunk size | 1 MiB nominal | Tail chunk may be shorter |
| Session event ring | ≥ 256 events | For `session.resume` replay |

All limits are minimum ceilings: implementations may enforce stricter local policies but must accept conformant traffic at these bounds.

## Threat model (v1)

| Asset | Adversary | Defense |
|---|---|---|
| Task data / workspace | Network eavesdropper | TLS 1.3 mandatory; chunks travel with plaintext-hash verification |
| Control plane | Unauthorized connection | Capability tokens, least-privilege caps, bindings |
| Window content | Over-privileged viewer | `display.view` + optional `windows` binding |
| Input control | Over-privileged injector | `display.input` + takeover state machine (agent mode hard-rejects) |
| Snapshot store at rest | Disk theft | Optional chunk-level AES-256-GCM, key derived per store (below) |
| Malicious agent | Escape to host | Container/VM isolation; agents reach the world only through MAP-Bridge `exec` |
| Host impersonation | MITM | TOFU fingerprint pinning |

Explicitly out of scope (documented, not defended): kernel escapes (delegated to the container runtime / hypervisor), physical attacks, and secrets remaining in CRIU memory images (v1: memory snapshots may contain sensitive data; encryption of memory images is a reserved v1.1 item).

### At-rest encryption (optional, normative when enabled)

- Chunk encryption: **AES-256-GCM**; key = HKDF-SHA256(masterKey, salt=`"map/snap/v1:<storeID>"`, info=`"chunk-aead"`, 32 bytes); nonce prepended; AAD binds `storeID` and the **plaintext** chunk hash (prevents chunk transplant between stores).
- The master key is 32 bytes, generated on first start, stored 0600.
- Chunk hashes on the wire are **always the plaintext hash**, regardless of storage encryption — this is what makes cross-Node deduplication work.
