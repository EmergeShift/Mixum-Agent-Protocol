# Third-party notices

MAP is Apache-2.0. The SDKs depend on third-party crates and npm packages; see
`sdk/rust/Cargo.lock` and `sdk/typescript/package-lock.json`.

Brand assets come from the EmergeShift identity kit (Apache-2.0); the wordmark
uses Inter SemiBold outlines (SIL OFL 1.1). Rust dependencies
are MIT/Apache-2.0/BSD/ISC; `webpki-roots` uses CDLA-Permissive-2.0 (data). Each dependency
keeps its own license. The documentation site is built with Astro/Starlight (MIT). Its build-time image
pipeline pulls `sharp` (Apache-2.0) and prebuilt `libvips` (LGPL-3.0-or-later);
they are not shipped in the static output.
