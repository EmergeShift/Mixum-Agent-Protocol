# Conformance vectors

Structural, language-neutral conformance vectors. A conformant implementation must accept every case under `valid` and reject every case under `invalid` with exactly the stated error code (where applicable).

| Directory | Locks | Spec chapter |
|---|---|---|
| `envelope/` | Envelope validation, reply convention, error usage | [Envelope & Framing](../site/src/content/docs/spec/envelope.md) |
| `snapshot-tree/` | Tree semantics: restore-inplace parallel timelines, fork numbering, leaf-only delete, remote roots | [Snapshots & Time Travel](../site/src/content/docs/spec/snapshots.md) |
| `bridge-session/` | A complete MAP-Bridge session transcript | [Agents & Bridge](../site/src/content/docs/spec/agents-bridge.md) |
| `privilege/` | Approval/deny/timeout/expiry flow, executable pinning and audit records | [Privilege](../site/src/content/docs/spec/privilege.md) |
| `clipboard/` | Clipboard set/changed payloads and the legacy pull reply | [Display & Interaction](../site/src/content/docs/spec/display.md) |
| `tokens/` | Signed `map1.` token pairs, claim validation, expiry and signature failure | [Transport & Security](../site/src/content/docs/spec/transport-security.md) |

Rules:

- ULIDs and timestamps in vectors are **examples**; implementations must accept any conformant value.
- `invalid` cases marked `"semantic": true` are enforced by dispatch or the state machine, not by the JSON Schema; the artifact check below only asserts schema rejection for unmarked cases.
- Vector files are data, not tests: each repo wires them into its own test harness (JSON Schema validation + state-machine execution).
- Repository check: `python scripts/validate_artifacts.py` (requires `jsonschema`) validates every schema and vector. CI runs the same script.
- Changes to vectors that tighten behavior require an RFC ([see governance](../GOVERNANCE.md)).

`tokens/tokens.json` is generated, not hand-written:

```
cd sdk/rust
cargo run -p map-core --features token-verify --example token_vectors > ../../vectors/tokens/tokens.json
```

The tokens are signed with a fixed test key (published in the file, test-only). ed25519
signatures are deterministic, so regenerating must reproduce the file byte for byte; if it
does not, the claims layout changed and the diff needs review. The public key in the file is
the pin: `map-core`'s test suite verifies every token against it, and a token that was
re-signed with another key fails there.
