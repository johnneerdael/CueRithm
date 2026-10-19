use cuerithm_autotag::{
    AnalysisTags, AutotagConfig, CatalogMatch, CatalogTrack, FieldChange, MatchStatus,
    MetadataField, SupportedTag, TagInput, TagMatchOutcome, TagWritePlan, read_metadata,
};
use cuerithm_media::{MediaRuntime, file_sha256};
use onetagger_tag::Tag;
use std::path::{Path, PathBuf};

struct Fixture {
    _dir: tempfile::TempDir,
    path: PathBuf,
    runtime: MediaRuntime,
}
impl Fixture {
    fn new(extension: &str, aliases: &[(&str, &str)]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        tar::Archive::new(flate2::read::GzDecoder::new(
            std::fs::File::open(root.join("tools/fixtures/media.tar.gz")).unwrap(),
        ))
        .unpack(dir.path())
        .unwrap();
        let path = dir.path().join(format!("pulses.{extension}"));
        let mut tag = Tag::load_file(&path, true).unwrap();
        for alias in ["VERSION", "SUBTITLE"] {
            tag.tag_mut().set_native(alias, vec![]);
        }
        for (alias, value) in aliases {
            tag.tag_mut().set_native(alias, vec![(*value).into()]);
        }
        tag.tag_mut().save_file(&path).unwrap();
        let runtime = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
        Self {
            _dir: dir,
            path,
            runtime,
        }
    }
    fn input(&self) -> TagInput {
        let audio = self.runtime.decode(&self.path, self._dir.path()).unwrap();
        TagInput {
            path: self.path.clone(),
            source_sha256: audio.source_sha256,
            media: audio.info,
            duration_seconds: audio.clock.duration_seconds(),
        }
    }
}
fn config(overwrite: bool) -> AutotagConfig {
    let mut config = AutotagConfig::default();
    config.options.tags = vec![SupportedTag::Version];
    config.options.overwrite = overwrite;
    config.options.overwrite_tags.clear();
    config.write_energy = false;
    config
}
fn outcome() -> TagMatchOutcome {
    TagMatchOutcome {
        status: MatchStatus::Matched,
        matches: vec![CatalogMatch::new(
            1.,
            CatalogTrack {
                platform: "beatport".into(),
                version: Some("Catalog Mix".into()),
                ..Default::default()
            },
        )],
        attempted: vec![],
        warnings: vec![],
    }
}

#[test]
fn mix_reads_canonical_subtitle_and_legacy_version_without_mutating_the_source() {
    for extension in ["flac", "ogg"] {
        for aliases in [
            vec![("SUBTITLE", "Canonical Mix")],
            vec![("VERSION", "Legacy Mix")],
            vec![("SUBTITLE", "Canonical Mix"), ("VERSION", "Legacy Mix")],
        ] {
            let fixture = Fixture::new(extension, &aliases);
            let before = file_sha256(&fixture.path).unwrap();
            let info = fixture.runtime.probe(&fixture.path).unwrap();
            let metadata = read_metadata(&fixture.path, &info).unwrap();
            assert_eq!(
                metadata.first(MetadataField::Mix),
                Some(aliases[0].1),
                "{extension}: {aliases:?}"
            );
            assert_eq!(file_sha256(&fixture.path).unwrap(), before);
        }
    }
}

#[test]
fn catalog_mix_respects_legacy_no_overwrite_and_synchronizes_existing_aliases_when_overwriting() {
    for extension in ["flac", "ogg"] {
        for aliases in [
            vec![("VERSION", "Legacy Mix")],
            vec![("SUBTITLE", "Canonical Mix"), ("VERSION", "Legacy Mix")],
        ] {
            let fixture = Fixture::new(extension, &aliases);
            let before = file_sha256(&fixture.path).unwrap();
            let audio = fixture
                .runtime
                .decode(&fixture.path, fixture._dir.path())
                .unwrap();
            for overwrite in [false, true] {
                let staged = TagWritePlan::build(
                    fixture.input(),
                    &outcome(),
                    &config(overwrite),
                    &AnalysisTags::default(),
                )
                .unwrap()
                .stage(&fixture.runtime, fixture._dir.path())
                .unwrap();
                let metadata = read_metadata(&staged.path, &staged.metadata).unwrap();
                assert_eq!(
                    metadata.first(MetadataField::Mix),
                    Some(if overwrite {
                        "Catalog Mix"
                    } else {
                        aliases[0].1
                    }),
                    "{extension}: overwrite={overwrite} {aliases:?}"
                );
                let tag = Tag::load_file(&staged.path, true).unwrap();
                for (alias, value) in &aliases {
                    assert_eq!(
                        tag.tag().get_native(alias),
                        Some(vec![
                            if overwrite { "Catalog Mix" } else { value }.to_string()
                        ])
                    );
                }
                if overwrite {
                    assert_eq!(
                        tag.tag().get_native("SUBTITLE"),
                        Some(vec!["Catalog Mix".into()])
                    );
                }
                let after = fixture
                    .runtime
                    .decode(&staged.path, fixture._dir.path())
                    .unwrap();
                assert_eq!(after.pcm_sha256().unwrap(), audio.pcm_sha256().unwrap());
                assert_eq!(file_sha256(&fixture.path).unwrap(), before);
            }
        }
    }
}

#[test]
fn cleanup_mix_replacement_and_clear_apply_to_all_existing_aliases() {
    for extension in ["flac", "ogg"] {
        let fixture = Fixture::new(
            extension,
            &[("SUBTITLE", "Canonical Mix"), ("VERSION", "Legacy Mix")],
        );
        let before = file_sha256(&fixture.path).unwrap();
        let info = fixture.runtime.probe(&fixture.path).unwrap();
        let original = read_metadata(&fixture.path, &info).unwrap();
        let mut settings = config(false);
        settings.options.tags.clear();
        for after in [vec!["Clean Mix".to_string()], vec![]] {
            let staged = TagWritePlan::build(
                fixture.input(),
                &outcome(),
                &settings,
                &AnalysisTags::default(),
            )
            .unwrap()
            .with_metadata_edits(vec![FieldChange {
                field: MetadataField::Mix,
                before: original.fields[&MetadataField::Mix].clone(),
                after: after.clone(),
            }])
            .unwrap()
            .stage(&fixture.runtime, fixture._dir.path())
            .unwrap();
            let metadata = read_metadata(&staged.path, &staged.metadata).unwrap();
            assert_eq!(
                metadata
                    .fields
                    .get(&MetadataField::Mix)
                    .cloned()
                    .unwrap_or_default(),
                after
            );
            let tag = Tag::load_file(&staged.path, true).unwrap();
            for alias in ["VERSION", "SUBTITLE"] {
                assert_eq!(
                    tag.tag().get_native(alias).unwrap_or_default(),
                    after,
                    "{extension}: {alias}"
                );
            }
            assert_eq!(file_sha256(&fixture.path).unwrap(), before);
        }
    }
}
