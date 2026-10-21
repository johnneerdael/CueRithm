//! Shared lossless native path encoding for private sync evidence.
pub use cuerithm_rekordbox::path_serde::*;

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use std::{collections::BTreeMap, path::PathBuf};
    #[derive(Serialize, Deserialize)]
    struct Paths {
        #[serde(with = "super")]
        path: PathBuf,
        #[serde(with = "super::option")]
        optional: Option<PathBuf>,
        #[serde(with = "super::map")]
        keyed: BTreeMap<PathBuf, u32>,
        #[serde(with = "super::rooted_map")]
        roots: BTreeMap<PathBuf, (Option<PathBuf>, u32)>,
    }
    #[test]
    fn raw_platform_paths_roundtrip_without_replacement_or_map_key_loss() {
        #[cfg(unix)]
        let path = {
            use std::os::unix::ffi::OsStringExt;
            PathBuf::from(std::ffi::OsString::from_vec(
                b"/music/raw-\xff.wav".to_vec(),
            ))
        };
        #[cfg(windows)]
        let path = {
            use std::os::windows::ffi::OsStringExt;
            PathBuf::from(std::ffi::OsString::from_wide(&[
                0x43, 0x3a, 0x5c, 0xd800, 0x2e, 0x77, 0x61, 0x76,
            ]))
        };
        assert!(path.to_str().is_none());
        let value = Paths {
            path: path.clone(),
            optional: Some(path.clone()),
            keyed: BTreeMap::from([(path.clone(), 1)]),
            roots: BTreeMap::from([(path.clone(), (Some(path.clone()), 2))]),
        };
        let bytes = serde_json::to_vec(&value).unwrap();
        let restored: Paths = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(restored.path, path);
        assert_eq!(restored.optional, Some(path.clone()));
        assert_eq!(restored.keyed, value.keyed);
        assert_eq!(restored.roots, value.roots);
    }
    #[test]
    fn ordinary_json_keeps_string_paths_and_object_maps() {
        let path = PathBuf::from("/music/ordinary.wav");
        let value = Paths {
            path: path.clone(),
            optional: None,
            keyed: BTreeMap::from([(path.clone(), 1)]),
            roots: BTreeMap::from([(path.clone(), (Some(path.clone()), 2))]),
        };
        let json = serde_json::to_value(&value).unwrap();
        assert_eq!(json["path"], "/music/ordinary.wav");
        assert_eq!(json["keyed"]["/music/ordinary.wav"], 1);
        assert_eq!(
            json["roots"]["/music/ordinary.wav"][0],
            "/music/ordinary.wav"
        );
        let _: Paths = serde_json::from_value(json).unwrap();
        let target: crate::SyncTarget =
            serde_json::from_str(r#"{"software":"engine_dj","path":"/music/ordinary.wav"}"#)
                .unwrap();
        assert_eq!(
            serde_json::to_value(target).unwrap()["path"],
            "/music/ordinary.wav"
        );
        let track = crate::SyncTrack {
            path,
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(track).unwrap()["path"],
            "/music/ordinary.wav"
        );
    }
}
