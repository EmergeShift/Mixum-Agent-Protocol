---
title: Privilege
description: Agent requests for one-shot root execution, human approval, and the audit boundary.
---

Stability: **Experimental** (mixumOS extension; not part of the v1 conformance set).

A Node does not give an agent a root shell. Agents run as an unprivileged task user;
a privileged operation is an explicit request that a human (or a CLI holding an
Admin token) approves or denies.

## Request event

The Node pushes to every connected session with the `admin` capability:

```json
{
  "op": "privilege.request",
  "payload": {
    "id": "01J...",
    "taskId": "01J...",
    "program": "npm",
    "args": ["install", "-g", "typescript"],
    "cwd": "/var/lib/mixum/work",
    "reason": "install the project toolchain",
    "risk": "package",
    "createdAt": 1790000000000,
    "expiresAt": 1790000000120
  }
}
```

`risk` is advisory (`package`, `destructive`, `system`, `network`, `shell`,
`unknown`). Clients must display the exact argv and the agent's task/reason.

## Decision

```json
{ "id": "req-1", "op": "privilege.decision", "payload": {
    "id": "01J...", "approve": true, "reason": "reviewed", "scope": "once"
} }
```

Requires `admin`. The first decision wins; a request expires after 120 s. On
approval the Node executes the exact argv once as root with a sanitized
environment, then returns stdout/stderr/exit to the requesting agent and writes an
audit record. `scope` is currently always `once`; no root shell and no arbitrary
subsequent commands are implied.

## Security

- Default deny when no Admin session is connected or the request times out.
- The request socket is local to the container and accepts only the task uid
  (or root); the sudo client is not setuid.
- Cookie values, tokens and environment secrets are not part of the audit record.
- Root containers are a separate, explicitly approved creation decision; they do
  not grant host root.
