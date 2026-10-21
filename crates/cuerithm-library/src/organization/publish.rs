use super::{OrganizationMode, journal::*, native::*};
use anyhow::{Context, Result, ensure};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Boundary {
    DirectoryCreated,
    StageReserved,
    StageProgress,
    StageDurable,
    Published,
    SourceQuarantined,
}
impl crate::Library {
    pub(crate) fn publish_organization(
        &self,
        record: &mut OrganizationRecord,
        mut boundary: impl FnMut(Boundary) -> Result<()>,
    ) -> Result<()> {
        if matches!(
            record.phase,
            OrganizationPhase::Planned | OrganizationPhase::Publishing
        ) {
            record.phase = OrganizationPhase::Publishing;
            self.save_organization(record)?;
            for index in 0..record.files.len() {
                if record.files[index].unchanged {
                    continue;
                }
                self.materialize_organization_parent(record, index, &mut boundary)?;
                self.stage_organization_file(record, index, &mut boundary)?;
                self.publish_organization_file(record, index, &mut boundary)?;
            }
            record.phase = OrganizationPhase::AwaitingReferences;
            self.save_organization(record)?;
        }
        if record.phase == OrganizationPhase::Cleanup {
            for index in 0..record.files.len() {
                verify_destination(&record.files[index])?;
                if record.files[index].unchanged || record.mode == OrganizationMode::Copy {
                    continue;
                }
                self.quarantine_organization_file(record, index, &mut boundary)?;
                verify_destination(&record.files[index])?;
            }
            record.phase = OrganizationPhase::Complete;
            self.finish_organization(record)?;
        }
        Ok(())
    }
    fn materialize_organization_parent(
        &self,
        record: &mut OrganizationRecord,
        index: usize,
        boundary: &mut impl FnMut(Boundary) -> Result<()>,
    ) -> Result<()> {
        let destination = record.files[index].destination.path()?;
        let wanted = destination
            .parent()
            .context("Missing destination directory")?;
        loop {
            let captured = record.files[index].destination_parent.path.path()?;
            if captured == wanted {
                return Ok(());
            }
            let remaining = wanted
                .strip_prefix(&captured)
                .context("Destination directory leaves captured authority")?;
            let name = remaining
                .components()
                .next()
                .context("Missing directory component")?;
            let next = captured.join(name.as_os_str());
            let known = record
                .created_directories
                .iter()
                .chain(record.files.iter().map(|f| &f.destination_parent))
                .find(|a| a.path.path().ok().as_ref() == Some(&next))
                .cloned();
            let authority = if let Some(known) = known {
                Parent::reopen(&known)?;
                known
            } else {
                let parent = Parent::reopen(&record.files[index].destination_parent)?;
                let authority = parent.create_directory(Path::new(name.as_os_str()))?;
                boundary(Boundary::DirectoryCreated)?;
                authority
            };
            if !record
                .created_directories
                .iter()
                .any(|a| a.path == authority.path)
            {
                record.created_directories.push(authority.clone());
            }
            record.files[index].destination_parent = authority;
            self.save_organization(record)?;
        }
    }
    fn stage_organization_file(
        &self,
        record: &mut OrganizationRecord,
        index: usize,
        boundary: &mut impl FnMut(Boundary) -> Result<()>,
    ) -> Result<()> {
        let file = &record.files[index];
        if file.published {
            return verify_destination(file);
        }
        let source = Parent::reopen(&file.source_parent)?;
        let destination = Parent::reopen(&file.destination_parent)?;
        let source_path = file.source.path()?;
        let source_name = source_path.file_name().context("Missing source name")?;
        let stage_name = Path::new(&file.stage_name);
        if let Some(expected) = &file.staged {
            if let Some(actual) = destination.optional(stage_name)? {
                ensure!(
                    actual.identity == expected.identity,
                    "Staging object was replaced; preserving it for recovery"
                );
                if file.stage_complete {
                    ensure!(
                        matches(&actual, expected),
                        "Staged bytes changed; preserving them for recovery"
                    );
                    return Ok(());
                }
                if actual.sha256 == file.original.sha256 && actual.bytes == file.original.bytes {
                    record.files[index].staged = Some(actual);
                    record.files[index].stage_complete = true;
                    self.save_organization(record)?;
                    return Ok(());
                }
                // Preserve interrupted partial bytes; start a different exclusive
                // temporary rather than truncating a possibly edited old object.
                let old_name = file.stage_name.clone();
                record.files[index]
                    .abandoned_stages
                    .push((old_name, actual));
                record.files[index].stage_name =
                    format!(".cuerithm-{}.stage", uuid::Uuid::new_v4());
                record.files[index].staged = None;
                self.save_organization(record)?;
            } else {
                // A rename may have reached disk before its journal update.
                let target_path = file.destination.path()?;
                let actual = destination.fingerprint(Path::new(
                    target_path.file_name().context("Missing target name")?,
                ))?;
                ensure!(
                    file.stage_complete && matches(&actual, expected),
                    "Missing staging file does not authorize an existing destination"
                );
                record.files[index].published = true;
                self.save_organization(record)?;
                return Ok(());
            }
        }
        let file = &record.files[index];
        let mut input = source.read(Path::new(source_name))?;
        ensure!(
            matches(&fingerprint(&mut input)?, &file.original),
            "Source changed since organization planning"
        );
        use std::io::{Read, Seek, SeekFrom, Write};
        input.seek(SeekFrom::Start(0))?;
        let mut output = destination.create(Path::new(&file.stage_name))?;
        record.files[index].staged = Some(fingerprint(&mut output)?);
        self.save_organization(record)?;
        boundary(Boundary::StageReserved)?;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let count = match input.read(&mut buffer) {
                Ok(count) => count,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            };
            if count == 0 {
                break;
            }
            output.write_all(&buffer[..count])?;
            boundary(Boundary::StageProgress)?;
        }
        output.set_permissions(input.metadata()?.permissions())?;
        output.sync_all()?;
        destination.sync()?;
        let staged = fingerprint(&mut output)?;
        ensure!(
            staged.sha256 == record.files[index].original.sha256
                && staged.bytes == record.files[index].original.bytes,
            "Organization copy failed byte verification"
        );
        ensure!(
            matches(&fingerprint(&mut input)?, &record.files[index].original),
            "Source changed while organization staged it"
        );
        boundary(Boundary::StageDurable)?;
        record.files[index].staged = Some(staged);
        record.files[index].stage_complete = true;
        self.save_organization(record)?;
        Ok(())
    }
    fn publish_organization_file(
        &self,
        record: &mut OrganizationRecord,
        index: usize,
        boundary: &mut impl FnMut(Boundary) -> Result<()>,
    ) -> Result<()> {
        if record.files[index].published {
            return verify_destination(&record.files[index]);
        }
        if record.files[index].case_only && !record.files[index].quarantined {
            self.quarantine_organization_file(record, index, boundary)?;
        }
        let file = &record.files[index];
        let source = Parent::reopen(&file.source_parent)?;
        let destination = Parent::reopen(&file.destination_parent)?;
        let source_path = file.source.path()?;
        let name = if file.quarantined {
            Path::new(&file.quarantine_name)
        } else {
            Path::new(source_path.file_name().context("Missing source name")?)
        };
        ensure!(
            matches(&source.fingerprint(name)?, &file.original),
            "Source changed before organization publication"
        );
        let staged = destination.fingerprint(Path::new(&file.stage_name))?;
        ensure!(
            matches(
                &staged,
                file.staged.as_ref().context("Missing verified stage")?
            ),
            "Staged file changed before publication"
        );
        let target_path = file.destination.path()?;
        destination.rename(
            Path::new(&file.stage_name),
            &destination,
            Path::new(target_path.file_name().context("Missing target name")?),
        )?;
        boundary(Boundary::Published)?;
        record.files[index].published = true;
        self.save_organization(record)?;
        verify_destination(&record.files[index])
    }
    fn quarantine_organization_file(
        &self,
        record: &mut OrganizationRecord,
        index: usize,
        boundary: &mut impl FnMut(Boundary) -> Result<()>,
    ) -> Result<()> {
        let file = &record.files[index];
        let source = Parent::reopen(&file.source_parent)?;
        // Revalidate the destination's complete parent chain at cleanup too.
        let _destination = Parent::reopen(&file.destination_parent)?;
        let source_path = file.source.path()?;
        let name = Path::new(source_path.file_name().context("Missing source name")?);
        let quarantine = Path::new(&file.quarantine_name);
        if let Some(actual) = source.optional(quarantine)? {
            ensure!(
                matches(&actual, &file.original),
                "Quarantine contains changed bytes; all files retained"
            );
            if !file.case_only {
                ensure!(
                    source.optional(name)?.is_none(),
                    "Source path was repopulated; all files retained"
                );
            }
        } else {
            ensure!(
                !file.quarantined,
                "Retained original recovery asset is missing"
            );
            ensure!(
                matches(&source.fingerprint(name)?, &file.original),
                "Original changed before cleanup; source retained"
            );
            source.rename(name, &source, quarantine)?;
            let actual = source.fingerprint(quarantine)?;
            if !matches(&actual, &file.original) {
                // A leaf replacement between validation and rename was moved,
                // never deleted. Restore only into an unoccupied original path.
                let restored = source.rename(quarantine, &source, name);
                anyhow::bail!(
                    "Source was replaced during cleanup; unexpected bytes preserved (restore: {})",
                    if restored.is_ok() {
                        "restored"
                    } else {
                        "quarantined; original occupied"
                    }
                );
            }
            boundary(Boundary::SourceQuarantined)?;
            if !file.case_only {
                ensure!(
                    source.optional(name)?.is_none(),
                    "Source path was repopulated during cleanup; all files retained"
                );
            }
        }
        record.files[index].quarantined = true;
        self.save_organization(record)?;
        Ok(())
    }
}
pub(crate) fn verify_destination(file: &OrganizationFile) -> Result<()> {
    let parent = Parent::reopen(&file.destination_parent)?;
    let path = file.destination.path()?;
    let expected = if file.unchanged {
        &file.original
    } else {
        file.staged.as_ref().context("Missing published identity")?
    };
    ensure!(
        matches(
            &parent.fingerprint(Path::new(
                path.file_name().context("Missing destination name")?
            ))?,
            expected
        ),
        "Published destination was changed or replaced; preserving all files"
    );
    Ok(())
}
