# Security Policy

MAP is a wire protocol plus SDKs. A bug in parsing or token verification can
affect every implementation.

## Reporting a vulnerability

Please **do not open a public issue**.

- Email: **security@emergeshift.com**
- Or use GitHub private vulnerability reporting: **Security** tab → **Report a vulnerability**.

Include the affected implementation, version or commit, a minimal transcript or
payload, and impact.

## Scope

- the specification, JSON Schemas and conformance vectors
- the Rust and TypeScript SDKs
- token verification, framing and control-plane parsing

## Known limitations

- The token and crypto conformance vectors are not final before v1.0.
- SDKs are skeletons; pin a commit rather than relying on a released package.
