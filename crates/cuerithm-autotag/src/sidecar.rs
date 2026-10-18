use crate::{AutotagConfig, TagInput, image::Artwork};
use anyhow::{Context, Result, ensure};
use cuerithm_media::file_sha256;
use onetagger_tag::TagImpl;
use std::path::{Component, Path, PathBuf};
#[derive(Debug)]
pub struct StagedSidecar {
    pub path: PathBuf,
    pub target_path: PathBuf,
    pub backup_path: Option<PathBuf>,
    pub before_sha256: Option<String>,
    pub after_sha256: String,
}
pub(crate) struct SidecarRequest {
    pub target: PathBuf,
    pub image: Artwork,
}
pub(crate) fn target_name(
    input: &TagInput,
    config: &AutotagConfig,
    tag: &dyn TagImpl,
) -> Result<PathBuf> {
    let parent = input
        .path
        .canonicalize()?
        .parent()
        .context("Source folder missing")?
        .to_owned();
    let mut info = input.audio_info(config)?;
    info.tags = tag.all_tags();
    if let Some(date) = tag.get_date() {
        info.tags.insert("year".into(), vec![date.year.to_string()]);
        if let Some(month) = date.month {
            info.tags.insert("month".into(), vec![month.to_string()]);
        }
        if let Some(day) = date.day {
            info.tags.insert("day".into(), vec![day.to_string()]);
        }
    }
    for value in info.tags.values().flatten() {
        ensure!(
            value.len() <= 65536,
            "Metadata value too long for artwork filename"
        );
    }
    let text = if let Some(template) = config
        .options
        .cover_filename
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        ensure!(
            template.len() <= 1024,
            "Artwork filename template exceeds limit"
        );
        let cfg = onetagger_template::RenamerConfig {
            separator: ", ".into(),
        };
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            onetagger_template::TemplateParser::parse(template).evaluate(&info, &cfg)
        }))
        .map_err(|_| anyhow::anyhow!("Invalid artwork filename template"))?
    } else {
        "cover".into()
    };
    ensure!(
        !text.is_empty() && text.len() <= 4096,
        "Invalid artwork filename"
    );
    // Relative subfolders are supported; traversal and platform-specific absolute paths are not.
    let relative = PathBuf::from(text.replace('\\', "/"));
    ensure!(
        relative
            .components()
            .all(|c| matches!(c, Component::Normal(_))),
        "Artwork filename must stay in the source folder"
    );
    ensure!(
        !relative.to_string_lossy().contains(':'),
        "Invalid artwork filename"
    );
    let target = parent.join(relative).with_extension("jpg");
    ensure!(
        target != input.path.canonicalize()?,
        "Artwork target collides with audio source"
    );
    let mut existing = target.parent().context("Invalid artwork destination")?;
    while !existing.exists() {
        existing = existing.parent().context("Invalid artwork destination")?;
    }
    ensure!(
        existing.canonicalize()?.starts_with(&parent),
        "Artwork destination leaves the source folder"
    );
    if let Ok(meta) = std::fs::symlink_metadata(&target) {
        ensure!(
            !meta.file_type().is_symlink() && meta.is_file(),
            "Artwork destination is not a regular file"
        );
        ensure!(
            target.canonicalize()? != input.path.canonicalize()?,
            "Artwork target collides with audio source"
        );
    }
    Ok(target)
}
pub(crate) fn stage(request: SidecarRequest, work: &Path, index: usize) -> Result<StagedSidecar> {
    let before = if request.target.exists() {
        Some(file_sha256(&request.target)?)
    } else {
        None
    };
    let backup = if before.is_some() {
        let p = work.join(format!("art-original-{index}.jpg"));
        std::fs::copy(&request.target, &p)?;
        ensure!(
            Some(file_sha256(&p)?) == before,
            "Artwork changed during capture"
        );
        Some(p)
    } else {
        None
    };
    let image = image::load_from_memory(&request.image.bytes)?.to_rgb8();
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 95).encode_image(&image)?;
    let path = work.join(format!("art-staged-{index}.jpg"));
    std::fs::write(&path, bytes)?;
    std::fs::File::options()
        .read(true)
        .write(true)
        .open(&path)?
        .sync_all()?;
    let current = if request.target.exists() {
        Some(file_sha256(&request.target)?)
    } else {
        None
    };
    ensure!(current == before, "Artwork changed during staging");
    Ok(StagedSidecar {
        after_sha256: file_sha256(&path)?,
        path,
        target_path: request.target,
        backup_path: backup,
        before_sha256: before,
    })
}
