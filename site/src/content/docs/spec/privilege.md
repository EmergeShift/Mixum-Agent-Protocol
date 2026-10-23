---
title: Privilege
description: Agent requests for one-shot root execution, human approval, executable pinning and the audit boundary.
---

Stability: **Experimental** (mixumOS extension; not part of the v1 conformance set).

A Node does not give an agent a root shell. Agents run as an unprivileged task user;
a privileged operation is an explicit request that a human (or a CLI holding an
Admin token) approves or denies.

Machine-readable forms: [`privilege.schema.json`](https://github.com/ashreo/Mixum-Agent-Protocol/blob/main/schemas/privilege.schema.json)
and the [approval-flow vectors](https://github.com/ashreo/Mixum-Agent-Protocol/blob/main/vectors/privilege/approval-flow.json).

## Request event

The Node pushes to every connected session with the `admin` capability:

```json
{
  "op": "privilege.request",
  "payload": {
    "id": "01JBV9C2Q7R9S1T3V5W7X9Y2Z4",
    "taskId": "01JBV9C2Q7R9S1T3V5W7X9Y2Z7",
    "peerUid": 0,
    "program": "npm",
    "resolved": "/usr/bin/npm",
    "sha256": "eec3baf7cedb1f0b0f9570b18b78ee0b87eb3ea1ce9389062b1b4b6d353db87b",
    "args": ["install", "-g", "typescript"],
    "cwd": "/var/lib/mixum/work",
    "reason": "install the project toolchain",
    "risk": "package",
    "createdAt": 1790000000000,
    "expiresAt": 1790000120000
  }
}
```

The `sha256` in this example is an illustrative placeholder (the SHA-256 of a
sample byte string), not the hash of any host's `/usr/bin/npm`. Real request
events must pin the bytes of the resolved executable.

- `program` is the exact argv[0] the agent asked for. `resolved` is the absolute,
  symlink-free executable that will actually run, and `sha256` pins its bytes.
  Approvers should compare both; `program` alone is not the executed object.
  Nodes that predate this hardening omit `resolved`/`sha256`; clients must still
  render such requests and may mark them as unpinned.
- `peerUid` is the kernel-reported uid of the local sudo client (`null` when the
  Node cannot obtain peer credentials). It is diagnostic, not authorization.
- `taskId` is a requester-supplied hint and is **not authenticated**. Nodes must
  not authorize on it; it may be empty or forged.
- `risk` is advisory (`package`, `destructive`, `system`, `network`, `shell`,
  `unknown`). Conformant Nodes reject shell requests before emitting an event, so
  `shell` only appears from implementations that still allow shells.
- Requests expire after 120 s (`expiresAt = createdAt + 120000`).

## Decision

```json
{ "id": "req-1", "op": "privilege.decision", "payload": {
    "id": "01JBV9C2Q7R9S1T3V5W7X9Y2Z4", "approve": true,
    "reason": "reviewed", "scope": "once"
} }
```

Requires `admin`. `scope` may be omitted and defaults to `once`; other values are
rejected with `conflict`. The first decision wins; a second decision for the same
id fails with `conflict`. A decision that arrives after `expiresAt` is rejected
with `conflict` and recorded as `expired`; it never executes.

The decision payload carries no approver. The Node records the authenticated
Admin session subject in the audit record; clients must not be trusted to supply it.

`privilege.list` (Admin only) returns `{ "requests": [ … ] }` with every pending
request, so an admin client that connected after the push can still approve them.

## Execution

On approval the Node executes the pinned `resolved` path once as root with a
sanitized environment and the exact approved argv. It must not re-resolve
`program` through `PATH`, follow a changed symlink, or start a shell. The result
(`exitCode`, `stdout`, `stderr`) returns to the requesting agent over the local
sudo socket; only the approval is a MAP event.

## Audit record

Every terminal outcome appends one JSON line to `dataDir/audit/privilege.log`
(mode 0600):

```json
{
  "ts": 1790000001000,
  "id": "01JBV9C2Q7R9S1T3V5W7X9Y2Z4",
  "result": "executed",
  "approver": "alice",
  "scope": "once",
  "reason": "reviewed",
  "detail": "exit=0",
  "request": { "…": "the exact request event payload" }
}
```

`result` is one of `executed`, `failed`, `denied`, `timeout`, `expired`.
`detail` is `exit=<code>` for executed commands and a short error for failed
spawns; `approver`, `scope` and `reason` are empty when no decision was made.
Cookie values, tokens, environment variables and their secrets are never part of
the record.

## Limits and default deny

- No Admin session, or no decision within 120 s, means denial. The reference sudo
  client exits 126; a Node must never default to allow.
- Pending requests are capped. The reference Node allows 8 concurrent pending
  requests and rate-limits each task (6/min) and the node globally (20/min);
  excess requests are rejected before an event is pushed.
- The local sudo socket accepts only the task uid or root. A failed peer-credential
  check rejects the connection instead of falling back to trust.
- Shells, relative program paths and executables outside trusted roots are
  rejected before approval. An executable must be an absolute path, owned by
  root, not group/other-writable, and every ancestor directory must be root-owned
  and non-writable by group/other.
- Root containers are a separate, explicitly approved creation decision; they do
  not grant host root.
