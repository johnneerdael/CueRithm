use super::*;
use anyhow::{Context, Result, bail, ensure};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::BTreeSet;

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct ValidationReport {
    pub tracks: u64,
    pub collections: u64,
    pub memberships: u64,
    pub issues: u64,
}
fn finite(value: Option<f64>, name: &str) -> Result<()> {
    ensure!(value.is_none_or(f64::is_finite), "Nonfinite {name}");
    Ok(())
}
fn positive(value: Option<f64>, name: &str) -> Result<()> {
    finite(value, name)?;
    ensure!(value.is_none_or(|v| v > 0.0), "Invalid {name}");
    Ok(())
}
fn fingerprint(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn validate_track(track: &TrackObservation, limits: &ImportLimits) -> Result<()> {
    ensure!(!track.external_id.is_empty(), "Empty track ID");
    if let Some(bpm) = &track.bpm {
        positive(bpm.value, "BPM")?;
    }
    if let Some(key) = &track.key {
        if let Some(camelot) = &key.camelot {
            ensure!(camelot.is_ascii(), "Invalid normalized Camelot key");
            let (number, mode) = camelot.split_at(camelot.len().saturating_sub(1));
            let number: usize = number.parse().context("Invalid normalized Camelot key")?;
            ensure!(
                (1..=12).contains(&number) && matches!(mode, "A" | "B"),
                "Invalid normalized Camelot key"
            );
            let pitches = if mode == "A" {
                [8, 3, 10, 5, 0, 7, 2, 9, 4, 11, 6, 1]
            } else {
                [11, 6, 1, 8, 3, 10, 5, 0, 7, 2, 9, 4]
            };
            ensure!(
                key.pitch_class.is_none_or(|p| p == pitches[number - 1]),
                "Key pitch class conflicts with Camelot"
            );
            ensure!(
                key.mode
                    .as_ref()
                    .is_none_or(|m| m == if mode == "A" { "minor" } else { "major" }),
                "Key mode conflicts with Camelot"
            );
        }
    }
    let mut ids = BTreeSet::new();
    for cue in &track.cues {
        ensure!(
            !cue.external_id.is_empty() && ids.insert(&cue.external_id),
            "Empty or duplicate cue ID"
        );
        for value in [
            cue.native_start_seconds,
            cue.native_end_seconds,
            cue.canonical_start_seconds,
            cue.canonical_end_seconds,
        ] {
            finite(value, "cue time")?;
        }
        if cue.clock_status == ClockStatus::UnresolvedNative {
            ensure!(
                cue.canonical_start_seconds.is_none() && cue.canonical_end_seconds.is_none(),
                "unresolved native cue has canonical values"
            );
        }
        if let (Some(start), Some(end)) = (cue.native_start_seconds, cue.native_end_seconds) {
            ensure!(end >= start, "Cue range ends before start");
        }
        ensure!(
            cue.color_rgb.is_none_or(|c| c <= 0xffffff),
            "Invalid cue RGB"
        );
        ensure!(
            cue.source_slot.is_none_or(|slot| slot >= 0),
            "Cue slot sentinel must be absent"
        );
    }
    if let Some(grid) = &track.grid {
        positive(grid.bpm, "grid BPM")?;
        positive(grid.sample_rate, "sample rate")?;
        finite(grid.native_first_beat_seconds, "grid origin")?;
        finite(grid.canonical_first_beat_seconds, "canonical grid origin")?;
        finite(grid.offset_seconds, "clock offset")?;
        if grid.clock_status == ClockStatus::UnresolvedNative {
            ensure!(
                grid.canonical_first_beat_seconds.is_none()
                    && grid.points.iter().all(|p| p.canonical_seconds.is_none()),
                "unresolved native grid has canonical values"
            );
        } else {
            ensure!(
                grid.timing_profile_id
                    .as_ref()
                    .is_some_and(|s| !s.is_empty()),
                "Canonical grid requires timing profile"
            );
        }
        let mut previous = None;
        for point in &grid.points {
            finite(Some(point.native_seconds), "grid time")?;
            finite(point.canonical_seconds, "canonical grid time")?;
            positive(point.bpm, "grid BPM")?;
            ensure!(
                previous.is_none_or(|p| point.native_seconds > p),
                "Grid points must increase"
            );
            previous = Some(point.native_seconds);
        }
    }
    for blob in track
        .file_tags
        .iter()
        .chain(track.grid.as_ref().and_then(|g| g.raw.as_ref()))
    {
        ensure!(
            fingerprint(&blob.sha256) && blob.bytes <= limits.blob_bytes,
            "Invalid blob identity or size"
        );
    }
    Ok(())
}
fn insert_member(
    db: &Connection,
    collection: &str,
    m: &MembershipObservation,
    report: &mut ValidationReport,
) -> Result<()> {
    ensure!(
        !m.external_occurrence_id.is_empty() && !m.track_external_id.is_empty(),
        "Empty membership identity"
    );
    db.execute(
        "INSERT INTO members(collection,id,track) VALUES(?1,?2,?3)",
        params![collection, m.external_occurrence_id, m.track_external_id],
    )
    .context("duplicate occurrence ID")?;
    report.memberships += 1;
    Ok(())
}
/// Validate semantic identity/graph/timing independently of the envelope schema.
/// Indexes live in a private disk database, bounding memory independently of library size.
pub fn validate_observation(
    header: &ObservationHeader,
    records: &mut dyn ObservationReader,
    limits: &ImportLimits,
) -> Result<ValidationReport> {
    ensure!(
        header.contract_version == 1,
        "Unsupported observation contract"
    );
    ensure!(
        !header.source.library_instance_id.is_empty(),
        "Empty source identity"
    );
    ensure!(
        fingerprint(&header.schema_fingerprint) && fingerprint(&header.snapshot_hash),
        "Invalid source fingerprint"
    );
    ensure!(
        limits.page_size > 0 && limits.hierarchy_depth > 0,
        "Invalid import limits"
    );
    let file = tempfile::NamedTempFile::new()?;
    let db = Connection::open(file.path())?;
    db.execute_batch(
        "PRAGMA cache_size=-2048; PRAGMA temp_store=FILE;
        CREATE TABLE tracks(id TEXT PRIMARY KEY);
        CREATE TABLE collections(id TEXT PRIMARY KEY,parent TEXT);
        CREATE TABLE members(collection TEXT,id TEXT,track TEXT,PRIMARY KEY(collection,id));
        BEGIN;",
    )?;
    let mut report = ValidationReport::default();
    while let Some(record) = records.next()? {
        match record {
            ObservationRecord::Track(track) => {
                validate_track(&track, limits)?;
                if header.completeness != Completeness::WholeLibrary {
                    ensure!(
                        !track
                            .metadata
                            .values()
                            .chain(track.cached_tags.values())
                            .any(|v| matches!(v, Presence::Deleted)),
                        "Deletion requires complete authoritative scope"
                    );
                }
                db.execute("INSERT INTO tracks VALUES(?1)", [&track.external_id])
                    .context("duplicate track ID")?;
                report.tracks += 1;
            }
            ObservationRecord::Collection(c) => {
                ensure!(!c.external_id.is_empty(), "Empty collection ID");
                db.execute(
                    "INSERT INTO collections VALUES(?1,?2)",
                    params![c.external_id, c.parent_id],
                )
                .context("duplicate collection ID")?;
                for member in c.occurrences {
                    insert_member(&db, &c.external_id, &member, &mut report)?;
                }
                report.collections += 1;
            }
            ObservationRecord::Membership {
                collection_id,
                occurrence,
            } => insert_member(&db, &collection_id, &occurrence, &mut report)?,
            ObservationRecord::Issue(_) => report.issues += 1,
        }
    }
    let dangling: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM members m LEFT JOIN tracks t ON t.id=m.track LEFT JOIN collections c ON c.id=m.collection WHERE t.id IS NULL OR c.id IS NULL)",[],|r|r.get(0))?;
    ensure!(
        !dangling,
        "Membership references unknown track or collection"
    );
    let mut query = db.prepare("SELECT id,parent FROM collections ORDER BY id")?;
    let mut rows = query.query([])?;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let mut parent: Option<String> = row.get(1)?;
        let mut ancestors = BTreeSet::from([id]);
        while let Some(id) = parent {
            ensure!(ancestors.insert(id.clone()), "Collection parent cycle");
            ensure!(
                ancestors.len() <= limits.hierarchy_depth,
                "Collection hierarchy depth limit exceeded"
            );
            let found: Option<Option<String>> = db
                .query_row("SELECT parent FROM collections WHERE id=?1", [&id], |r| {
                    r.get(0)
                })
                .optional()?;
            parent = found.with_context(|| "Unknown collection parent")?;
        }
    }
    for (expected, actual, kind) in [
        (header.accounting.tracks_seen, report.tracks, "track"),
        (
            header.accounting.collections_seen,
            report.collections,
            "collection",
        ),
        (
            header.accounting.memberships_seen,
            report.memberships,
            "membership",
        ),
    ] {
        if let Some(expected) = expected {
            ensure!(actual == expected, "Incomplete {kind} accounting");
        }
    }
    if header.accounting.unrepresented_records > 0
        && header.completeness != Completeness::PartialWithErrors
    {
        bail!("Unrepresented source records require partial completeness");
    }
    db.execute_batch("COMMIT")?;
    Ok(report)
}
