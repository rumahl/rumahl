use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

pub(crate) fn unique_test_root(kind: char) -> PathBuf {
    loop {
        let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("r{kind}{:x}{sequence:x}", std::process::id()));
        match fs::create_dir(&root) {
            Ok(()) => return root,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("failed to reserve temporary test directory: {error}"),
        }
    }
}
