use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

// On Linux, `fork`/`posix_spawn` duplicates the process's open file
// descriptors until the child execs. When one test thread still holds a
// writable descriptor for a helper script, a concurrent spawn in another
// thread can leak that descriptor into its child, and the script's own exec
// then fails with `ETXTBSY`. Tests that create and run temporary helper
// executables hold this lock for their whole body so that no write and no
// spawn can overlap. It affects test scheduling only, never production code.
static PROCESS_SPAWN_LOCK: Mutex<()> = Mutex::new(());

pub(crate) fn process_spawn_guard() -> std::sync::MutexGuard<'static, ()> {
    PROCESS_SPAWN_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

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
