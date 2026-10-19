//! WASM bindings for `.mart` manifests.
//!
//! The browser cannot mmap a file, so the wasm side only exposes the format
//! contract and pure helpers; a caller that already has the manifest JSON (for
//! example from a Node bridge) can render it without pulling the full archive
//! reader into the bundle.

use wasm_bindgen::prelude::*;

/// `.mart` format version supported by this build.
#[wasm_bindgen]
pub fn mart_format_version() -> u32 {
    map_snapshot::FORMAT_VERSION
}

/// Human-readable format name, handy for capability probes.
#[wasm_bindgen]
pub fn mart_format_name() -> String {
    "M-ART/.mart".to_owned()
}

/// Validate a relative manifest path (the traversal rule is shared with the
/// native reader). Returns `true` when the path is safe to restore.
#[wasm_bindgen]
pub fn mart_path_is_safe(path: &str) -> bool {
    map_snapshot::validate_rel_path(path).is_ok()
}
