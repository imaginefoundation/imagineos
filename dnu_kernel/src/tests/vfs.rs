#![allow(dead_code)]
//! Integration test for the VFS facade over the RAMFS/overlay backend.
//!
//! The module list below mirrors what `kernel/src/main.rs` wires together, so the
//! routing exercised here is the real one. Keeping this compilable also guards
//! the RAMFS tests that used to fail to build when run standalone.

#[path = "../drivers/ata.rs"]
mod ata;
#[path = "../drivers/block.rs"]
mod block;
#[path = "../fs/dfs.rs"]
mod dfs;
#[path = "../fs/gpt.rs"]
mod gpt;
#[path = "../fs/ramfs.rs"]
mod ramfs;
#[path = "../fs/vfs.rs"]
mod vfs;

#[path = "../config.rs"]
mod config;

use vfs::{FileSystem, Vfs};

/// The VFS asks `config::keyboard_layout` while parsing nothing, but the RAMFS
/// tests rely on a mounted archive. Mount a tiny USTAR image with one file.
fn mounted_overlay() -> &'static [u8] {
    // One 512-byte header for `bin/hello` plus its 5-byte payload, padded.
    let mut archive = vec![0u8; 1024];
    archive[..5].copy_from_slice(b"hello");
    archive[345..348].copy_from_slice(b"bin");
    archive[100..108].copy_from_slice(b"0000755\0");
    archive[124..136].copy_from_slice(b"00000000005\0");
    archive[156] = b'0';
    archive[257..262].copy_from_slice(b"ustar");
    archive[512..517].copy_from_slice(b"world");
    Box::leak(archive.into_boxed_slice())
}

#[test]
fn routes_reads_writes_and_metadata_through_the_active_backend() {
    let archive = mounted_overlay();
    ramfs::mount(archive);

    let fs = Vfs::current();

    // Read-only view of the USTAR layer.
    assert!(fs.is_file("bin/hello"));
    assert!(!fs.is_directory("bin/hello"));
    assert_eq!(fs.read_static("bin/hello"), Some(&b"world"[..]));
    assert_eq!(fs.file_len("bin/hello"), Ok(5));

    let mut buffer = [0u8; 8];
    assert_eq!(fs.read_at("bin/hello", 0, &mut buffer), Ok(5));
    assert_eq!(&buffer[..5], b"world");

    // A missing path is reported uniformly through the VFS error type.
    assert_eq!(
        fs.read_at("bin/missing", 0, &mut buffer),
        Err(vfs::VfsError::NotFound)
    );

    // Mutations land in the overlay and are visible through the same handle.
    assert_eq!(fs.write_file_as("bin/greeting", b"ola mundo", 0, 0), Ok(()));
    assert!(fs.is_file("bin/greeting"));
    assert_eq!(fs.read_static("bin/greeting"), Some(&b"ola mundo"[..]));

    let metadata = fs.metadata("bin/greeting").expect("overlay metadata");
    assert_eq!(metadata.size, 9);
    assert!(!metadata.is_directory);

    // Directory listing merges the archive and the overlay.
    let mut listing = [0u8; 64];
    let length = fs
        .list_directory("bin", false, &mut listing)
        .expect("listing bin");
    let listing = &listing[..length];
    assert!(listing.windows(5).any(|w| w == b"hello"));
    assert!(listing.windows(8).any(|w| w == b"greeting"));

    assert_eq!(fs.remove("bin/greeting", false), Ok(()));
    assert!(!fs.is_file("bin/greeting"));
}

#[test]
fn reports_that_no_persistent_root_is_mounted() {
    // No DFS was mounted in this test process.
    assert!(!vfs::has_persistent_root());
}

#[test]
fn error_codes_follow_the_syscall_abi() {
    assert_eq!(vfs::VfsError::NotFound.to_errno(), -2);
    assert_eq!(vfs::VfsError::IsDirectory.to_errno(), -21);
    assert_eq!(vfs::VfsError::NoSpace.to_errno(), -28);
    assert_eq!(vfs::VfsError::Io.to_errno(), -5);
}
