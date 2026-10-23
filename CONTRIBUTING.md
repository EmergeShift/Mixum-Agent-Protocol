# Contributing to MAP

Thank you for helping make MAP a protocol the whole agent ecosystem can rely on.

## The golden rule

**The spec is the product.** Code, SDKs and sites are conveniences. Every change to observable wire behavior must land as a spec change first, with:

1. A spec chapter diff (`site/src/content/docs/spec/…`).
2. A JSON Schema diff in `schemas/` if a control-plane payload changed.
3. A conformance vector in `vectors/` for new or tricky behavior.
4. A minor or major version decision (see [Versioning & Governance](site/src/content/docs/spec/versioning-governance)).

## RFCs

Anything that changes semantics, adds an op, or adds a capability requires an RFC in `rfcs/` (copy `rfcs/0000-template.md`). Lifecycle: `Idea → Draft → Last Call → Accepted → Implemented`. RFCs are written in English; translation follows acceptance.

## Language policy

- English (`en`) is the **normative** language. In case of conflict, the English text wins.
- Chinese (`zh`) translations cover the overview and key concept pages; community help expanding coverage is welcome. Translations must not invent semantics — open an issue instead.

## Development

CI runs four jobs on every push to `main` and every pull request. Run the same
checks locally before opening a PR:

```bash
# schemas + vectors
pip install jsonschema
python scripts/validate_artifacts.py

# Rust SDK
cd sdk/rust
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# TypeScript SDK
cd sdk/typescript
npm ci
npm run check
npm test

# docs site
cd site
npm ci
npm run dev      # local preview at localhost:4321
npm run build    # production build (also our link/typos sanity pass)
```

Schemas are plain JSON Schema draft 2020-12. Every schema change needs a vector
under `vectors/`; `scripts/validate_artifacts.py` rejects valid cases and checks
that invalid cases are actually invalid (cases marked `"semantic": true` are
enforced by dispatch/state machine instead of the schema).

SDKs in `sdk/` are skeletons until v1.0; PRs there should mirror the spec and add tests, not invent behavior.

## Commit style

One logical change per commit, imperative subject, spec rationale in the body. Example:

```
spec(tasks): define task.reply input channel

Agents need a standard "ask the human" primitive...
```
