use cuerithm_media::CancellationToken;
use cuerithm_rekordbox::{
    DatabaseLocationState, DesktopPlatform, DiscoveryPaths, discover_targets,
};
use std::path::{Path, PathBuf};
fn app(root: &Path, version: &str) -> PathBuf {
    let bundle = root
        .join(format!("rekordbox {version}"))
        .join("rekordbox.app");
    std::fs::create_dir_all(bundle.join("Contents/MacOS")).unwrap();
    std::fs::write(
        bundle.join("Contents/MacOS/rekordbox"),
        b"generated executable marker",
    )
    .unwrap();
    let dict = plist::Dictionary::from_iter([
        (
            "CFBundleExecutable",
            plist::Value::String("rekordbox".into()),
        ),
        (
            "CFBundleShortVersionString",
            plist::Value::String(version.into()),
        ),
    ]);
    plist::Value::Dictionary(dict)
        .to_file_xml(bundle.join("Contents/Info.plist"))
        .unwrap();
    bundle
}
fn paths(root: &Path) -> DiscoveryPaths {
    DiscoveryPaths {
        platform: DesktopPlatform::MacOS,
        install_roots: vec![root.join("apps")],
        settings_file: root.join("settings/rekordbox3.settings"),
        default_database_dir: root.join("default-db"),
    }
}
#[test]
fn installed_six_and_seven_and_custom_database_location_are_detected_without_writes() {
    let d = tempfile::tempdir().unwrap();
    let p = paths(d.path());
    std::fs::create_dir(&p.install_roots[0]).unwrap();
    app(&p.install_roots[0], "6.7.7");
    app(&p.install_roots[0], "7.2.19.0342");
    let custom = d.path().join("external & library");
    std::fs::create_dir(&custom).unwrap();
    std::fs::write(custom.join("master.db"), b"encrypted marker").unwrap();
    std::fs::create_dir_all(p.settings_file.parent().unwrap()).unwrap();
    let text = format!(
        "<PROPERTIES><VALUE name=\"unrelated\" val=\"not selected\"/><VALUE name=\"masterDbDirectory\" val=\"{}\"/></PROPERTIES>",
        custom.to_string_lossy().replace('&', "&amp;")
    );
    std::fs::write(&p.settings_file, &text).unwrap();
    let result = discover_targets(&p, &CancellationToken::default()).unwrap();
    assert!(result.platform_supported);
    assert_eq!(result.installations.len(), 2);
    assert_eq!(result.installations[0].major, 7);
    assert!(
        result
            .installations
            .iter()
            .all(|a| a.version_source == "bundle_plist")
    );
    let db = result.database.unwrap();
    assert_eq!(db.path, custom.join("master.db"));
    assert_eq!(db.state, DatabaseLocationState::Present);
    assert_eq!(db.source, "configured");
    assert_eq!(std::fs::read_to_string(&p.settings_file).unwrap(), text);
    assert_eq!(std::fs::read(db.path).unwrap(), b"encrypted marker");
}
#[test]
fn malformed_or_ambiguous_settings_are_reported_instead_of_silently_selecting_a_target() {
    let d = tempfile::tempdir().unwrap();
    let p = paths(d.path());
    std::fs::create_dir_all(p.settings_file.parent().unwrap()).unwrap();
    for text in [
        "<malformed>",
        "<PROPERTIES><VALUE name=\"masterDbDirectory\" val=\"/a\"/><VALUE name=\"masterDbDirectory\" val=\"/b\"/></PROPERTIES>",
        "<!DOCTYPE PROPERTIES [<!ENTITY x SYSTEM 'file:///private/unread'>]><PROPERTIES><VALUE name=\"masterDbDirectory\" val=\"&x;\"/></PROPERTIES>",
    ] {
        std::fs::write(&p.settings_file, text).unwrap();
        let result = discover_targets(&p, &CancellationToken::default()).unwrap();
        assert!(result.database.is_none());
        assert!(result.issues.iter().any(|i| i.code == "invalid_settings"));
    }
}
#[test]
fn missing_settings_use_the_default_and_distinguish_missing_unavailable_and_nonregular_database() {
    let d = tempfile::tempdir().unwrap();
    let p = paths(d.path());
    let token = CancellationToken::default();
    let result = discover_targets(&p, &token).unwrap();
    assert_eq!(
        result.database.unwrap().state,
        DatabaseLocationState::Unavailable
    );
    std::fs::create_dir(&p.default_database_dir).unwrap();
    let result = discover_targets(&p, &token).unwrap();
    assert_eq!(
        result.database.unwrap().state,
        DatabaseLocationState::Missing
    );
    std::fs::create_dir(p.default_database_dir.join("master.db")).unwrap();
    let result = discover_targets(&p, &token).unwrap();
    assert_eq!(
        result.database.unwrap().state,
        DatabaseLocationState::NotRegular
    );
}
#[test]
fn linux_reports_local_target_unavailability_and_cancellation_does_not_create_paths() {
    let d = tempfile::tempdir().unwrap();
    let mut p = paths(d.path());
    p.platform = DesktopPlatform::Linux;
    let result = discover_targets(&p, &CancellationToken::default()).unwrap();
    assert!(!result.platform_supported);
    assert!(result.database.is_none() && result.installations.is_empty());
    p.platform = DesktopPlatform::MacOS;
    let cancel = CancellationToken::default();
    cancel.cancel();
    assert!(discover_targets(&p, &cancel).is_err());
    assert!(!p.default_database_dir.exists());
}
#[cfg(unix)]
#[test]
fn discovery_does_not_follow_directory_symlinks_or_accept_symbolic_database_files() {
    use std::os::unix::fs::symlink;
    let d = tempfile::tempdir().unwrap();
    let p = paths(d.path());
    std::fs::create_dir(&p.install_roots[0]).unwrap();
    let external = d.path().join("outside");
    std::fs::create_dir(&external).unwrap();
    app(&external, "7.2.19");
    symlink(&external, p.install_roots[0].join("rekordbox alias")).unwrap();
    std::fs::create_dir(&p.default_database_dir).unwrap();
    let real = d.path().join("actual.db");
    std::fs::write(&real, b"data").unwrap();
    symlink(&real, p.default_database_dir.join("master.db")).unwrap();
    let result = discover_targets(&p, &CancellationToken::default()).unwrap();
    assert!(result.installations.is_empty());
    assert_eq!(
        result.database.unwrap().state,
        DatabaseLocationState::NotRegular
    );
    assert_eq!(std::fs::read(real).unwrap(), b"data");
}
