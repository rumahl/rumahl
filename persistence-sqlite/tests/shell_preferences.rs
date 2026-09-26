use rumahl_core::{
    BrowserProfileId, PreferenceScope::*, ShellMode::*, ShellPreferencesError,
    ShellPreferencesRepository, UserId,
};
use rumahl_persistence_sqlite::SqliteShellPreferences;
fn profile(n: u32) -> BrowserProfileId {
    BrowserProfileId::parse(&format!("00000000-0000-4000-8000-{n:012x}")).unwrap()
}
#[test]
fn scopes_inherit_without_leaking_users_and_survive_restart() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("preferences.sqlite");
    let store = SqliteShellPreferences::open(&path).unwrap();
    let alice = UserId::new();
    let bob = UserId::new();
    let initial = store.load(alice, profile(1)).unwrap();
    assert_eq!(initial.effective_mode(), Desktop);
    store
        .save(alice, profile(1), 0, User, Some(Launcher))
        .unwrap();
    assert_eq!(
        store.load(alice, profile(2)).unwrap().effective_mode(),
        Launcher
    );
    store
        .save(alice, profile(1), 1, Device, Some(Desktop))
        .unwrap();
    assert_eq!(
        store.load(alice, profile(1)).unwrap().effective_mode(),
        Desktop
    );
    assert_eq!(
        store.load(alice, profile(2)).unwrap().effective_mode(),
        Launcher
    );
    assert_eq!(store.load(bob, profile(1)).unwrap().revision, 0);
    assert_eq!(
        store.save(alice, profile(2), 1, User, Some(Desktop)),
        Err(ShellPreferencesError::Conflict)
    );
    drop(store);
    let store = SqliteShellPreferences::open(path).unwrap();
    assert_eq!(
        store.load(alice, profile(1)).unwrap().device_mode,
        Some(Desktop)
    );
    let inherited = store.save(alice, profile(1), 2, Device, None).unwrap();
    assert_eq!(inherited.device_mode, None);
    assert_eq!(inherited.effective_mode(), Launcher);
    store
        .save(alice, profile(2), 3, User, Some(Desktop))
        .unwrap();
    assert_eq!(
        store.load(alice, profile(1)).unwrap().effective_mode(),
        Desktop
    );
}
#[test]
fn concurrent_writers_cannot_silently_overwrite_and_profiles_are_bounded() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("preferences.sqlite");
    let store = SqliteShellPreferences::open(&path).unwrap();
    let user = UserId::new();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let threads: Vec<_> = [Desktop, Launcher]
        .into_iter()
        .map(|mode| {
            let store = SqliteShellPreferences::open(&path).unwrap();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                store.save(user, profile(1), 0, User, Some(mode))
            })
        })
        .collect();
    let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| **r == Err(ShellPreferencesError::Conflict))
            .count(),
        1
    );
    for i in 0..128 {
        store
            .save(user, profile(i), u64::from(i) + 1, Device, Some(Launcher))
            .unwrap();
    }
    assert_eq!(
        store.save(user, profile(128), 129, Device, Some(Launcher)),
        Err(ShellPreferencesError::Limit)
    );
    store.save(user, profile(0), 129, Device, None).unwrap();
    store
        .save(user, profile(128), 130, Device, Some(Launcher))
        .unwrap();
    assert!(BrowserProfileId::parse("../other-user").is_none());
}
