use std::fs;
use std::path::PathBuf;

use rumahl_ui_contracts::{
    ResolvedTheme, ShellSnapshot, ShellSnapshotError, ThemeManifest, ThemeManifestError,
    ThemeToken, ThemeTokenValue, WindowChromeVariant, typescript_contracts_v1,
};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixture(path: &str) -> Vec<u8> {
    fs::read(crate_root().join("fixtures").join(path)).unwrap()
}

#[test]
fn resolves_stock_and_contrasting_themes_deterministically() {
    let stock = ThemeManifest::from_json(&fixture("themes/stock.json")).unwrap();
    let contrast = ThemeManifest::from_json(&fixture("themes/contrast.json")).unwrap();
    let stock = ResolvedTheme::resolve(&stock);
    let contrast = ResolvedTheme::resolve(&contrast);

    assert_eq!(stock.window_chrome(), WindowChromeVariant::Standard);
    assert_eq!(contrast.window_chrome(), WindowChromeVariant::Compact);
    assert_eq!(
        contrast.tokens().get(&ThemeToken::RadiusWindow),
        Some(&ThemeTokenValue::Pixels(0))
    );
    assert_eq!(contrast.compile_css(), contrast.compile_css());
    assert_ne!(stock.compile_css(), contrast.compile_css());
    assert_eq!(contrast.compile_css().matches("--rumahl-").count(), 9);
    assert!(!contrast.compile_css().contains("url("));
}

#[test]
fn rejects_unknown_or_executable_theme_values() {
    let unknown = br##"{
      "manifestVersion": 1,
      "uiContractVersion": 1,
      "id": "com.example.theme",
      "name": "Bad",
      "tokens": {"css.global": "body { display: none; }"}
    }"##;
    assert!(matches!(
        ThemeManifest::from_json(unknown),
        Err(ThemeManifestError::UnknownToken(_))
    ));

    let executable = br##"{
      "manifestVersion": 1,
      "uiContractVersion": 1,
      "id": "com.example.theme",
      "name": "Bad",
      "tokens": {"color.accent.primary": "url(https://example.test/track)"}
    }"##;
    assert!(matches!(
        ThemeManifest::from_json(executable),
        Err(ThemeManifestError::InvalidTokenValue(_))
    ));
}

#[test]
fn rejects_theme_that_hides_protected_window_controls() {
    let hidden_controls = br##"{
      "manifestVersion": 1,
      "uiContractVersion": 1,
      "id": "com.example.theme",
      "name": "Hidden controls",
      "tokens": {
        "color.window.titlebar.background": "#ffffff",
        "color.window.titlebar.foreground": "#ffffff"
      }
    }"##;
    assert_eq!(
        ThemeManifest::from_json(hidden_controls).unwrap_err(),
        ThemeManifestError::InsufficientContrast("window titlebar")
    );
}

#[test]
fn parses_and_canonically_serializes_authenticated_snapshot() {
    let bytes = fixture("snapshots/authenticated.json");
    let snapshot = ShellSnapshot::from_json(&bytes).unwrap();

    assert_eq!(snapshot.shell_build_id(), "shell-build-001");
    assert_eq!(snapshot.user().locale(), "de-DE");
    assert_eq!(snapshot.system_status().installed_app_count(), 12);
    assert_eq!(
        snapshot.system_status().last_activity_at_unix_ms(),
        Some(1_790_105_880_000)
    );
    assert_eq!(snapshot.contributions().len(), 3);
    assert_eq!(
        ShellSnapshot::from_json(snapshot.to_json().unwrap().as_bytes()).unwrap(),
        snapshot
    );
}

#[test]
fn rejects_inconsistent_system_status_timestamps() {
    let mut value: serde_json::Value =
        serde_json::from_slice(&fixture("snapshots/authenticated.json")).unwrap();
    value["systemStatus"]["lastActivityAtUnixMs"] = 1_790_106_120_001_u64.into();

    assert_eq!(
        ShellSnapshot::from_json(&serde_json::to_vec(&value).unwrap()).unwrap_err(),
        ShellSnapshotError::InvalidSystemStatus
    );
}

#[test]
fn rejects_unknown_snapshot_fields_and_contract_versions() {
    let unknown = br##"{
      "snapshotVersion": 1,
      "uiContractVersion": 1,
      "extensionApiVersion": 1,
      "shellBuildId": "shell-build-001",
      "revision": "revision-001",
      "user": {"displayName": "Ada", "locale": "de-DE", "admin": true},
      "theme": {
        "stylesheetUrl": "/shell/themes/sha256-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.css",
        "windowChrome": "standard"
      },
      "contributions": []
    }"##;
    assert_eq!(
        ShellSnapshot::from_json(unknown).unwrap_err(),
        ShellSnapshotError::InvalidJson
    );

    let mut value: serde_json::Value =
        serde_json::from_slice(&fixture("snapshots/authenticated.json")).unwrap();
    value["snapshotVersion"] = 2.into();
    assert_eq!(
        ShellSnapshot::from_json(&serde_json::to_vec(&value).unwrap()).unwrap_err(),
        ShellSnapshotError::UnsupportedSnapshotVersion(2)
    );
}

#[test]
fn checked_in_typescript_contract_is_current() {
    let checked_in = fs::read_to_string(
        crate_root()
            .join("..")
            .join("frontend/packages/contracts/src/v1.ts"),
    )
    .unwrap();
    assert_eq!(checked_in, typescript_contracts_v1());
}

#[test]
fn checked_in_schemas_are_valid_json() {
    for schema in [
        "schemas/theme-manifest-v1.schema.json",
        "schemas/shell-snapshot-v1.schema.json",
    ] {
        let bytes = fs::read(crate_root().join(schema)).unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            parsed["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
    }
}
