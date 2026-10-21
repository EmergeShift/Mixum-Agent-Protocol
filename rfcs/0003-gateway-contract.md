# RFC 0003: Gateway routing and control ops

Status: draft (implementation tracked in mixumOS gateway).

## Envelope extension

`Envelope` gains an optional `node` string (1..=64 chars, no `:`):

- Direct node connections ignore it.
- A gateway routes the envelope to the named node, or returns
  `*.err` with `code=not_found` if the node is offline.
- Nodes registered through a gateway are addressed by the same name used in
  `gateway.register`.

## Ops (control plane, gateway-terminated)

| op | caller | payload | reply |
|---|---|---|---|
| `gateway.token.issue` | admin token (`Cap::Admin`) | `{kind: "user"\|"node", name}` | `{token}` |
| `gateway.token.revoke` | admin | `{jti}` | `{revoked: true}` |
| `gateway.node.list` | user/admin | `{}` | `{nodes: [name]}` |
| `gateway.pair.create` | user | `{}` | `{code, expiresIn}` |
| `gateway.pair.claim` | unauthenticated first message | `{code, deviceName}` | `{token, deviceId}` |
| `gateway.device.list` | user/admin | `{}` | `{devices: [{jti,name,sub,owner,iat}]}` |
| `gateway.device.revoke` | user (owner) / admin | `{jti}` | `{revoked: true}` |

Rules:

- Node tokens carry `sub=node:<name>`; a gateway MUST reject a registration
  whose token subject does not match the requested name.
- Device tokens are owned by the pairing user; list/revoke is owner-scoped,
  admin sees all.
- Pairing codes are single-use, 5 minutes, CSPRNG, and rate-limited per source
  IP (10 failures/minute).
- Errors use the normal `*.err` envelope; `code` is one of `bad_request`,
  `forbidden`, `not_found`, `conflict`, `internal`.

## Replica routing

Multiple gateway replicas share node metadata (k8s Lease backend) and forward
client traffic over an internal mesh. The mesh is cluster-internal and
authenticated with a shared token; TLS/mTLS is a separate follow-up.
