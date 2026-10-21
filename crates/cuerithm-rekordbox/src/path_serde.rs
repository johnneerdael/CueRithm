//! Lossless native paths in private sync evidence. Ordinary paths retain their
//! existing JSON string representation; raw paths require the originating OS.
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum EncodedPath {
    Text(String),
    Unix { unix: Vec<u8> },
    Windows { windows: Vec<u16> },
}
impl EncodedPath {
    fn new(path: &Path) -> Self {
        if let Some(text) = path.to_str() {
            return Self::Text(text.into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            Self::Unix {
                unix: path.as_os_str().as_bytes().to_vec(),
            }
        }
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            Self::Windows {
                windows: path.as_os_str().encode_wide().collect(),
            }
        }
    }
    fn decode<E: Error>(self) -> Result<PathBuf, E> {
        match self {
            Self::Text(text) => Ok(text.into()),
            Self::Unix { unix } => {
                #[cfg(unix)]
                {
                    use std::os::unix::ffi::OsStringExt;
                    Ok(std::ffi::OsString::from_vec(unix).into())
                }
                #[cfg(not(unix))]
                {
                    let _ = unix;
                    Err(E::custom("Unix sync paths require platform mapping"))
                }
            }
            Self::Windows { windows } => {
                #[cfg(windows)]
                {
                    use std::os::windows::ffi::OsStringExt;
                    Ok(std::ffi::OsString::from_wide(&windows).into())
                }
                #[cfg(not(windows))]
                {
                    let _ = windows;
                    Err(E::custom("Windows sync paths require platform mapping"))
                }
            }
        }
    }
}
pub fn serialize<S: Serializer>(path: &Path, serializer: S) -> Result<S::Ok, S::Error> {
    EncodedPath::new(path).serialize(serializer)
}
pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<PathBuf, D::Error> {
    EncodedPath::deserialize(deserializer)?.decode()
}
pub mod option {
    use super::*;
    pub fn serialize<S: Serializer>(
        path: &Option<PathBuf>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        path.as_deref().map(EncodedPath::new).serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<PathBuf>, D::Error> {
        Option::<EncodedPath>::deserialize(deserializer)?
            .map(EncodedPath::decode)
            .transpose()
    }
}
pub mod map {
    use super::*;
    pub fn serialize<S: Serializer, V: Serialize>(
        paths: &BTreeMap<PathBuf, V>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        if paths.keys().all(|path| path.to_str().is_some()) {
            serializer.collect_map(
                paths
                    .iter()
                    .map(|(path, value)| (path.to_str().unwrap(), value)),
            )
        } else {
            serializer.collect_seq(
                paths
                    .iter()
                    .map(|(path, value)| (EncodedPath::new(path), value)),
            )
        }
    }
    pub fn deserialize<'de, D: Deserializer<'de>, V: Deserialize<'de>>(
        deserializer: D,
    ) -> Result<BTreeMap<PathBuf, V>, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr<V> {
            Object(BTreeMap<String, V>),
            Entries(Vec<(EncodedPath, V)>),
        }
        match Repr::<V>::deserialize(deserializer)? {
            Repr::Object(values) => Ok(values
                .into_iter()
                .map(|(key, value)| (key.into(), value))
                .collect()),
            Repr::Entries(values) => {
                let mut result = BTreeMap::new();
                for (key, value) in values {
                    if result.insert(key.decode::<D::Error>()?, value).is_some() {
                        return Err(D::Error::custom("Duplicate native sync path"));
                    }
                }
                Ok(result)
            }
        }
    }
}
/// Serato ownership associates each database with its optional original share root.
pub mod rooted_map {
    use super::*;
    type Paths<V> = BTreeMap<PathBuf, (Option<PathBuf>, V)>;
    pub fn serialize<S: Serializer, V: Serialize>(
        paths: &Paths<V>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let encoded: BTreeMap<_, _> = paths
            .iter()
            .map(|(path, (root, value))| {
                (path.clone(), (root.as_deref().map(EncodedPath::new), value))
            })
            .collect();
        map::serialize(&encoded, serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>, V: Deserialize<'de>>(
        deserializer: D,
    ) -> Result<Paths<V>, D::Error> {
        let encoded = map::deserialize::<D, (Option<EncodedPath>, V)>(deserializer)?;
        encoded
            .into_iter()
            .map(|(path, (root, value))| {
                Ok((path, (root.map(EncodedPath::decode).transpose()?, value)))
            })
            .collect()
    }
}
