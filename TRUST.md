# The Mixum Trust Commitments

> This document is a public commitment, not marketing. It states what the Mixum
> ecosystem will never do, what it collects when you explicitly allow it, and
> how each claim can be independently verified. Changes to it require a
> Process RFC and are announced in release notes.

Infrastructure has one job: to be dependable and to stay out of the way. MAP
exists to make agent environments interoperable — which only matters if every
participant, including competing vendors and skeptical users, can verify that
adopting it does not create a new surveillance or lock-in surface. This page is
the contract behind that claim.

**Scope:** these commitments bind the Mixum project's repositories
([mixumOS](https://github.com/ashreo/mixumOS), Mixum Agent Protocol (this repo),
[MixumART](https://github.com/ashreo/mixum-Agent-Env)) and any service Mixum
operates. Third parties building on MAP set their own policies; the conformance
program (see [Versioning & Governance](site/src/content/docs/spec/versioning-governance.md))
is how they demonstrate comparable behavior.

## The invariants

### I-1 — The protocol never phones home

MAP — the specification, the schemas, the vectors, and the SDKs — performs no
telemetry, no update checks, no phone-home. An SDK or conformant Node making
any network connection beyond the MAP sessions it was explicitly given is a
bug: report it, and it will be treated as a security-adjacent defect.

### I-2 — Credentials never transit Mixum infrastructure

Agent platform credentials (API keys, OAuth sessions) travel from the user or
their Host directly to the Node they designate. Mixum-operated services never
see, store, proxy, or broker them. There is no "Mixum account" requirement for
routing your own credentials to your own machines.

### I-3 — Content stays in the user's environment

Workspace files, task instructions, window contents, and agent memory records
leave a user's environment only toward endpoints that user explicitly
configured (their own nodes, their own remotes). They are never sent to Mixum.

### I-4 — Telemetry, if any, lives in the app and is off by default

The only place Mixum ships analytics is MixumART, the desktop application:

- **Off by default.** One click enables, one click disables, and the choice
  persists.
- **Aggregate and anonymous only:** rotating daily identifiers derived from a
  random install id, app version, OS, runtime type, and feature counters.
  Never content, file paths, window titles, account data, or raw IP retention.
- **Methodology public; collection endpoint open source.**

The protocol, mixumOS distribution default builds, and the SDKs contain no
analytics. Cloud operators may enable metrics for their own fleets — that is
their configuration for their users, not ours.

### I-5 — The gateway is open source and self-hostable, forever

When the optional cloud gateway ships, it is open source. Pointing MixumART at
your own gateway forfeits no protocol capability. Mixum's hosted gateway is a
convenience, never a toll booth.

### I-6 — No feature is gated on an account with Mixum

Everything in the spec is usable without registering anywhere. Paid offerings
sit above the protocol — hosting, support, enterprise services — never inside
it.

### I-7 — Open source, in perpetuity, without land-grabs

All Mixum repositories are Apache-2.0. Contributions use DCO (Signed-off-by),
not a CLA. There is no kill switch: if the project or the company disappears,
the last release keeps working. We will not relicense the protocol or the
reference implementations under terms that break this document.

### I-8 — Portability is a right

Your state is content-addressed and exportable: snapshots move between nodes,
memory records export to open formats. Deliberate lock-in — format friction,
degraded exports, artificial barriers to leaving — is a violation of this
document, not a product decision.

### I-9 — We route credentials; we do not facilitate abuse

Ecosystem tooling carries a user's **own** credentials for their **own**
accounts to their **own** environments. We do not build account-sharing,
usage-metering-evasion, or anti-automation-circumvention features, and we
expect platform terms of service to prevail where they apply.

## How to verify

- **I-1 / I-4** — run a conformant Node on an isolated network: it needs no
  outbound connection beyond its listeners. The conformance runner includes an
  egress check.
- **I-2** — the credential path is one hop, user/Host → Node, covered by the
  vault vectors; no Mixum service sits in between.
- **I-4** — read the collector source, and watch MART's network log with
  telemetry disabled: silence.
- **I-5 / I-7** — the source is the proof.

## Changing this document

Amendments require a Process RFC and are called out in release notes. Knowingly
shipping against a commitment here is a governance incident under
[GOVERNANCE.md](GOVERNANCE.md).

---

*Infrastructure outlives companies. Docker the company stumbled; OCI the
standard did not. The point of MAP is to be the standard.*
