use cuerithm_media::CancellationToken;
use cuerithm_rekordbox::*;
use std::{
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

const CIPHERTEXT: &str = "QUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUE=";
const FUSE: &[u8] = b"dL7pKGdnNz796PbbjQWNKmHXBZaB9tsX\x01\x06\x31\x31\x31\x30\x30\x30";
fn node() -> PathBuf {
    let name = if cfg!(windows) { "node.exe" } else { "node" };
    std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|p| p.join(name))
        .find(|p| p.is_file())
        .expect("Node is required for generated bridge tests, never for the shipped desktop")
        .canonicalize()
        .unwrap()
}
fn write_asar(path: &Path, entries: Vec<(&str, Vec<u8>)>) {
    let mut tree = serde_json::json!({"files":{}});
    let mut body = Vec::new();
    for (name, bytes) in entries {
        let mut object = &mut tree;
        let parts: Vec<_> = name.split('/').collect();
        for part in &parts[..parts.len() - 1] {
            let files = object["files"].as_object_mut().unwrap();
            object = files
                .entry((*part).to_owned())
                .or_insert_with(|| serde_json::json!({"files":{}}));
        }
        object["files"][parts[parts.len() - 1]] =
            serde_json::json!({"size":bytes.len(),"offset":body.len().to_string()});
        body.extend(bytes);
    }
    let header = serde_json::to_vec(&tree).unwrap();
    let padded = (4 + header.len() + 3) & !3;
    let mut output = Vec::new();
    output.extend(4u32.to_le_bytes());
    output.extend(((padded + 4) as u32).to_le_bytes());
    output.extend((padded as u32).to_le_bytes());
    output.extend((header.len() as u32).to_le_bytes());
    output.extend(header);
    output.resize(12 + padded, 0);
    output.extend(body);
    std::fs::write(path, output).unwrap();
}
fn fixture(dir: &Path, helper: &str) -> AgentKeySource {
    let runtime = node();
    let cache = dir.join("cache.bin");
    let forbidden = serde_json::to_string(&dir.join("forbidden-file")).unwrap();
    let module = format!(
        r#"(function(exports,require,module,__filename,__dirname){{
        function buildBFKey(){{{helper}}}
        require('fs').writeFileSync({forbidden},'must not be written');
        require('child_process').spawn('forbidden-command');
        console.log('do not leak fixture log');
        require('electron').app.on('ready', function(){{return buildBFKey()}});
    }})"#
    );
    let result=Command::new(&runtime).args(["-e","const fs=require('fs'),vm=require('vm'),v8=require('v8');v8.setFlagsFromString('--no-lazy');fs.writeFileSync(process.argv[1],new vm.Script(process.argv[2]).createCachedData());"]).arg(&cache).arg(module).output().unwrap();
    assert!(
        result.status.success(),
        "Could not produce generated VM fixture"
    );
    let crypto=br#"class B {constructor(seed){if(seed!=='synthetic-seed')throw Error('wrong seed')} decode(data){return Buffer.from(data).toString('utf8')}} B.MODE={ECB:0};B.PADDING={PKCS5:0};B.TYPE={STRING:0};module.exports=B;"#;
    let archive = dir.join("app.asar");
    let bytes = std::fs::read(cache).unwrap();
    write_asar(
        &archive,
        vec![
            ("jsc/main_core.jsc", bytes.clone()),
            ("jsc_arm64/main_core.jsc", bytes.clone()),
            ("jsc_x64/main_core.jsc", bytes),
            (
                "node_modules/egoroof-blowfish/dist/blowfish.js",
                crypto.to_vec(),
            ),
        ],
    );
    let fuse_image = dir.join("fuses.bin");
    std::fs::write(&fuse_image, FUSE).unwrap();
    AgentKeySource {
        executable: runtime,
        archive,
        fuse_image,
    }
}
fn options(dir: &Path, dp: &str) -> PathBuf {
    let path = dir.join("options.json");
    std::fs::write(&path,serde_json::to_vec(&serde_json::json!({"options":[["unrelated-account-token","do-not-return-this"],["dp",dp],["db-path","not-used"]],"defaults":{"dp":"wrong-default"}})).unwrap()).unwrap();
    path
}
#[test]
fn automatic_local_bridge_opens_encryption_without_manual_key_and_ignores_unrelated_options() {
    let d = tempfile::tempdir().unwrap();
    let source = fixture(d.path(), "return 'synthetic-seed';");
    let path = options(d.path(), CIPHERTEXT);
    let key = resolve_database_key(
        &source,
        &path,
        Default::default(),
        &CancellationToken::default(),
    )
    .unwrap();
    assert_eq!(format!("{key:?}"), "DatabaseKey(<redacted>)");
    let database = d.path().join("master.db");
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
    let db = ReadOnlyDatabase::open(&database, &key).unwrap();
    assert!(db.checks().unwrap().integrity_ok);
    assert!(!d.path().join("forbidden-file").exists());
}
#[test]
fn invalid_missing_or_ambiguous_selected_options_do_not_expose_values() {
    let d = tempfile::tempdir().unwrap();
    let source = fixture(d.path(), "return 'synthetic-seed';");
    let path = d.path().join("options.json");
    for text in [
        r#"{}"#,
        r#"{"options":[["dp",null]]}"#,
        r#"{"options":[["dp",""]]}"#,
        r#"{"options":[["dp","SENSITIVE"],["dp","SENSITIVE2"]]}"#,
        r#"{"options":[["dp",{"private":"SENSITIVE"}]]}"#,
        r#"{"options":"SENSITIVE"}"#,
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
    let long = "x".repeat(32 * 1024);
    options(d.path(), &long);
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
fn cancellation_and_deadlines_reap_only_the_owned_helper_process() {
    let d = tempfile::tempdir().unwrap();
    let source = fixture(d.path(), "while(true){}");
    let path = options(d.path(), CIPHERTEXT);
    let token = CancellationToken::default();
    token.cancel();
    let start = Instant::now();
    assert!(resolve_database_key(&source, &path, Default::default(), &token).is_err());
    assert!(start.elapsed() < Duration::from_secs(1));
    let start = Instant::now();
    assert!(
        resolve_database_key(
            &source,
            &path,
            KeyProviderOptions {
                timeout: Duration::from_millis(200)
            },
            &CancellationToken::default()
        )
        .is_err()
    );
    assert!(start.elapsed() < Duration::from_secs(5));
    let cancel = CancellationToken::default();
    let other = cancel.clone();
    let worker = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        other.cancel()
    });
    assert!(resolve_database_key(&source, &path, Default::default(), &cancel).is_err());
    worker.join().unwrap();
}
#[test]
fn disabled_fuses_corrupt_archives_and_unexpected_helper_values_fail_closed() {
    let d = tempfile::tempdir().unwrap();
    let source = fixture(d.path(), "return 'synthetic-seed';");
    let path = options(d.path(), CIPHERTEXT);
    let mut disabled = FUSE.to_vec();
    disabled[34] = b'0';
    std::fs::write(&source.fuse_image, &disabled).unwrap();
    assert!(
        resolve_database_key(
            &source,
            &path,
            Default::default(),
            &CancellationToken::default()
        )
        .is_err()
    );
    std::fs::write(&source.fuse_image, FUSE).unwrap();
    std::fs::write(&source.archive, b"broken-asar").unwrap();
    assert!(
        resolve_database_key(
            &source,
            &path,
            Default::default(),
            &CancellationToken::default()
        )
        .is_err()
    );
    let source = fixture(d.path(), "return {secret:'do-not-echo'};");
    let error = resolve_database_key(
        &source,
        &path,
        Default::default(),
        &CancellationToken::default(),
    )
    .unwrap_err();
    assert!(!format!("{error:?}").contains("do-not-echo"));
}
#[test]
fn agent_locations_follow_platform_layout_without_launching_or_using_a_reference_app() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("rekordbox.app");
    let app = root.join("Contents/MacOS/rekordboxAgent.app/Contents");
    for path in [
        app.join("MacOS/rekordboxAgent"),
        app.join("Resources/app.asar"),
        app.join("Frameworks/Electron Framework.framework/Versions/A/Electron Framework"),
    ] {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"generated").unwrap();
    }
    let target = TargetInstallation {
        root: root.clone(),
        executable: root.join("Contents/MacOS/rekordbox"),
        version: "7.2.19".into(),
        major: 7,
        version_source: "bundle_plist".into(),
    };
    let found = AgentKeySource::for_installation(&target, DesktopPlatform::MacOS).unwrap();
    assert_eq!(found.executable, app.join("MacOS/rekordboxAgent"));
    assert!(AgentKeySource::for_installation(&target, DesktopPlatform::Linux).is_err());
    let win = d.path().join("rekordbox 6.8.5");
    let agent = win.join("rekordboxAgent-win32-x64");
    std::fs::create_dir_all(agent.join("resources")).unwrap();
    std::fs::write(agent.join("rekordboxAgent.exe"), b"generated").unwrap();
    std::fs::write(agent.join("resources/app.asar"), b"generated").unwrap();
    // Windows installations can contain many DLLs alongside the agent folder.
    for index in 0..130 {
        std::fs::write(win.join(format!("native-{index}.dll")), b"generated").unwrap();
    }
    let target = TargetInstallation {
        root: win,
        executable: PathBuf::new(),
        version: "6.8.5".into(),
        major: 6,
        version_source: "version_resource".into(),
    };
    let found = AgentKeySource::for_installation(&target, DesktopPlatform::Windows).unwrap();
    assert_eq!(found.archive, agent.join("resources/app.asar"));
}
#[cfg(unix)]
#[test]
fn symbolic_secret_inputs_and_agent_files_are_rejected() {
    use std::os::unix::fs::symlink;
    let d = tempfile::tempdir().unwrap();
    let source = fixture(d.path(), "return 'synthetic-seed';");
    let path = options(d.path(), CIPHERTEXT);
    let link = d.path().join("linked.json");
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
    let mut other = source.clone();
    let archive = d.path().join("linked.asar");
    symlink(&source.archive, &archive).unwrap();
    other.archive = archive;
    assert!(
        resolve_database_key(
            &other,
            &path,
            Default::default(),
            &CancellationToken::default()
        )
        .is_err()
    );
}
