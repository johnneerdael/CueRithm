use super::*;
use anyhow::Result;
use cuerithm_media::CancellationToken;
use std::collections::BTreeMap;

/// Byte order established from the originating source architecture, never the host.
/// This qualifies legacy byte decoding only; it grants no canonical-clock authority.
#[derive(Debug, Clone, Copy)]
pub enum SourceByteOrder {
    Little,
    Big,
}
#[derive(Debug, Clone, Default)]
pub struct ReadOptions {
    pub limits: ImportLimits,
    pub include_history: bool,
    pub include_queue: bool,
    pub capture_file_tags: bool,
    pub origin: Option<ClockIdentity>,
    pub source_byte_order: Option<SourceByteOrder>,
    pub path_mappings: Vec<PathMapping>,
    pub library_instance_id: Option<String>,
}
pub trait SourceAdapter: Send + Sync {
    fn provider(&self) -> SourceProvider;
    fn inspect(&self, snapshot: &ImportSnapshot) -> Result<SourceProfile>;
    fn read(
        &self,
        snapshot: &ImportSnapshot,
        options: &ReadOptions,
        sink: &mut dyn ObservationSink,
        cancel: &CancellationToken,
    ) -> Result<ObservationHeader>;
}
struct UnavailableReader(SourceProvider);
impl SourceAdapter for UnavailableReader {
    fn provider(&self) -> SourceProvider {
        self.0
    }
    fn inspect(&self, _: &ImportSnapshot) -> Result<SourceProfile> {
        Ok(SourceProfile {
            provider: self.0,
            profile_id: "reader-unavailable".into(),
            schema_fingerprint: String::new(),
            origin: BTreeMap::new(),
            fields: BTreeMap::new(),
            issues: vec![ImportIssue {
                code: "reader_unavailable".into(),
                message: "This source reader is not available in this build".into(),
                ..Default::default()
            }],
        })
    }
    fn read(
        &self,
        _: &ImportSnapshot,
        _: &ReadOptions,
        _: &mut dyn ObservationSink,
        _: &CancellationToken,
    ) -> Result<ObservationHeader> {
        anyhow::bail!("Source reader is unavailable")
    }
}
pub fn adapter(provider: SourceProvider) -> &'static dyn SourceAdapter {
    if provider == SourceProvider::Mixxx {
        return &super::readers::mixxx::MixxxReader;
    }
    if provider == SourceProvider::Traktor {
        return &super::readers::traktor::TraktorReader;
    }
    if provider == SourceProvider::VirtualDj {
        return &super::readers::virtual_dj::VirtualDjReader;
    }
    if provider == SourceProvider::M3u {
        return &super::readers::m3u::M3uReader;
    }
    if provider == SourceProvider::AppleMusicXml {
        return &super::readers::apple_music::AppleMusicReader;
    }
    static READERS: [UnavailableReader; 13] = [
        UnavailableReader(SourceProvider::Rekordbox),
        UnavailableReader(SourceProvider::Serato3),
        UnavailableReader(SourceProvider::Serato4),
        UnavailableReader(SourceProvider::EngineDj),
        UnavailableReader(SourceProvider::Traktor),
        UnavailableReader(SourceProvider::VirtualDj),
        UnavailableReader(SourceProvider::DjayPro),
        UnavailableReader(SourceProvider::AppleMusicXml),
        UnavailableReader(SourceProvider::PioneerUsb),
        UnavailableReader(SourceProvider::M3u),
        UnavailableReader(SourceProvider::MixedInKey11),
        UnavailableReader(SourceProvider::DjStudio),
        UnavailableReader(SourceProvider::Mixxx),
    ];
    READERS
        .iter()
        .find(|r| r.0 == provider)
        .expect("exhaustive provider registry")
}
