use cuerithm_dj_sync::import::*;
use std::path::PathBuf;

fn local_root(path: &str) -> PathBuf {
    #[cfg(windows)]
    {
        PathBuf::from(r"C:\").join(path.trim_start_matches('/'))
    }
    #[cfg(not(windows))]
    {
        PathBuf::from(path)
    }
}

#[test]
fn mappings_use_longest_component_prefix_and_preserve_unicode() {
    let mappings = vec![
        PathMapping {
            source_prefix: "C:\\Music".into(),
            local_root: local_root("/mapped"),
            root_id: "general".into(),
        },
        PathMapping {
            source_prefix: "C:\\Music\\House".into(),
            local_root: local_root("/house"),
            root_id: "house".into(),
        },
    ];
    let e = LocationEvidence {
        original: Some("c:\\Music\\House\\α.flac".into()),
        platform: Some("windows".into()),
        case_sensitive: Some(false),
        ..Default::default()
    };
    let r = resolve_location(&e, &mappings).unwrap();
    assert_eq!(r.resolved, Some(local_root("/house").join("α.flac")));
    let e = LocationEvidence {
        original: Some("C:\\Musical\\α.flac".into()),
        ..e
    };
    let unmatched = resolve_location(&e, &mappings).unwrap();
    assert_eq!(unmatched.mapping_root_id, None);
    #[cfg(not(windows))]
    assert_eq!(unmatched.resolved, None);
    #[cfg(windows)]
    assert_eq!(unmatched.resolved, Some(PathBuf::from("C:/Musical/α.flac")));
}
#[test]
fn unc_uri_case_and_traversal_are_explicit() {
    let mapping = PathMapping {
        source_prefix: "\\\\server\\share".into(),
        local_root: local_root("/share"),
        root_id: "share".into(),
    };
    let e = LocationEvidence {
        original: Some("file://server/share/Track%20α.flac".into()),
        platform: Some("windows".into()),
        case_sensitive: Some(false),
        ..Default::default()
    };
    assert_eq!(
        resolve_location(&e, &[mapping]).unwrap().resolved,
        Some(local_root("/share").join("Track α.flac"))
    );
    let e = LocationEvidence {
        original: Some("C:\\Music\\..\\private.flac".into()),
        ..e
    };
    assert!(resolve_location(&e, &[]).is_err());
}
#[test]
fn clock_inverse_requires_exact_profile_identity() {
    let t = NativeTime {
        seconds: 1.25,
        identity: ClockIdentity {
            source_profile: "mixxx-mad".into(),
            codec: "mp3".into(),
            sample_rate: 44100,
            media_sha256: "a".repeat(64),
            decoder: "mad-profile".into(),
            origin_os: "linux".into(),
            renderer: None,
        },
    };
    assert_eq!(align_time(&t, None).unwrap().canonical_seconds, None);
    let p = QualifiedClockProfile {
        profile_id: "qualified-test".into(),
        identity: t.identity.clone(),
        offset_seconds: 0.025,
    };
    assert!((align_time(&t, Some(&p)).unwrap().canonical_seconds.unwrap() - 1.225).abs() < 1e-12);
    let mut other = t;
    other.identity.origin_os = "macos".into();
    assert_eq!(
        align_time(&other, Some(&p)).unwrap().canonical_seconds,
        None
    );
}

#[cfg(unix)]
#[test]
fn native_non_utf8_paths_roundtrip_without_lossy_display_matching() {
    use std::os::unix::ffi::OsStringExt;
    let native = PathBuf::from(std::ffi::OsString::from_vec(
        b"/music/raw-\xff.flac".to_vec(),
    ));
    let e = LocationEvidence {
        native_path: Some(native.clone()),
        platform: Some(std::env::consts::OS.into()),
        ..Default::default()
    };
    let serialized = serde_json::to_vec(&e).unwrap();
    let e: LocationEvidence = serde_json::from_slice(&serialized).unwrap();
    assert_eq!(resolve_location(&e, &[]).unwrap().resolved, Some(native));
}
