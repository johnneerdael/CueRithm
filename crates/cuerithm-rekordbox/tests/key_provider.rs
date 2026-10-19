use cuerithm_media::CancellationToken;
use cuerithm_rekordbox::*;
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

// Ciphertext from unchanged egoroof-blowfish 2.2.2, ECB/PKCS5, on authored
// seed 0123456789abcdef and plaintext A*32. It is not target key material.
const MARKER: &[u8] = b"rekordbox_client\0";
fn image(seed: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0xca, 0xfe, 0xba, 0xbe];
    bytes.extend([0; 124]);
    bytes.extend(MARKER);
    bytes.extend(seed);
    bytes.push(0);
    bytes
}
fn fixture(dir: &Path) -> NativeKeySource {
    let executable = dir.join("rekordbox");
    std::fs::write(&executable, image(b"0123456789abcdef")).unwrap();
    NativeKeySource { executable }
}
fn options(dir: &Path, dp: &str) -> PathBuf {
    let path = dir.join("options.json");
    std::fs::write(&path,serde_json::to_vec(&serde_json::json!({"options":[["unrelated-token","do-not-return-this"],["dp",dp],["db-path","unused"]],"defaults":{"dp":"wrong-default"}})).unwrap()).unwrap();
    path
}
fn wire() -> String {
    let v: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/native-key/reference.json")).unwrap();
    v["ciphertext"].as_str().unwrap().into()
}

fn assert_unlocks_fixture(dir: &Path, key: &DatabaseKey) {
    let database = dir.join("master.db");
    let conn = rusqlite::Connection::open(&database).unwrap();
    let bytes = [b'A'; 32];
    assert_eq!(
        unsafe {
            rusqlite::ffi::sqlite3_key(conn.handle(), bytes.as_ptr().cast(), bytes.len() as i32)
        },
        rusqlite::ffi::SQLITE_OK
    );
    conn.pragma_update(None, "cipher_compatibility", 4).unwrap();
    conn.execute_batch("CREATE TABLE original(id INTEGER)")
        .unwrap();
    drop(conn);
    assert!(
        ReadOnlyDatabase::open(&database, key)
            .unwrap()
            .checks()
            .unwrap()
            .integrity_ok
    );
}
#[test]
fn native_local_provider_opens_encryption_without_any_external_runtime_or_manual_key() {
    let d = tempfile::tempdir().unwrap();
    let source = fixture(d.path());
    let path = options(d.path(), &wire());
    let key = resolve_database_key(
        &source,
        &path,
        Default::default(),
        &CancellationToken::default(),
    )
    .unwrap();
    assert_eq!(format!("{key:?}"), "DatabaseKey(<redacted>)");
    assert_unlocks_fixture(d.path(), &key);
}
#[test]
fn native_seed_scanning_crosses_chunk_boundaries_deduplicates_architectures_and_rejects_ambiguity()
{
    let d = tempfile::tempdir().unwrap();
    let source = fixture(d.path());
    let path = options(d.path(), &wire());
    let mut bytes = vec![0; 65530];
    bytes[..4].copy_from_slice(&[0xca, 0xfe, 0xba, 0xbe]);
    bytes.extend(MARKER);
    bytes.extend(*b"0123456789abcdef");
    bytes.push(0);
    bytes.extend(image(b"0123456789abcdef"));
    std::fs::write(&source.executable, &bytes).unwrap();
    assert!(
        resolve_database_key(
            &source,
            &path,
            Default::default(),
            &CancellationToken::default()
        )
        .is_ok()
    );
    bytes.extend(image(&[b'T'; 16]));
    std::fs::write(&source.executable, bytes).unwrap();
    assert!(
        resolve_database_key(
            &source,
            &path,
            Default::default(),
            &CancellationToken::default()
        )
        .is_err()
    );
}
#[test]
fn unsupported_images_missing_markers_and_invalid_ciphertext_are_explicit_errors() {
    let d = tempfile::tempdir().unwrap();
    let source = fixture(d.path());
    let path = options(d.path(), &wire());
    for bytes in [
        b"unknown format".to_vec(),
        vec![0xca, 0xfe, 0xba, 0xbe],
        image(&[b'S'; 15]),
        image(&[0; 16]),
    ] {
        std::fs::write(&source.executable, bytes).unwrap();
        assert!(
            resolve_database_key(
                &source,
                &path,
                Default::default(),
                &CancellationToken::default()
            )
            .is_err()
        );
    }
    std::fs::write(&source.executable, image(b"0123456789abcdef")).unwrap();
    for ciphertext in ["invalid-base64!", "AAAA", "AAAAAAAAAAA="] {
        let path = options(d.path(), ciphertext);
        assert!(
            resolve_database_key(
                &source,
                &path,
                Default::default(),
                &CancellationToken::default()
            )
            .is_err()
        );
    }
}
#[test]
fn scoped_options_errors_do_not_expose_values_and_excessive_input_is_not_read() {
    let d = tempfile::tempdir().unwrap();
    let source = fixture(d.path());
    let path = d.path().join("options.json");
    for text in [
        r#"{}"#,
        r#"{"options":[["dp",null]]}"#,
        r#"{"options":[["dp",""]]}"#,
        r#"{"options":[["dp","SENSITIVE"],["dp","SENSITIVE2"]]}"#,
        r#"{"options":[["dp",{"private":"SENSITIVE"}]]}"#,
    ] {
        std::fs::write(&path, text).unwrap();
        let err = resolve_database_key(
            &source,
            &path,
            Default::default(),
            &CancellationToken::default(),
        )
        .unwrap_err();
        assert!(!format!("{err:?}").contains("SENSITIVE"));
    }
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(16 * 1024 * 1024 + 1).unwrap();
    drop(file);
    assert!(
        resolve_database_key(
            &source,
            &path,
            Default::default(),
            &CancellationToken::default()
        )
        .is_err()
    );
}
#[test]
fn cancellation_deadlines_and_binary_budget_stop_native_scanning() {
    let d = tempfile::tempdir().unwrap();
    let source = fixture(d.path());
    let path = options(d.path(), &wire());
    let cancel = CancellationToken::default();
    cancel.cancel();
    let start = Instant::now();
    assert!(resolve_database_key(&source, &path, Default::default(), &cancel).is_err());
    assert!(start.elapsed() < Duration::from_secs(1));
    assert!(
        resolve_database_key(
            &source,
            &path,
            KeyProviderOptions {
                timeout: Duration::ZERO
            },
            &CancellationToken::default()
        )
        .is_err()
    );
    let file = std::fs::File::create(&source.executable).unwrap();
    file.set_len(1024 * 1024 * 1024 + 1).unwrap();
    drop(file);
    assert!(
        resolve_database_key(
            &source,
            &path,
            Default::default(),
            &CancellationToken::default()
        )
        .is_err()
    );
}
#[cfg(unix)]
#[test]
fn symbolic_options_and_executables_are_rejected_without_following_them() {
    use std::os::unix::fs::symlink;
    let d = tempfile::tempdir().unwrap();
    let source = fixture(d.path());
    let path = options(d.path(), &wire());
    let link = d.path().join("link.json");
    symlink(&path, &link).unwrap();
    assert!(
        resolve_database_key(
            &source,
            &link,
            Default::default(),
            &CancellationToken::default()
        )
        .is_err()
    );
    let link = d.path().join("linked-executable");
    symlink(&source.executable, &link).unwrap();
    let source = NativeKeySource { executable: link };
    assert!(
        resolve_database_key(
            &source,
            &path,
            Default::default(),
            &CancellationToken::default()
        )
        .is_err()
    );
}
#[test]
fn native_source_uses_the_installed_main_executable_and_reports_linux_unavailability() {
    let d = tempfile::tempdir().unwrap();
    let source = fixture(d.path());
    let target = TargetInstallation {
        root: d.path().into(),
        executable: source.executable.clone(),
        version: "7.2.19".into(),
        major: 7,
        version_source: "bundle_plist".into(),
    };
    let found = NativeKeySource::for_installation(&target, DesktopPlatform::MacOS).unwrap();
    assert_eq!(found.executable, source.executable);
    assert!(NativeKeySource::for_installation(&target, DesktopPlatform::Windows).is_ok());
    assert!(NativeKeySource::for_installation(&target, DesktopPlatform::Linux).is_err());
}

#[test]
fn native_provider_accepts_the_observed_unpadded_aligned_option_variant() {
    let d = tempfile::tempdir().unwrap();
    let source = fixture(d.path());
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/native-key/reference.json")).unwrap();
    let path = options(d.path(), reference["unpadded_ciphertext"].as_str().unwrap());
    let key = resolve_database_key(
        &source,
        &path,
        Default::default(),
        &CancellationToken::default(),
    )
    .unwrap();
    assert_unlocks_fixture(d.path(), &key);
}
