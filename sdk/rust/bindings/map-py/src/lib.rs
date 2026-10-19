//! Python bindings for `.mart` snapshots (PyO3).
//!
//! Build (from `sdk/rust`):
//! ```text
//! cargo build --manifest-path bindings/map-py/Cargo.toml --release
//! ```
//! Then point `PYTHONPATH` at `target/release` and `import map_py`.

use pyo3::prelude::*;
use pyo3::types::PyModule;

fn to_py_err(error: map_snapshot::MartError) -> PyErr {
    pyo3::exceptions::PyRuntimeError::new_err(error.to_string())
}

/// Return the `.mart` format version supported by this build.
#[pyfunction]
fn format_version() -> u32 {
    map_snapshot::FORMAT_VERSION
}

/// Inspect an archive and return the JSON document (metadata + MAP manifest).
#[pyfunction]
fn inspect(path: &str) -> PyResult<String> {
    let archive = map_snapshot::MartArchive::open(path).map_err(to_py_err)?;
    serde_json::to_string(&archive.inspect_json())
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
}

/// Verify every chunk digest; returns the report as JSON.
#[pyfunction]
fn verify(path: &str) -> PyResult<String> {
    let archive = map_snapshot::MartArchive::open(path).map_err(to_py_err)?;
    let report = archive.verify().map_err(to_py_err)?;
    serde_json::to_string(&report)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
}

/// Pack a directory into a `.mart`; returns pack statistics as JSON.
#[pyfunction]
#[pyo3(signature = (root, out, base=None))]
fn pack(root: &str, out: &str, base: Option<String>) -> PyResult<String> {
    let options = map_snapshot::PackOptions {
        base: base.map(std::path::PathBuf::from),
        ..map_snapshot::PackOptions::default()
    };
    let stats = map_snapshot::pack(root, out, &options).map_err(to_py_err)?;
    serde_json::to_string(&stats)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
}

/// Restore an archive into a directory; returns restore statistics as JSON.
#[pyfunction]
fn restore(path: &str, dest: &str) -> PyResult<String> {
    let archive = map_snapshot::MartArchive::open(path).map_err(to_py_err)?;
    let stats = archive.restore(dest).map_err(to_py_err)?;
    serde_json::to_string(&stats)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
}

#[pymodule]
fn map_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(format_version, m)?)?;
    m.add_function(wrap_pyfunction!(inspect, m)?)?;
    m.add_function(wrap_pyfunction!(verify, m)?)?;
    m.add_function(wrap_pyfunction!(pack, m)?)?;
    m.add_function(wrap_pyfunction!(restore, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
