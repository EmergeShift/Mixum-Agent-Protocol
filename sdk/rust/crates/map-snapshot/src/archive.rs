//! Read, verify, restore and diff `.mart` archives.
//!
//! Opening memory-maps the file, validates the header and the archive digest,
//! then interprets the rkyv root in place. Chunk payloads are slices of the
//! mapping, so restore writes straight from page cache without copying the
//! snapshot into a second in-memory representation.

use crate::error::{MartError, Result};
use crate::format::{sha256_hex, validate_rel_path, MartHeader, HEADER_LEN};
use crate::manifest::{
    ArchivedMartChunk, ArchivedMartFile, ArchivedMartManifest, MartManifest, MANIFEST_VERSION,
};
use memmap2::Mmap;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct MartArchive {
    mmap: Mmap,
    header: MartHeader,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyReport {
    pub version: u32,
    pub files: u64,
    pub dirs: u64,
    pub chunks: u64,
    pub bytes: u64,
    pub ok: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreStats {
    pub files: u64,
    pub dirs: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffKind {
    Added,
    Removed,
    Modified,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffEntry {
    pub path: String,
    pub kind: DiffKind,
    pub from_size: u64,
    pub to_size: u64,
    pub chunks_added: u64,
    pub chunks_removed: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffReport {
    pub files_added: Vec<String>,
    pub files_removed: Vec<String>,
    pub files_modified: Vec<DiffEntry>,
}

impl MartArchive {
    /// Memory-map and validate an archive. Integrity is checked before rkyv is
    /// allowed to interpret any bytes.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };
        let header = MartHeader::decode(&mmap)?;
        header.validate(mmap.len() as u64)?;
        let archive =
            &mmap[header.archive_off as usize..(header.archive_off + header.archive_len) as usize];
        if sha256_bytes(archive) != header.archive_sha256 {
            return Err(MartError::Corrupt("archive digest mismatch".into()));
        }
        let this = Self { mmap, header };
        {
            let manifest = this.manifest();
            if manifest.version != MANIFEST_VERSION {
                return Err(MartError::Unsupported(manifest.version));
            }
            this.check_manifest()?;
        }
        Ok(this)
    }

    pub fn header(&self) -> &MartHeader {
        &self.header
    }

    pub fn manifest(&self) -> &ArchivedMartManifest {
        let bytes = self.archive_bytes();
        // SAFETY: `open` validated the header, the full archive digest and the
        // structural invariants below before exposing this type.
        unsafe { rkyv::archived_root::<MartManifest>(bytes) }
    }

    fn archive_bytes(&self) -> &[u8] {
        let start = self.header.archive_off as usize;
        let end = start + self.header.archive_len as usize;
        &self.mmap[start..end]
    }

    /// Validate chunk bounds, file size sums and manifest paths.
    pub fn check_manifest(&self) -> Result<()> {
        let manifest = self.manifest();
        if manifest.chunks.len() > u32::MAX as usize {
            return Err(MartError::Corrupt("too many chunks".into()));
        }
        for dir in manifest.dirs.iter() {
            validate_rel_path(dir.path.as_str())?;
        }
        for file in manifest.files.iter() {
            validate_rel_path(file.path.as_str())?;
            let mut total = 0u64;
            for index in file.chunks.iter() {
                let chunk = manifest.chunks.get(*index as usize).ok_or_else(|| {
                    MartError::Corrupt(format!("chunk index {index} out of range"))
                })?;
                total += chunk.len;
            }
            if file.symlink.is_none() && total != file.size {
                return Err(MartError::Corrupt(format!(
                    "{}: chunks sum to {total}, manifest size {}",
                    file.path.as_str(),
                    file.size
                )));
            }
        }
        let chunk_region_end = self.header.chunk_off + self.header.chunk_len;
        for (index, chunk) in manifest.chunks.iter().enumerate() {
            let start = chunk.offset;
            let end = start + chunk.len;
            if end > chunk_region_end {
                return Err(MartError::Corrupt(format!(
                    "chunk {index} ({}) lies outside the chunk region",
                    chunk.hash.as_str()
                )));
            }
            if start < HEADER_LEN {
                return Err(MartError::Corrupt(format!(
                    "chunk {index} overlaps the header"
                )));
            }
        }
        Ok(())
    }

    pub fn chunk_bytes(&self, index: usize) -> Result<&[u8]> {
        let manifest = self.manifest();
        let chunk: &ArchivedMartChunk = manifest
            .chunks
            .get(index)
            .ok_or_else(|| MartError::Corrupt(format!("chunk index {index} out of range")))?;
        let start = chunk.offset as usize;
        let end = start
            .checked_add(chunk.len as usize)
            .ok_or_else(|| MartError::Corrupt("chunk length overflow".into()))?;
        self.mmap
            .get(start..end)
            .ok_or_else(|| MartError::Corrupt("chunk slice out of bounds".into()))
    }

    /// Recompute every chunk digest and verify structural invariants.
    pub fn verify(&self) -> Result<VerifyReport> {
        let manifest = self.manifest();
        let mut bytes = 0u64;
        for (index, chunk) in manifest.chunks.iter().enumerate() {
            let payload = self.chunk_bytes(index)?;
            if sha256_hex(payload) != chunk.hash.as_str() {
                return Err(MartError::Corrupt(format!(
                    "chunk {index} digest mismatch ({}), archive is damaged",
                    chunk.hash.as_str()
                )));
            }
            bytes += payload.len() as u64;
        }
        self.check_manifest()?;
        Ok(VerifyReport {
            version: manifest.version,
            files: manifest.files.len() as u64,
            dirs: manifest.dirs.len() as u64,
            chunks: manifest.chunks.len() as u64,
            bytes,
            ok: true,
        })
    }

    /// Restore the archived tree into `dest` (created if missing).
    pub fn restore(&self, dest: impl AsRef<Path>) -> Result<RestoreStats> {
        let dest = dest.as_ref();
        fs::create_dir_all(dest)?;
        let manifest = self.manifest();
        let mut stats = RestoreStats::default();

        for dir in manifest.dirs.iter() {
            let relative = validate_rel_path(dir.path.as_str())?;
            let target = dest.join(relative);
            fs::create_dir_all(&target)?;
            set_mode(&target, dir.mode)?;
            stats.dirs += 1;
        }

        for file in manifest.files.iter() {
            let relative = validate_rel_path(file.path.as_str())?;
            let target = dest.join(relative);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            if let Some(link) = file.symlink.as_ref() {
                create_symlink(link.as_str(), &target)?;
                stats.files += 1;
                continue;
            }
            let mut output = File::create(&target)?;
            for index in file.chunks.iter() {
                let payload = self.chunk_bytes(*index as usize)?;
                output.write_all(payload)?;
                stats.bytes += payload.len() as u64;
            }
            output.flush()?;
            drop(output);
            set_mode(&target, file.mode)?;
            stats.files += 1;
        }
        Ok(stats)
    }

    /// MAP snapshot-manifest JSON (`schemas/snapshot-manifest.schema.json`).
    pub fn to_snapshot_json(&self) -> serde_json::Value {
        let manifest = self.manifest();
        let files: Vec<serde_json::Value> = manifest
            .files
            .iter()
            .map(|file| file_json(manifest, file))
            .collect();
        let dirs: Vec<serde_json::Value> = manifest
            .dirs
            .iter()
            .map(|dir| serde_json::json!({ "path": dir.path.as_str(), "mode": dir.mode }))
            .collect();
        serde_json::json!({ "version": 1, "files": files, "dirs": dirs })
    }

    /// Full inspection document: archive metadata + MAP manifest.
    pub fn inspect_json(&self) -> serde_json::Value {
        let manifest = self.manifest();
        let mut chunk_bytes = 0u64;
        for (index, _) in manifest.chunks.iter().enumerate() {
            chunk_bytes += self.chunk_bytes(index).map(|b| b.len() as u64).unwrap_or(0);
        }
        serde_json::json!({
            "mart": {
                "version": self.header.version,
                "createdAt": self.header.created_at,
                "source": manifest.source.as_str(),
                "fileBytes": self.header.file_len,
                "chunkBytes": chunk_bytes,
                "chunks": manifest.chunks.len(),
                "files": manifest.files.len(),
                "dirs": manifest.dirs.len(),
            },
            "manifest": self.to_snapshot_json(),
        })
    }

    /// Structural diff against another archive. Compare by path, mode, size and
    /// chunk digests (content).
    pub fn diff(&self, other: &MartArchive) -> DiffReport {
        let from = fingerprints(self.manifest());
        let to = fingerprints(other.manifest());
        let mut report = DiffReport::default();
        for (path, old) in from.iter() {
            match to.get(path) {
                None => report.files_removed.push(path.clone()),
                Some(new) if new != old => {
                    let old_chunks: BTreeSet<&String> = old.chunks.iter().collect();
                    let new_chunks: BTreeSet<&String> = new.chunks.iter().collect();
                    report.files_modified.push(DiffEntry {
                        path: path.clone(),
                        kind: DiffKind::Modified,
                        from_size: old.size,
                        to_size: new.size,
                        chunks_added: new_chunks.difference(&old_chunks).count() as u64,
                        chunks_removed: old_chunks.difference(&new_chunks).count() as u64,
                    });
                }
                Some(_) => {}
            }
        }
        for path in to.keys() {
            if !from.contains_key(path) {
                report.files_added.push(path.clone());
            }
        }
        report
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Fingerprint {
    mode: u32,
    size: u64,
    symlink: Option<String>,
    chunks: Vec<String>,
}

fn fingerprints(manifest: &ArchivedMartManifest) -> BTreeMap<String, Fingerprint> {
    let mut map = BTreeMap::new();
    for file in manifest.files.iter() {
        let chunks = file
            .chunks
            .iter()
            .map(|index| {
                manifest
                    .chunks
                    .get(*index as usize)
                    .map(|chunk| chunk.hash.as_str().to_owned())
                    .unwrap_or_default()
            })
            .collect();
        map.insert(
            file.path.as_str().to_owned(),
            Fingerprint {
                mode: file.mode,
                size: file.size,
                symlink: file.symlink.as_ref().map(|s| s.as_str().to_owned()),
                chunks,
            },
        );
    }
    map
}

fn file_json(manifest: &ArchivedMartManifest, file: &ArchivedMartFile) -> serde_json::Value {
    let chunks: Vec<String> = file
        .chunks
        .iter()
        .filter_map(|index| {
            manifest
                .chunks
                .get(*index as usize)
                .map(|chunk| chunk.hash.as_str().to_owned())
        })
        .collect();
    let mut value = serde_json::json!({
        "path": file.path.as_str(),
        "mode": file.mode,
        "size": file.size,
        "chunks": chunks,
    });
    if let Some(link) = file.symlink.as_ref() {
        value["symlink"] = serde_json::json!(link.as_str());
    }
    value
}

fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let permissions = fs::Permissions::from_mode(mode);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn create_symlink(target: &str, link: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, link)?;
    Ok(())
}

#[cfg(not(unix))]
fn create_symlink(_target: &str, link: &Path) -> Result<()> {
    Err(MartError::Corrupt(format!(
        "symlink restore is not supported on this platform: {}",
        link.display()
    )))
}

/// Re-export for callers that only need to know it is a path inside `dest`.
pub fn safe_join(dest: &Path, relative: &str) -> Result<PathBuf> {
    Ok(dest.join(validate_rel_path(relative)?))
}
