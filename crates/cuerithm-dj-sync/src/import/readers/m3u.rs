use super::*;
use std::collections::BTreeSet;
pub struct M3uReader;
pub(super) fn text(bytes: &[u8]) -> Result<String> {
    if let Some(bytes) = bytes.strip_prefix(&[0xff, 0xfe]) {
        ensure!(bytes.len().is_multiple_of(2), "Truncated UTF-16LE playlist");
        let units = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect::<Vec<_>>();
        Ok(String::from_utf16(&units).context("Invalid UTF-16LE playlist")?)
    } else {
        Ok(std::str::from_utf8(bytes.strip_prefix(&[0xef,0xbb,0xbf]).unwrap_or(bytes))
            .context("Playlist requires UTF-8 or BOM-marked UTF-16LE; legacy encoding is unspecified")?.into())
    }
}
impl SourceAdapter for M3uReader {
    fn provider(&self) -> SourceProvider {
        SourceProvider::M3u
    }
    fn inspect(&self, snapshot: &ImportSnapshot) -> Result<SourceProfile> {
        ensure!(
            snapshot.provider == self.provider(),
            "M3U provider mismatch"
        );
        for artifact in &snapshot.artifacts {
            ensure!(
                matches!(artifact.kind, ArtifactKind::StableFile),
                "M3U requires text artifacts"
            );
            let extension = snapshot
                .original_artifact_path(&artifact.id)?
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            ensure!(
                extension.eq_ignore_ascii_case("m3u") || extension.eq_ignore_ascii_case("m3u8"),
                "Unsupported M3U artifact profile"
            );
        }
        Ok(profile(
            self.provider(),
            "m3u-text-v1",
            &["collections", "descriptive_tags", "key", "bpm"],
        ))
    }
    fn read(
        &self,
        snapshot: &ImportSnapshot,
        options: &ReadOptions,
        sink: &mut dyn ObservationSink,
        cancel: &CancellationToken,
    ) -> Result<ObservationHeader> {
        cancel.check()?;
        snapshot.validate_source(cancel)?;
        let profile = self.inspect(snapshot)?;
        let mut tracks = 0;
        let mut members = 0;
        for artifact in &snapshot.artifacts {
            let source = snapshot.original_artifact_path(&artifact.id)?;
            let text = text(&bytes(snapshot, &artifact.id, options, cancel)?)?;
            let collection = format!("playlist:{}", digest(artifact.id.as_bytes()));
            sink.push(ObservationRecord::Collection(CollectionObservation {
                external_id: collection.clone(),
                name: source
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into(),
                kind: CollectionKind::Playlist,
                ..Default::default()
            }))?;
            let mut seen = BTreeSet::new();
            let mut position = 0;
            let mut hint = None;
            for line in text
                .split(['\r', '\n'])
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                cancel.check()?;
                if let Some(value) = line.strip_prefix("#EXTINF:") {
                    hint = Some(value.to_owned());
                    continue;
                }
                if line.starts_with('#') {
                    continue;
                }
                let locations = location(line, source.parent(), options)?;
                let identity = locations
                    .native_path
                    .as_ref()
                    .map(|p| {
                        #[cfg(windows)]
                        if let Some(normalized) = locations
                            .extensions
                            .get("normalized_source_path")
                            .and_then(Value::as_str)
                        {
                            return normalized.as_bytes().to_vec();
                        }
                        p.as_os_str().as_encoded_bytes().to_vec()
                    })
                    .unwrap_or_else(|| {
                        locations
                            .extensions
                            .get("normalized_source_path")
                            .and_then(Value::as_str)
                            .or(locations.original.as_deref())
                            .unwrap_or(line)
                            .replace('\\', "/")
                            .into_bytes()
                    });
                let id = format!("{collection}/track:{}", digest(&identity));
                if seen.insert(id.clone()) {
                    emit_track(
                        TrackObservation {
                            external_id: id.clone(),
                            locations,
                            ..Default::default()
                        },
                        options,
                        sink,
                        cancel,
                    )?;
                    tracks += 1;
                }
                let mut member = membership(&collection, &id, position);
                if let ObservationRecord::Membership { occurrence, .. } = &mut member {
                    occurrence
                        .extensions
                        .insert("native_reference".into(), json!(line));
                }
                if let Some(hint) = hint.take()
                    && let ObservationRecord::Membership { occurrence, .. } = &mut member
                {
                    occurrence.extensions.insert("extinf".into(), json!(hint));
                }
                sink.push(member)?;
                position += 1;
                members += 1;
            }
        }
        cancel.check()?;
        snapshot.validate_source(cancel)?;
        Ok(header(
            snapshot,
            options,
            &profile,
            None,
            tracks,
            snapshot.artifacts.len() as u64,
            members,
        ))
    }
}
