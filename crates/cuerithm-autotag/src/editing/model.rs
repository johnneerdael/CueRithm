use crate::TransferDate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataTextField {
    Title,
    Artist,
    Album,
    AlbumArtist,
    Genre,
    Style,
    Label,
    Version,
    Remixer,
    Producer,
    Composer,
    Lyricist,
    Grouping,
    Comment,
    CatalogNumber,
    Isrc,
    Key,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TextEdit {
    #[default]
    Keep,
    Set {
        values: Vec<String>,
    },
    Clear,
    Append {
        values: Vec<String>,
    },
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ValueEdit<T> {
    #[default]
    Keep,
    Set {
        value: T,
    },
    Clear,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RatingIdentity {
    pub owner: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeRating {
    pub identity: RatingIdentity,
    pub raw: u8,
    pub counter: u64,
    pub stars: Option<u8>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentIdentity {
    pub language: String,
    pub description: String,
    pub sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeComment {
    pub identity: CommentIdentity,
    pub values: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtworkIdentity {
    pub kind: Option<u32>,
    pub description: Option<String>,
    pub sha256: String,
    pub ordinal: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeArtwork {
    pub identity: ArtworkIdentity,
    pub mime: String,
    pub bytes: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RatingEdit {
    pub identity: RatingIdentity,
    pub value: ValueEdit<u8>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentEdit {
    pub identity: CommentIdentity,
    pub value: TextEdit,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtworkValue {
    pub kind: Option<u32>,
    pub description: Option<String>,
    pub bytes: Vec<u8>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtworkEdit {
    pub target: Option<ArtworkIdentity>,
    pub replacement: Option<ArtworkValue>,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MetadataPatch {
    pub allow_id3_upgrade: bool,
    pub text: BTreeMap<MetadataTextField, TextEdit>,
    pub release_date: ValueEdit<TransferDate>,
    pub original_date: ValueEdit<TransferDate>,
    pub track_number: ValueEdit<u16>,
    pub track_total: ValueEdit<u16>,
    pub disc_number: ValueEdit<u16>,
    pub disc_total: ValueEdit<u16>,
    pub bpm: ValueEdit<f64>,
    #[serde(skip)]
    pub(crate) artwork_order: Option<Vec<usize>>,
    #[serde(skip)]
    pub(crate) restored_ratings: Vec<(RatingIdentity, Option<NativeRating>)>,
    pub ratings: Vec<RatingEdit>,
    pub comments: Vec<CommentEdit>,
    pub artwork: Vec<ArtworkEdit>,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MetadataSnapshot {
    pub text: BTreeMap<MetadataTextField, Vec<String>>,
    pub release_date: Option<TransferDate>,
    pub original_date: Option<TransferDate>,
    pub track_number: Option<u16>,
    pub track_total: Option<u16>,
    pub disc_number: Option<u16>,
    pub disc_total: Option<u16>,
    pub bpm: Option<f64>,
    pub ratings: Vec<NativeRating>,
    pub comments: Vec<NativeComment>,
    pub artwork: Vec<NativeArtwork>,
    pub capabilities: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetadataEditorReceipt {
    pub before: MetadataSnapshot,
    pub after: MetadataSnapshot,
}
