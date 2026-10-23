# Security Policy

## Reporting a vulnerability

Please **do not open a public issue** for security problems.

Use GitHub's private vulnerability reporting on this repository
(**Security → Report a vulnerability**). If that is unavailable, contact the
maintainers through the organization profile and ask for a private channel.

Include:

- affected repository/version/commit;
- reproduction steps or a proof of concept;
- impact assessment (what an attacker can read, write, or execute);
- any suggested fix or mitigation.

We will acknowledge reports as soon as possible and coordinate disclosure.
Please give us a reasonable window to ship a fix before publishing details.

## Supported versions

Security fixes target the latest `main` and the latest tagged release.
Older releases may receive fixes at the maintainers' discretion.

## Scope

In scope: the MAP wire protocol, schemas, SDKs, conformance vectors, and the
reference implementations in the mixumOS / MixumART repositories.

Out of scope: vulnerabilities in third-party runtimes, kernels, container
engines, or agents that MAP merely connects to, unless the issue is caused by
MAP's own contract or implementation.
