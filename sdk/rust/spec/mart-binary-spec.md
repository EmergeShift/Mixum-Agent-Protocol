# `.mart` binary format (M-ART snapshot)

Status: **v1 draft**, implemented by `crates/map-snapshot`.

A `.mart` file is a single-file, memory-mappable snapshot of a directory tree.
It is the on-disk form behind MAP's snapshot profile, not a wire protocol: the
control plane still speaks `snap.*` envelopes and the normative snapshot
manifest stays [`schemas/snapshot-manifest.schema.json`](../../../schemas/snapshot-manifest.schema.json).

## Goals

- **Zero-copy open.** Opening validates a digest, then interprets an rkyv
  archive in place; no JSON parse and no full-tree allocation.
- **Content addressing.** Every chunk is addressed by the plaintext SHA-256 of
  its bytes, so dedupe works across files, snapshots and nodes.
- **Self-contained.** One file carries the manifest and the payloads; moving a
  snapshot is a file copy, not a directory dance.
- **Safe restore.** Paths are relative and traversal-free; chunk digests are
  re-verified on demand.

Out of scope for v1: compression, encryption, CRIU memory images. They are
orthogonal layers; the chunk table already carries `raw_len` for a future
compressed variant.

## Layout

All integers are little-endian.

```text
offset 0        magic "MART" (4 bytes)
offset 4        format version, u32 (currently 1)
offset 8        fixed 128-byte header
offset 128      chunk payload region (raw bytes, content-addressed)
...             zero padding to a 16-byte boundary
...             rkyv archive of MartManifest
EOF
```

The 16-byte alignment is required because rkyv interprets the archive in place;
the mmap base is page-aligned, so aligning the file offset is sufficient.

### Header (128 bytes)

| Offset | Size | Field | Meaning |
|---|---|---|---|
| 0 | 4 | magic | `MART` |
| 4 | 4 | version | `.mart` format version |
| 8 | 8 | archive_off | absolute offset of the rkyv archive |
| 16 | 8 | archive_len | archive length in bytes |
| 24 | 8 | chunk_off | always 128 in v1 |
| 32 | 8 | chunk_len | payload region length (includes alignment padding) |
| 40 | 8 | file_len | total file length |
| 48 | 8 | created_at | Unix milliseconds |
| 56 | 8 | flags | reserved, must be 0 in v1 |
| 64 | 32 | archive_sha256 | SHA-256 of the archive bytes |
| 96 | 32 | reserved | zero |

A reader rejects the file when the magic is wrong, the version is unknown,
regions overflow `file_len`, or `file_len` differs from the actual file size.

## Manifest

The archive root is `MartManifest` (rkyv). Its logical shape:

```text
MartManifest
  version: u32                 # manifest schema version (1)
  created_at: i64
  source: string               # free-form origin label
  files: [MartFile]
  dirs: [MartDir]
  chunks: [MartChunk]
```

```text
MartFile
  path: string                 # relative, '/'-separated, no "..", no leading '/'
  mode: u32                    # octal permission bits (420 = 0o644)
  size: u64
  symlink: option<string>      # target when the entry is a symlink
  chunks: [u32]                # indices into MartManifest.chunks

MartDir
  path: string
  mode: u32

MartChunk
  hash: string                 # lowercase hex SHA-256 of the plaintext bytes
  offset: u64                  # absolute file offset of the payload
  len: u64                     # stored length
  raw_len: u64                 # uncompressed length (equals len in v1)
```

Every non-symlink file's chunk lengths sum to `size`. Every chunk index is in
range. Every chunk lies inside `[128, file_len)`. A reader validates these
invariants before serving restore.

## Manifest JSON view

`MartArchive::to_snapshot_json` converts the archive to the normative snapshot
manifest shape:

```json
{
  "version": 1,
  "files": [
    { "path": "a.txt", "mode": 420, "size": 11, "chunks": ["<sha256>"] }
  ],
  "dirs": [{ "path": "sub", "mode": 493 }]
}
```

This is what `mart inspect --manifest-only` prints, so an archive can be
checked against `schemas/snapshot-manifest.schema.json` without a second
converter.

## Chunking and dedupe

- Default chunk size: 1 MiB (`DEFAULT_CHUNK_SIZE`).
- Within one archive, identical chunks are stored once and referenced by index.
- With `PackOptions::base = Some(old.mart)`, chunks with a matching hash are
  copied from the base archive instead of the source tree. The result is still
  self-contained; only bytes are reused, not offsets.
- A future revision may reference an external base archive instead of copying;
  that requires a multi-file address space and is intentionally deferred.

## Integrity

- `archive_sha256` protects the manifest during `open`; rkyv never interprets
  bytes that fail the digest.
- `verify()` recomputes every chunk SHA-256 and re-checks structural
  invariants. Restore does **not** re-hash by default (it would double I/O);
  run `mart verify` before trusting an archive from an untrusted channel.

## Restore safety

- `path` must be relative, contain no `..`, root or prefix components.
- `restore` creates parent directories and writes chunks sequentially; a later
  version may stream chunks concurrently.
- Symlinks are restored with the recorded target on Unix. The current Windows
  implementation returns an error for symlink entries rather than silently
  materializing a copy.
- Modes are applied after content is written (Unix).

## Versioning

- `MartHeader.version` covers the file layout; `MartManifest.version` covers
  the manifest schema. They evolve independently.
- Additive manifest fields are minor revisions; a reader must reject unknown
  major versions, not guess.
- The archive is **not** guaranteed portable across CPU endianness until a
  little-endian archive feature is enabled; v1 assumes the native endianness of
  the writing machine and is intended for same-architecture roaming first.

## Tooling

```text
mart pack     <dir> -o snap.mart [--base old.mart] [--chunk-size N]
mart inspect  snap.mart [--manifest-only]
mart verify   snap.mart
mart restore  snap.mart <dir>
mart diff     old.mart new.mart
```

`cargo run -p map-snapshot --example instant_roaming` demonstrates pack →
zero-copy open → restore timings.
