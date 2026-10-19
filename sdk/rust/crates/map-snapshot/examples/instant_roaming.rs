//! Instant roaming: pack a tree, open the archive zero-copy, restore it and
//! report timings. Run with:
//!
//! ```text
//! cargo run -p map-snapshot --example instant_roaming
//! ```

use map_snapshot::{pack, MartArchive, PackOptions};
use std::fs;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let root = dir.path().join("world");
    fs::create_dir_all(root.join("project/src"))?;
    fs::write(root.join("project/README.md"), "# demo\n")?;
    let blob: Vec<u8> = (0..4 * 1024 * 1024).map(|i| (i % 251) as u8).collect();
    fs::write(root.join("project/src/blob.bin"), &blob)?;

    let mart = dir.path().join("world.mart");
    let start = Instant::now();
    let stats = pack(&root, &mart, &PackOptions::default())?;
    let pack_ms = start.elapsed().as_secs_f64() * 1000.0;

    let start = Instant::now();
    let archive = MartArchive::open(&mart)?;
    let open_ms = start.elapsed().as_secs_f64() * 1000.0;

    let start = Instant::now();
    let restored = dir.path().join("restored");
    let restored_stats = archive.restore(&restored)?;
    let restore_ms = start.elapsed().as_secs_f64() * 1000.0;

    assert_eq!(
        fs::read(restored.join("project/src/blob.bin"))?,
        blob,
        "restored bytes differ"
    );
    archive.verify()?;

    println!(
        "packed {} files / {} chunks in {:.1} ms ({:.1} MiB); zero-copy open {:.3} ms; restore {} files in {:.1} ms",
        stats.files,
        stats.chunks_stored,
        pack_ms,
        stats.bytes as f64 / (1024.0 * 1024.0),
        open_ms,
        restored_stats.files,
        restore_ms,
    );
    Ok(())
}
