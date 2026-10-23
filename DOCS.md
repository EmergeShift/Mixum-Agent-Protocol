# Documentation map

English is the canonical language. The spec site is built from
`site/src/content/docs/`; Chinese translations live under
`site/src/content/docs/zh/`. The English text is normative.

## Spec pages

| Page | Stability | Chinese |
|---|---|---|
| [Overview](site/src/content/docs/spec/overview.md) | draft | [中文](site/src/content/docs/zh/spec/overview.md) |
| [Architecture](site/src/content/docs/spec/architecture.md) | draft | [中文](site/src/content/docs/zh/spec/architecture.md) |
| [Envelope](site/src/content/docs/spec/envelope.md) | Stable | — |
| [Tasks](site/src/content/docs/spec/tasks.md) | Stable | — |
| [Snapshots](site/src/content/docs/spec/snapshots.md) | Stable | — |
| [Mobility](site/src/content/docs/spec/mobility.md) | Stable | — |
| [Display](site/src/content/docs/spec/display.md) | Stable | — |
| [Memory](site/src/content/docs/spec/memory.md) | Stable at v1.0; cross-node sync Experimental | [中文](site/src/content/docs/zh/spec/memory.md) |
| [VM / Host profile](site/src/content/docs/spec/vm-host.md) | Stable at v1.0 | — |
| [Agents bridge](site/src/content/docs/spec/agents-bridge.md) | Stable framing; driver trait Experimental | — |
| [Privilege](site/src/content/docs/spec/privilege.md) | **Experimental** | — |
| [Session](site/src/content/docs/spec/session.md) | Stable | — |
| [Transport security](site/src/content/docs/spec/transport-security.md) | Stable | — |
| [Versioning](site/src/content/docs/spec/versioning-governance.md) | process | — |

## Docs-first / experimental

- **Memory cross-node sync** is experimental; the local record APIs are implemented.
- **Privilege** is a mixumOS extension, not part of the v1 conformance set.
- **Agents bridge** driver trait is not frozen; the framing is stable.
- The VM/Host profile is stable at v1.0 but implementations may lag the text;
  check the reference implementations before relying on a field.

## Translation status

Only the overview, architecture and memory pages have Chinese counterparts.
The remaining pages need Chinese translations; until then the English page is
the only text. This gap is tracked here rather than hidden.
