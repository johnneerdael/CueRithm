//! CueRithm playlists as Mixxx playlists or crates, with local ownership evidence.
//! Mixxx has no folders and no smart containers, so folders flatten to a path-like
//! name and smart playlists export their captured members.
use crate::{
    MixxxContainer, MixxxExistingTracks, SyncLibrary, SyncOptions, SyncPlaylist,
    ownership::{
        PlaylistBinding, PlaylistOwnership, PlaylistOwnershipReport, PlaylistOwnershipRequest,
        fingerprint,
    },
};
use anyhow::{Context, Result, bail, ensure};
use rusqlite::{Connection, Transaction, params};
use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

pub(super) const IDENTITY: &str = "mixxx:mixxxdb.sqlite";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Playlist,
    Crate,
}
impl Kind {
    fn tag(self) -> &'static str {
        match self {
            Kind::Playlist => "playlist",
            Kind::Crate => "crate",
        }
    }
    fn noun(self) -> &'static str {
        self.tag()
    }
}

/// `playlist:<id>` or `crate:<id>`; Mixxx ids are autoincrement integers.
fn locator(kind: Kind, id: i64) -> String {
    format!("{}:{id}", kind.tag())
}
fn parse_locator(text: &str) -> Option<(Kind, i64)> {
    let (kind, id) = text.split_once(':')?;
    let kind = match kind {
        "playlist" => Kind::Playlist,
        "crate" => Kind::Crate,
        _ => return None,
    };
    (!id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
        .then(|| id.parse().ok())
        .flatten()
        .map(|id| (kind, id))
}

/// A Mixxx playlist or crate as it is now.
#[derive(Debug, Clone)]
struct Native {
    name: String,
    locked: bool,
    /// Auto DJ and history playlists; never touched.
    hidden: bool,
    /// Playlists keep order and duplicates; crates are sorted and unique.
    members: Vec<i64>,
}
impl Native {
    fn print(&self, kind: Kind) -> Result<String> {
        fingerprint(&(kind.tag(), &self.name, &self.members))
    }
}

fn load(connection: &Connection, kind: Kind) -> Result<BTreeMap<i64, Native>> {
    let mut found = BTreeMap::new();
    match kind {
        Kind::Playlist => {
            let mut statement = connection.prepare(
                "SELECT id, COALESCE(name, ''), COALESCE(locked, 0), hidden FROM Playlists",
            )?;
            for row in statement.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    Native {
                        name: row.get(1)?,
                        locked: row.get::<_, i64>(2)? != 0,
                        hidden: row.get::<_, i64>(3)? != 0,
                        members: Vec::new(),
                    },
                ))
            })? {
                let (id, native) = row?;
                found.insert(id, native);
            }
            let mut statement = connection.prepare(
                "SELECT playlist_id, track_id FROM PlaylistTracks ORDER BY playlist_id, position, id",
            )?;
            for row in
                statement.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))?
            {
                let (playlist, track) = row?;
                if let Some(native) = found.get_mut(&playlist) {
                    native.members.push(track);
                }
            }
        }
        Kind::Crate => {
            let mut statement =
                connection.prepare("SELECT id, name, COALESCE(locked, 0) FROM crates")?;
            for row in statement.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    Native {
                        name: row.get(1)?,
                        locked: row.get::<_, i64>(2)? != 0,
                        hidden: false,
                        members: Vec::new(),
                    },
                ))
            })? {
                let (id, native) = row?;
                found.insert(id, native);
            }
            let mut statement = connection.prepare(
                "SELECT crate_id, track_id FROM crate_tracks ORDER BY crate_id, track_id",
            )?;
            for row in
                statement.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))?
            {
                let (container, track) = row?;
                if let Some(native) = found.get_mut(&container) {
                    native.members.push(track);
                }
            }
        }
    }
    Ok(found)
}

/// One CueRithm playlist as it should appear in Mixxx.
struct Wanted {
    source_id: String,
    parent_id: Option<String>,
    name: String,
    members: Vec<i64>,
}

fn flat_name(library: &SyncLibrary, playlist: &SyncPlaylist) -> Result<String> {
    let mut names = vec![playlist.name.trim().to_owned()];
    let mut seen = BTreeSet::from([playlist.id.as_str()]);
    let mut parent = playlist.parent_id.as_deref();
    while let Some(id) = parent {
        ensure!(seen.insert(id), "Playlist folders contain a cycle");
        let Some(folder) = library
            .playlists
            .iter()
            .find(|candidate| candidate.id == id)
        else {
            break;
        };
        names.push(folder.name.trim().to_owned());
        parent = folder.parent_id.as_deref();
    }
    names.reverse();
    names.retain(|name| !name.is_empty());
    Ok(if names.is_empty() {
        "Untitled playlist".to_owned()
    } else {
        names.join(" / ")
    })
}

fn plan(
    library: &SyncLibrary,
    kind: Kind,
    library_ids: &BTreeMap<String, i64>,
) -> Result<Vec<Wanted>> {
    let mut used = BTreeSet::new();
    let mut wanted = Vec::new();
    for playlist in library.playlists.iter().filter(|playlist| !playlist.folder) {
        let base = flat_name(library, playlist)?;
        let mut name = base.clone();
        for suffix in 2.. {
            if used.insert(name.clone()) {
                break;
            }
            name = format!("{base} ({suffix})");
        }
        let mut members = Vec::new();
        for track in &playlist.track_ids {
            members.push(*library_ids.get(track).with_context(|| {
                format!(
                    "Playlist '{}' contains a track that is not part of this sync",
                    playlist.name
                )
            })?);
        }
        if kind == Kind::Crate {
            let mut seen = BTreeSet::new();
            members.retain(|id| seen.insert(*id));
            members.sort_unstable();
        }
        wanted.push(Wanted {
            source_id: playlist.id.clone(),
            parent_id: playlist.parent_id.clone(),
            name,
            members,
        });
    }
    Ok(wanted)
}

fn create(tx: &Transaction<'_>, kind: Kind, name: &str, members: &[i64]) -> Result<i64> {
    let id = match kind {
        Kind::Playlist => {
            let position: i64 = tx.query_row(
                "SELECT COALESCE(MAX(position), 0) + 1 FROM Playlists",
                [],
                |row| row.get(0),
            )?;
            tx.execute(
                "INSERT INTO Playlists(name, position, hidden, date_created, date_modified)
                 VALUES(?1, ?2, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
                params![name, position],
            )?;
            tx.last_insert_rowid()
        }
        Kind::Crate => {
            tx.execute(
                "INSERT INTO crates(name, locked, autodj_source) VALUES(?1, 0, 0)",
                [name],
            )?;
            tx.last_insert_rowid()
        }
    };
    insert_members(tx, kind, id, 0, members)?;
    Ok(id)
}

/// Append `members` after `existing` entries (playlists number from 1).
fn insert_members(
    tx: &Transaction<'_>,
    kind: Kind,
    id: i64,
    existing: usize,
    members: &[i64],
) -> Result<()> {
    for (offset, track) in members.iter().enumerate() {
        match kind {
            Kind::Playlist => tx.execute(
                "INSERT INTO PlaylistTracks(playlist_id, track_id, position, pl_datetime_added)
                 VALUES(?1, ?2, ?3, CURRENT_TIMESTAMP)",
                params![id, track, (existing + offset + 1) as i64],
            )?,
            Kind::Crate => tx.execute(
                "INSERT OR IGNORE INTO crate_tracks(crate_id, track_id) VALUES(?1, ?2)",
                params![id, track],
            )?,
        };
    }
    Ok(())
}

fn touch(tx: &Transaction<'_>, kind: Kind, id: i64) -> Result<()> {
    if kind == Kind::Playlist {
        tx.execute(
            "UPDATE Playlists SET date_modified = CURRENT_TIMESTAMP WHERE id = ?1",
            [id],
        )?;
    }
    Ok(())
}

/// Make the object exactly CueRithm's version. Returns whether anything changed.
fn set_exact(
    tx: &Transaction<'_>,
    kind: Kind,
    id: i64,
    native: &Native,
    want: &Wanted,
    warnings: &mut Vec<String>,
) -> Result<bool> {
    let mut changed = false;
    if native.name != want.name {
        let taken = kind == Kind::Crate
            && tx.query_row(
                "SELECT COUNT(*) FROM crates WHERE name = ?1 AND id != ?2",
                params![want.name, id],
                |row| row.get::<_, i64>(0),
            )? > 0;
        if taken {
            warnings.push(format!(
                "Crate '{}' could not be renamed to '{}': that name is in use; it keeps its name for now",
                native.name, want.name
            ));
        } else {
            let sql = match kind {
                Kind::Playlist => "UPDATE Playlists SET name = ?1 WHERE id = ?2",
                Kind::Crate => "UPDATE crates SET name = ?1 WHERE id = ?2",
            };
            tx.execute(sql, params![want.name, id])?;
            changed = true;
        }
    }
    if native.members != want.members {
        match kind {
            Kind::Playlist => {
                tx.execute("DELETE FROM PlaylistTracks WHERE playlist_id = ?1", [id])?;
                insert_members(tx, kind, id, 0, &want.members)?;
            }
            Kind::Crate => {
                tx.execute("DELETE FROM crate_tracks WHERE crate_id = ?1", [id])?;
                insert_members(tx, kind, id, 0, &want.members)?;
            }
        }
        changed = true;
    }
    if changed {
        touch(tx, kind, id)?;
    }
    Ok(changed)
}

/// Add the members the object lacks, after what it already holds. Nothing is removed,
/// renamed or reordered.
fn append_missing(
    tx: &Transaction<'_>,
    kind: Kind,
    id: i64,
    native: &Native,
    want: &Wanted,
) -> Result<bool> {
    let present: BTreeSet<i64> = native.members.iter().copied().collect();
    let mut seen = BTreeSet::new();
    let missing: Vec<i64> = want
        .members
        .iter()
        .copied()
        .filter(|track| !present.contains(track) && seen.insert(*track))
        .collect();
    if missing.is_empty() {
        return Ok(false);
    }
    // Positions can have gaps after edits in Mixxx; continue after the highest one.
    let base = match kind {
        Kind::Playlist => tx.query_row(
            "SELECT COALESCE(MAX(position), 0) FROM PlaylistTracks WHERE playlist_id = ?1",
            [id],
            |row| row.get::<_, i64>(0),
        )? as usize,
        Kind::Crate => 0,
    };
    insert_members(tx, kind, id, base, &missing)?;
    touch(tx, kind, id)?;
    Ok(true)
}

fn delete(tx: &Transaction<'_>, kind: Kind, id: i64) -> Result<()> {
    match kind {
        Kind::Playlist => {
            tx.execute("DELETE FROM PlaylistTracks WHERE playlist_id = ?1", [id])?;
            tx.execute("DELETE FROM Playlists WHERE id = ?1", [id])?;
        }
        Kind::Crate => {
            tx.execute("DELETE FROM crate_tracks WHERE crate_id = ?1", [id])?;
            tx.execute("DELETE FROM crates WHERE id = ?1", [id])?;
        }
    }
    Ok(())
}

fn validate(request: &PlaylistOwnershipRequest) -> Result<BTreeMap<String, PlaylistBinding>> {
    ensure!(
        request.previous.identity.is_empty() || request.previous.identity == IDENTITY,
        "Mixxx ownership target identity changed"
    );
    ensure!(
        !request.previous.identity.is_empty() || request.previous.bindings.is_empty(),
        "Mixxx ownership evidence has no target identity"
    );
    let mut previous = BTreeMap::new();
    let mut locators = BTreeSet::new();
    for binding in &request.previous.bindings {
        ensure!(
            !binding.source_id.is_empty()
                && previous
                    .insert(binding.source_id.clone(), binding.clone())
                    .is_none(),
            "Duplicate Mixxx source ownership identity"
        );
        ensure!(
            parse_locator(&binding.locator).is_some() && locators.insert(binding.locator.clone()),
            "Invalid or duplicate Mixxx ownership locator"
        );
        ensure!(
            !binding.fingerprint.is_empty(),
            "Mixxx binding has no structural fingerprint"
        );
    }
    Ok(previous)
}

/// What a container pass did, for the preview.
#[derive(Debug)]
pub(super) struct ContainerOutcome {
    pub report: PlaylistOwnershipReport,
    pub created: usize,
    pub updated: usize,
    pub deleted: usize,
}

struct Cache {
    maps: BTreeMap<Kind, BTreeMap<i64, Native>>,
}
impl Cache {
    fn get(&mut self, connection: &Connection, kind: Kind) -> Result<&mut BTreeMap<i64, Native>> {
        Ok(match self.maps.entry(kind) {
            Entry::Vacant(slot) => slot.insert(load(connection, kind)?),
            Entry::Occupied(slot) => slot.into_mut(),
        })
    }
}

pub(super) fn write_containers(
    tx: &Transaction<'_>,
    library: &SyncLibrary,
    options: &SyncOptions,
    request: &PlaylistOwnershipRequest,
    library_ids: &BTreeMap<String, i64>,
    warnings: &mut Vec<String>,
) -> Result<ContainerOutcome> {
    let previous = validate(request)?;
    let kind = match options.mixxx.container {
        MixxxContainer::Playlists => Kind::Playlist,
        MixxxContainer::Crates => Kind::Crate,
    };
    let replace = options.mixxx.existing_tracks == MixxxExistingTracks::Replace;
    let wanted = plan(library, kind, library_ids)?;
    let active: BTreeSet<&str> = library.playlists.iter().map(|p| p.id.as_str()).collect();
    let mut cache = Cache {
        maps: BTreeMap::new(),
    };
    let mut bindings: BTreeMap<String, PlaylistBinding> = previous.clone();
    let mut deleted = BTreeSet::new();
    let mut retired = BTreeSet::new();
    let mut outcome = (0usize, 0usize, 0usize); // created, updated, deleted

    // Removals first, so a deleted object can neither be adopted nor block a name.
    for source in &request.removed {
        ensure!(
            !active.contains(source.as_str()),
            "Mixxx playlist is both active and removed"
        );
        let binding = previous
            .get(source)
            .context("Mixxx removal has no acknowledged ownership binding")?;
        let (bound_kind, id) = parse_locator(&binding.locator).context("Invalid Mixxx locator")?;
        bindings.remove(source);
        if !binding.created {
            retired.insert(source.clone());
            continue;
        }
        let natives = cache.get(tx, bound_kind)?;
        match natives.get(&id) {
            None => {
                deleted.insert(source.clone());
            }
            Some(native)
                if !native.locked
                    && !native.hidden
                    && native.print(bound_kind)? == binding.fingerprint =>
            {
                delete(tx, bound_kind, id)?;
                natives.remove(&id);
                deleted.insert(source.clone());
                outcome.2 += 1;
            }
            Some(native) => {
                warnings.push(format!(
                    "Mixxx {} '{}' was changed or locked after CueRithm created it, so it was kept",
                    bound_kind.noun(),
                    native.name
                ));
                retired.insert(source.clone());
            }
        }
    }

    let bound_here: BTreeSet<String> = bindings.values().map(|b| b.locator.clone()).collect();
    let mut claimed: BTreeSet<i64> = BTreeSet::new();
    for want in &wanted {
        let natives = cache.get(tx, kind)?;
        let existing_binding = previous.get(&want.source_id);
        let mut created = true;
        let mut id = None;
        match existing_binding.and_then(|b| parse_locator(&b.locator).map(|(k, i)| (b, k, i))) {
            Some((binding, bound_kind, bound_id)) if bound_kind == kind => {
                created = binding.created;
                match natives.get(&bound_id).cloned() {
                    Some(native) if native.hidden || native.locked => {
                        warnings.push(format!(
                            "Mixxx {} '{}' is locked, so it was left unchanged",
                            kind.noun(),
                            native.name
                        ));
                        id = Some(bound_id);
                    }
                    Some(native) => {
                        id = Some(bound_id);
                        let unchanged = native.print(kind)? == binding.fingerprint;
                        if unchanged || replace {
                            if !unchanged {
                                warnings.push(format!(
                                    "Mixxx {} '{}' was edited in Mixxx and was reset to CueRithm's version",
                                    kind.noun(),
                                    native.name
                                ));
                            }
                            if set_exact(tx, kind, bound_id, &native, want, warnings)? {
                                outcome.1 += 1;
                            }
                        } else {
                            warnings.push(format!(
                                "Mixxx {} '{}' was edited in Mixxx, so only missing tracks were added",
                                kind.noun(),
                                native.name
                            ));
                            if append_missing(tx, kind, bound_id, &native, want)? {
                                outcome.1 += 1;
                            }
                        }
                    }
                    None => {
                        warnings.push(format!(
                            "Mixxx {} '{}' was removed in Mixxx and was created again",
                            kind.noun(),
                            want.name
                        ));
                        created = true;
                    }
                }
            }
            Some((_, other, _)) => {
                warnings.push(format!(
                    "'{}' was exported as a Mixxx {} before; that one was kept and a {} was created",
                    want.name,
                    other.noun(),
                    kind.noun()
                ));
            }
            None => {
                let candidates: Vec<i64> = natives
                    .iter()
                    .filter(|(candidate, native)| {
                        !native.hidden
                            && native.name == want.name
                            && !bound_here.contains(&locator(kind, **candidate))
                            && !claimed.contains(*candidate)
                    })
                    .map(|(candidate, _)| *candidate)
                    .collect();
                match candidates.as_slice() {
                    [adopted] => {
                        let native = natives[adopted].clone();
                        id = Some(*adopted);
                        created = false;
                        if native.locked {
                            warnings.push(format!(
                                "Mixxx {} '{}' is locked, so it was left unchanged",
                                kind.noun(),
                                native.name
                            ));
                        } else if replace {
                            if set_exact(tx, kind, *adopted, &native, want, warnings)? {
                                outcome.1 += 1;
                            }
                        } else if append_missing(tx, kind, *adopted, &native, want)? {
                            outcome.1 += 1;
                        }
                    }
                    [] => {}
                    several => {
                        warnings.push(format!(
                            "{} Mixxx playlists are named '{}'; a separate one was created",
                            several.len(),
                            want.name
                        ));
                    }
                }
            }
        }
        let id = match id {
            Some(id) => id,
            None => {
                outcome.0 += 1;
                create(tx, kind, &want.name, &want.members)?
            }
        };
        claimed.insert(id);
        bindings.insert(
            want.source_id.clone(),
            PlaylistBinding {
                source_id: want.source_id.clone(),
                locator: locator(kind, id),
                parent_source_id: want.parent_id.clone(),
                created,
                // Replaced below from the final database state.
                fingerprint: String::new(),
            },
        );
    }

    // Fingerprints come from what is actually stored once every change is made.
    let mut final_state: BTreeMap<Kind, BTreeMap<i64, Native>> = BTreeMap::new();
    for want in &wanted {
        let binding = bindings.get_mut(&want.source_id).expect("bound above");
        let (bound_kind, id) = parse_locator(&binding.locator).context("Invalid Mixxx locator")?;
        if let Entry::Vacant(slot) = final_state.entry(bound_kind) {
            slot.insert(load(tx, bound_kind)?);
        }
        let native = final_state[&bound_kind]
            .get(&id)
            .context("A Mixxx container vanished during the sync")?;
        binding.fingerprint = native.print(bound_kind)?;
    }
    if bindings.values().any(|b| b.fingerprint.is_empty()) {
        bail!("Mixxx ownership report is incomplete");
    }
    Ok(ContainerOutcome {
        report: PlaylistOwnershipReport {
            current: PlaylistOwnership {
                identity: IDENTITY.into(),
                bindings: bindings.into_values().collect(),
            },
            deleted,
            retired,
        },
        created: outcome.0,
        updated: outcome.1,
        deleted: outcome.2,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MixxxOptions, mixxx::fixture};
    use cuerithm_smartlists::SmartDefinition;

    struct Setup {
        _directory: tempfile::TempDir,
        connection: Connection,
        tracks: BTreeMap<String, i64>,
    }
    fn setup() -> Setup {
        let directory = tempfile::tempdir().unwrap();
        let connection = fixture::create(&directory.path().join("mixxxdb.sqlite"), 39);
        let tracks = ["a", "b", "c"]
            .iter()
            .map(|id| {
                (
                    (*id).to_owned(),
                    fixture::add_track(&connection, &format!("/music/{id}.mp3"), id),
                )
            })
            .collect();
        Setup {
            _directory: directory,
            connection,
            tracks,
        }
    }
    fn pl(
        id: &str,
        parent: Option<&str>,
        name: &str,
        folder: bool,
        tracks: &[&str],
    ) -> SyncPlaylist {
        SyncPlaylist {
            id: id.into(),
            parent_id: parent.map(str::to_owned),
            name: name.into(),
            folder,
            track_ids: tracks.iter().map(|t| (*t).to_owned()).collect(),
            smart: None,
        }
    }
    fn opts(container: MixxxContainer, mode: MixxxExistingTracks) -> SyncOptions {
        SyncOptions {
            mixxx: MixxxOptions {
                container,
                existing_tracks: mode,
                ..MixxxOptions::default()
            },
            ..SyncOptions::default()
        }
    }
    fn request(previous: &PlaylistOwnershipReport, removed: &[&str]) -> PlaylistOwnershipRequest {
        PlaylistOwnershipRequest {
            previous: previous.current.clone(),
            removed: removed.iter().map(|s| (*s).to_owned()).collect(),
        }
    }
    impl Setup {
        fn run(
            &mut self,
            playlists: Vec<SyncPlaylist>,
            options: &SyncOptions,
            request: &PlaylistOwnershipRequest,
        ) -> Result<(ContainerOutcome, Vec<String>)> {
            let library = SyncLibrary {
                tracks: vec![],
                playlists,
            };
            let mut warnings = Vec::new();
            let tx = self.connection.transaction()?;
            let outcome =
                write_containers(&tx, &library, options, request, &self.tracks, &mut warnings)?;
            tx.commit()?;
            Ok((outcome, warnings))
        }
        fn id(&self, track: &str) -> i64 {
            self.tracks[track]
        }
        fn playlists(&self) -> Vec<(String, Vec<i64>)> {
            load(&self.connection, Kind::Playlist)
                .unwrap()
                .into_values()
                .map(|n| (n.name, n.members))
                .collect()
        }
        fn crates(&self) -> Vec<(String, Vec<i64>)> {
            load(&self.connection, Kind::Crate)
                .unwrap()
                .into_values()
                .map(|n| (n.name, n.members))
                .collect()
        }
        fn binding<'a>(&self, outcome: &'a ContainerOutcome, source: &str) -> &'a PlaylistBinding {
            outcome
                .report
                .current
                .bindings
                .iter()
                .find(|b| b.source_id == source)
                .unwrap()
        }
    }
    fn none() -> PlaylistOwnershipRequest {
        PlaylistOwnershipRequest::default()
    }
    const PLAYLISTS: MixxxContainer = MixxxContainer::Playlists;
    const KEEP: MixxxExistingTracks = MixxxExistingTracks::Keep;
    const REPLACE: MixxxExistingTracks = MixxxExistingTracks::Replace;

    #[test]
    fn new_playlists_are_ordered_and_flattened_with_unique_names() {
        let mut s = setup();
        let (a, b) = (s.id("a"), s.id("b"));
        let (outcome, _) = s
            .run(
                vec![
                    pl("f", None, "House", true, &[]),
                    pl("p1", Some("f"), "Peak", false, &["a", "b", "a"]),
                    pl("p2", Some("f"), "Peak", false, &["b"]),
                    pl("p3", None, "  ", false, &["c"]),
                ],
                &opts(PLAYLISTS, KEEP),
                &none(),
            )
            .unwrap();
        assert_eq!(
            s.playlists(),
            vec![
                ("House / Peak".into(), vec![a, b, a]),
                ("House / Peak (2)".into(), vec![b]),
                ("Untitled playlist".into(), vec![s.id("c")]),
            ]
        );
        assert_eq!(outcome.created, 3);
        assert!(s.crates().is_empty(), "folders emit nothing");
        let positions: Vec<i64> =
            fixture::dump(&s.connection, "SELECT position FROM Playlists ORDER BY id")
                .into_iter()
                .map(|r| match r[0] {
                    rusqlite::types::Value::Integer(i) => i,
                    _ => panic!(),
                })
                .collect();
        assert_eq!(positions, vec![1, 2, 3]);
        let order: Vec<i64> = fixture::dump(
            &s.connection,
            "SELECT position FROM PlaylistTracks WHERE playlist_id = 1 ORDER BY position",
        )
        .into_iter()
        .map(|r| match r[0] {
            rusqlite::types::Value::Integer(i) => i,
            _ => panic!(),
        })
        .collect();
        assert_eq!(order, vec![1, 2, 3]);
    }

    #[test]
    fn new_playlists_continue_after_the_existing_position() {
        let mut s = setup();
        fixture::add_playlist(&s.connection, "Mine", &[]);
        s.run(
            vec![pl("p", None, "New", false, &["a"])],
            &opts(PLAYLISTS, KEEP),
            &none(),
        )
        .unwrap();
        let positions = fixture::dump(&s.connection, "SELECT position FROM Playlists ORDER BY id");
        assert_eq!(
            positions,
            vec![
                vec![rusqlite::types::Value::Integer(1)],
                vec![rusqlite::types::Value::Integer(2)]
            ]
        );
    }

    #[test]
    fn crates_dedupe_members_and_ignore_order() {
        let mut s = setup();
        let (a, b) = (s.id("a"), s.id("b"));
        s.run(
            vec![pl("p", None, "Crate One", false, &["b", "a", "b"])],
            &opts(MixxxContainer::Crates, KEEP),
            &none(),
        )
        .unwrap();
        assert_eq!(s.crates(), vec![("Crate One".into(), vec![a, b])]);
        assert!(s.playlists().is_empty());
    }

    #[test]
    fn smart_playlist_exports_captured_members() {
        let mut s = setup();
        let mut smart = pl("p", None, "Smart", false, &["a", "c"]);
        smart.smart = Some(SmartDefinition {
            match_all: true,
            rules: vec![],
        });
        s.run(vec![smart], &opts(PLAYLISTS, KEEP), &none()).unwrap();
        assert_eq!(
            s.playlists(),
            vec![("Smart".into(), vec![s.id("a"), s.id("c")])]
        );
    }

    #[test]
    fn rename_and_reorder_update_the_bound_object_in_place() {
        let mut s = setup();
        let (first, _) = s
            .run(
                vec![pl("p", None, "Old", false, &["a", "b"])],
                &opts(PLAYLISTS, KEEP),
                &none(),
            )
            .unwrap();
        let id_before = fixture::dump(&s.connection, "SELECT id FROM Playlists");
        let (second, _) = s
            .run(
                vec![pl("p", None, "New", false, &["b", "c", "a"])],
                &opts(PLAYLISTS, KEEP),
                &request(&first.report, &[]),
            )
            .unwrap();
        assert_eq!(
            fixture::dump(&s.connection, "SELECT id FROM Playlists"),
            id_before
        );
        assert_eq!(
            s.playlists(),
            vec![("New".into(), vec![s.id("b"), s.id("c"), s.id("a")])]
        );
        assert_eq!(second.updated, 1);
        assert!(s.binding(&second, "p").created);
        assert_ne!(
            s.binding(&second, "p").fingerprint,
            s.binding(&first, "p").fingerprint
        );
    }

    #[test]
    fn unchanged_second_run_writes_nothing() {
        let mut s = setup();
        let lists = vec![
            pl("p", None, "Same", false, &["a", "b"]),
            pl("q", None, "Other", false, &["c"]),
        ];
        let (first, _) = s
            .run(lists.clone(), &opts(PLAYLISTS, REPLACE), &none())
            .unwrap();
        let changes = s.connection.total_changes();
        let (second, warnings) = s
            .run(
                lists,
                &opts(PLAYLISTS, REPLACE),
                &request(&first.report, &[]),
            )
            .unwrap();
        assert_eq!(s.connection.total_changes(), changes, "no row was written");
        assert_eq!((second.created, second.updated, second.deleted), (0, 0, 0));
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            first.report.current.bindings.len(),
            second.report.current.bindings.len()
        );
    }

    #[test]
    fn owned_edit_in_mixxx_is_reset_in_replace_and_appended_in_keep() {
        for (mode, expected_name, expected) in [
            (REPLACE, "Set", vec!["a", "b"]),
            (KEEP, "Renamed by user", vec!["c", "a", "b"]),
        ] {
            let mut s = setup();
            let (first, _) = s
                .run(
                    vec![pl("p", None, "Set", false, &["a", "b"])],
                    &opts(PLAYLISTS, KEEP),
                    &none(),
                )
                .unwrap();
            // The user renames it, drops "b" and adds "c" in Mixxx.
            s.connection
                .execute("UPDATE Playlists SET name = 'Renamed by user'", [])
                .unwrap();
            s.connection
                .execute(
                    "DELETE FROM PlaylistTracks WHERE track_id = ?1",
                    [s.id("b")],
                )
                .unwrap();
            s.connection
                .execute(
                    "INSERT INTO PlaylistTracks(playlist_id, track_id, position) VALUES(1, ?1, 9)",
                    [s.id("c")],
                )
                .unwrap();
            let (second, warnings) = s
                .run(
                    vec![pl("p", None, "Set", false, &["a", "b"])],
                    &opts(PLAYLISTS, mode),
                    &request(&first.report, &[]),
                )
                .unwrap();
            let want: Vec<i64> = expected.iter().map(|t| s.id(t)).collect();
            // Keep appends only what is missing, after what the user left.
            let want = if mode == KEEP {
                vec![s.id("a"), s.id("c"), s.id("b")]
            } else {
                want
            };
            assert_eq!(
                s.playlists(),
                vec![(expected_name.into(), want)],
                "{mode:?}"
            );
            assert!(
                warnings.iter().any(|w| w.contains("edited in Mixxx")),
                "{warnings:?}"
            );
            assert!(s.binding(&second, "p").created);
        }
    }

    #[test]
    fn deleted_owned_playlist_is_recreated() {
        let mut s = setup();
        let (first, _) = s
            .run(
                vec![pl("p", None, "Set", false, &["a"])],
                &opts(PLAYLISTS, KEEP),
                &none(),
            )
            .unwrap();
        s.connection
            .execute("DELETE FROM PlaylistTracks", [])
            .unwrap();
        s.connection.execute("DELETE FROM Playlists", []).unwrap();
        let (second, warnings) = s
            .run(
                vec![pl("p", None, "Set", false, &["a"])],
                &opts(PLAYLISTS, KEEP),
                &request(&first.report, &[]),
            )
            .unwrap();
        assert_eq!(s.playlists(), vec![("Set".into(), vec![s.id("a")])]);
        assert!(s.binding(&second, "p").created);
        assert!(
            warnings.iter().any(|w| w.contains("created again")),
            "{warnings:?}"
        );
    }

    #[test]
    fn same_name_foreign_playlist_is_adopted_replace_vs_keep() {
        for (mode, expected) in [(REPLACE, vec!["a", "b"]), (KEEP, vec!["x", "a", "b"])] {
            let mut s = setup();
            let x = fixture::add_track(&s.connection, "/music/x.mp3", "x");
            s.tracks.insert("x".into(), x);
            fixture::add_playlist(&s.connection, "Warmup", &[x]);
            let (outcome, _) = s
                .run(
                    vec![pl("p", None, "Warmup", false, &["a", "b"])],
                    &opts(PLAYLISTS, mode),
                    &none(),
                )
                .unwrap();
            let want: Vec<i64> = expected.iter().map(|t| s.id(t)).collect();
            assert_eq!(s.playlists(), vec![("Warmup".into(), want)], "{mode:?}");
            assert!(
                !s.binding(&outcome, "p").created,
                "an adopted object is never deleted later"
            );
            assert_eq!(outcome.created, 0);
        }
    }

    #[test]
    fn two_same_name_playlists_are_not_adopted() {
        let mut s = setup();
        fixture::add_playlist(&s.connection, "Warmup", &[]);
        fixture::add_playlist(&s.connection, "Warmup", &[]);
        let (outcome, warnings) = s
            .run(
                vec![pl("p", None, "Warmup", false, &["a"])],
                &opts(PLAYLISTS, REPLACE),
                &none(),
            )
            .unwrap();
        assert_eq!(s.playlists().len(), 3);
        assert!(s.binding(&outcome, "p").created);
        assert!(
            warnings.iter().any(|w| w.contains("2 Mixxx playlists")),
            "{warnings:?}"
        );
        assert!(
            s.playlists()[0].1.is_empty() && s.playlists()[1].1.is_empty(),
            "foreign ones untouched"
        );
    }

    #[test]
    fn a_foreign_object_bound_to_another_source_is_not_adopted() {
        let mut s = setup();
        let (first, _) = s
            .run(
                vec![pl("p", None, "Set", false, &["a"])],
                &opts(PLAYLISTS, KEEP),
                &none(),
            )
            .unwrap();
        // A second CueRithm playlist wants the same name; the existing object is p's.
        let (second, _) = s
            .run(
                vec![
                    pl("p", None, "Set", false, &["a"]),
                    pl("q", None, "Set", false, &["b"]),
                ],
                &opts(PLAYLISTS, REPLACE),
                &request(&first.report, &[]),
            )
            .unwrap();
        assert_eq!(s.playlists().len(), 2);
        assert_ne!(
            s.binding(&second, "p").locator,
            s.binding(&second, "q").locator
        );
        assert!(s.binding(&second, "q").created);
    }

    #[test]
    fn removed_unchanged_owned_playlist_is_deleted_and_edited_one_is_retired() {
        let mut s = setup();
        let (first, _) = s
            .run(
                vec![
                    pl("p", None, "Gone", false, &["a"]),
                    pl("q", None, "Edited", false, &["b"]),
                ],
                &opts(PLAYLISTS, KEEP),
                &none(),
            )
            .unwrap();
        s.connection
            .execute(
                "UPDATE Playlists SET name = 'Edited!' WHERE name = 'Edited'",
                [],
            )
            .unwrap();
        let (second, warnings) = s
            .run(
                vec![],
                &opts(PLAYLISTS, KEEP),
                &request(&first.report, &["p", "q"]),
            )
            .unwrap();
        assert_eq!(s.playlists(), vec![("Edited!".into(), vec![s.id("b")])]);
        assert_eq!(second.report.deleted, BTreeSet::from(["p".to_owned()]));
        assert_eq!(second.report.retired, BTreeSet::from(["q".to_owned()]));
        assert!(
            second.report.current.bindings.is_empty(),
            "both bindings are dropped"
        );
        assert_eq!(second.deleted, 1);
        assert!(
            warnings.iter().any(|w| w.contains("was kept")),
            "{warnings:?}"
        );
        assert_eq!(
            fixture::dump(&s.connection, "SELECT * FROM PlaylistTracks").len(),
            1
        );
    }

    #[test]
    fn removed_adopted_playlist_is_retired_and_absent_owned_one_is_verified_deleted() {
        let mut s = setup();
        fixture::add_playlist(&s.connection, "Mine", &[]);
        let (first, _) = s
            .run(
                vec![
                    pl("adopted", None, "Mine", false, &["a"]),
                    pl("owned", None, "Ours", false, &["a"]),
                ],
                &opts(PLAYLISTS, REPLACE),
                &none(),
            )
            .unwrap();
        s.connection
            .execute("DELETE FROM PlaylistTracks WHERE playlist_id = 2", [])
            .unwrap();
        s.connection
            .execute("DELETE FROM Playlists WHERE id = 2", [])
            .unwrap();
        let (second, _) = s
            .run(
                vec![],
                &opts(PLAYLISTS, REPLACE),
                &request(&first.report, &["adopted", "owned"]),
            )
            .unwrap();
        assert_eq!(
            second.report.retired,
            BTreeSet::from(["adopted".to_owned()])
        );
        assert_eq!(second.report.deleted, BTreeSet::from(["owned".to_owned()]));
        assert_eq!(s.playlists().len(), 1, "the adopted playlist stays");
    }

    #[test]
    fn removal_without_a_binding_or_with_an_active_playlist_is_an_error() {
        let mut s = setup();
        let nothing = PlaylistOwnershipReport::default();
        let error = s
            .run(
                vec![],
                &opts(PLAYLISTS, KEEP),
                &request(&nothing, &["ghost"]),
            )
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("no acknowledged ownership binding"),
            "{error}"
        );
        let (first, _) = s
            .run(
                vec![pl("p", None, "Set", false, &["a"])],
                &opts(PLAYLISTS, KEEP),
                &none(),
            )
            .unwrap();
        let error = s
            .run(
                vec![pl("p", None, "Set", false, &["a"])],
                &opts(PLAYLISTS, KEEP),
                &request(&first.report, &["p"]),
            )
            .unwrap_err()
            .to_string();
        assert!(error.contains("both active and removed"), "{error}");
    }

    #[test]
    fn hidden_and_locked_objects_are_never_modified() {
        let mut s = setup();
        // A history playlist named like ours is never adopted.
        s.connection
            .execute(
                "INSERT INTO Playlists(name, position, hidden) VALUES('Set', 1, 2)",
                [],
            )
            .unwrap();
        let (first, _) = s
            .run(
                vec![pl("p", None, "Set", false, &["a"])],
                &opts(PLAYLISTS, REPLACE),
                &none(),
            )
            .unwrap();
        assert_eq!(s.playlists().len(), 2);
        assert!(s.binding(&first, "p").created);
        assert!(
            s.playlists()
                .iter()
                .any(|(n, m)| n == "Set" && m.is_empty()),
            "history playlist untouched"
        );
        // A bound playlist the user then locked is left alone, with a warning.
        s.connection
            .execute("UPDATE Playlists SET locked = 1 WHERE hidden = 0", [])
            .unwrap();
        let before = fixture::dump(&s.connection, "SELECT * FROM PlaylistTracks");
        let (_, warnings) = s
            .run(
                vec![pl("p", None, "Set", false, &["b"])],
                &opts(PLAYLISTS, REPLACE),
                &request(&first.report, &[]),
            )
            .unwrap();
        assert_eq!(
            fixture::dump(&s.connection, "SELECT * FROM PlaylistTracks"),
            before
        );
        assert!(
            warnings.iter().any(|w| w.contains("locked")),
            "{warnings:?}"
        );
        // A locked foreign candidate is adopted but not modified.
        let mut s = setup();
        fixture::add_playlist(&s.connection, "Warmup", &[]);
        s.connection
            .execute("UPDATE Playlists SET locked = 1", [])
            .unwrap();
        let (_, warnings) = s
            .run(
                vec![pl("p", None, "Warmup", false, &["a"])],
                &opts(PLAYLISTS, REPLACE),
                &none(),
            )
            .unwrap();
        assert!(s.playlists()[0].1.is_empty());
        assert!(warnings.iter().any(|w| w.contains("locked")));
        // A locked crate cannot be removed even when CueRithm created it.
        let mut s = setup();
        let (first, _) = s
            .run(
                vec![pl("p", None, "C", false, &["a"])],
                &opts(MixxxContainer::Crates, KEEP),
                &none(),
            )
            .unwrap();
        s.connection
            .execute("UPDATE crates SET locked = 1", [])
            .unwrap();
        let (second, _) = s
            .run(
                vec![],
                &opts(MixxxContainer::Crates, KEEP),
                &request(&first.report, &["p"]),
            )
            .unwrap();
        assert_eq!(s.crates().len(), 1);
        assert!(second.report.retired.contains("p"));
    }

    #[test]
    fn container_switch_keeps_the_old_object_and_creates_the_new_kind() {
        let mut s = setup();
        let (first, _) = s
            .run(
                vec![pl("p", None, "Set", false, &["a"])],
                &opts(PLAYLISTS, KEEP),
                &none(),
            )
            .unwrap();
        let (second, warnings) = s
            .run(
                vec![pl("p", None, "Set", false, &["a"])],
                &opts(MixxxContainer::Crates, KEEP),
                &request(&first.report, &[]),
            )
            .unwrap();
        assert_eq!(s.playlists().len(), 1, "the playlist stays");
        assert_eq!(s.crates(), vec![("Set".into(), vec![s.id("a")])]);
        assert!(s.binding(&second, "p").locator.starts_with("crate:"));
        assert!(second.report.retired.is_empty());
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("was kept and a crate was created")),
            "{warnings:?}"
        );
    }

    #[test]
    fn crate_rename_onto_a_taken_name_keeps_the_old_name_with_a_warning() {
        let mut s = setup();
        let (first, _) = s
            .run(
                vec![pl("p", None, "A", false, &["a"])],
                &opts(MixxxContainer::Crates, KEEP),
                &none(),
            )
            .unwrap();
        fixture::add_crate(&s.connection, "B", &[]);
        let (second, warnings) = s
            .run(
                vec![pl("p", None, "B", false, &["a"])],
                &opts(MixxxContainer::Crates, KEEP),
                &request(&first.report, &[]),
            )
            .unwrap();
        let names: BTreeSet<String> = s.crates().into_iter().map(|c| c.0).collect();
        assert_eq!(names, BTreeSet::from(["A".to_owned(), "B".to_owned()]));
        assert!(
            warnings.iter().any(|w| w.contains("could not be renamed")),
            "{warnings:?}"
        );
        assert_eq!(s.binding(&second, "p").fingerprint, {
            let native = load(&s.connection, Kind::Crate)
                .unwrap()
                .into_values()
                .find(|n| n.name == "A")
                .unwrap();
            native.print(Kind::Crate).unwrap()
        });
    }

    #[test]
    fn unmapped_track_and_bad_ownership_evidence_are_errors() {
        let mut s = setup();
        let error = s
            .run(
                vec![pl("p", None, "Set", false, &["a", "ghost"])],
                &opts(PLAYLISTS, KEEP),
                &none(),
            )
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("Set") && error.contains("not part of this sync"),
            "{error}"
        );
        let binding = |locator: &str| PlaylistBinding {
            source_id: "p".into(),
            locator: locator.into(),
            parent_source_id: None,
            created: true,
            fingerprint: "x".into(),
        };
        for bad in ["playlist:abc", "playlist:", "folder:1", "7", "crate:-1"] {
            let request = PlaylistOwnershipRequest {
                previous: PlaylistOwnership {
                    identity: IDENTITY.into(),
                    bindings: vec![binding(bad)],
                },
                removed: BTreeSet::new(),
            };
            assert!(
                s.run(vec![], &opts(PLAYLISTS, KEEP), &request).is_err(),
                "{bad}"
            );
        }
        let foreign_identity = PlaylistOwnershipRequest {
            previous: PlaylistOwnership {
                identity: "engine".into(),
                bindings: vec![],
            },
            removed: BTreeSet::new(),
        };
        assert!(
            s.run(vec![], &opts(PLAYLISTS, KEEP), &foreign_identity)
                .is_err()
        );
        let no_identity = PlaylistOwnershipRequest {
            previous: PlaylistOwnership {
                identity: String::new(),
                bindings: vec![binding("playlist:1")],
            },
            removed: BTreeSet::new(),
        };
        assert!(s.run(vec![], &opts(PLAYLISTS, KEEP), &no_identity).is_err());
        let duplicate = PlaylistOwnershipRequest {
            previous: PlaylistOwnership {
                identity: IDENTITY.into(),
                bindings: vec![binding("playlist:1"), binding("playlist:1")],
            },
            removed: BTreeSet::new(),
        };
        assert!(s.run(vec![], &opts(PLAYLISTS, KEEP), &duplicate).is_err());
    }

    #[test]
    fn untouched_bindings_survive_and_report_bindings_match_native_state() {
        let mut s = setup();
        let (first, _) = s
            .run(
                vec![
                    pl("p", None, "One", false, &["a"]),
                    pl("q", None, "Two", false, &["b"]),
                ],
                &opts(PLAYLISTS, KEEP),
                &none(),
            )
            .unwrap();
        // A later partial sync only mentions "p": q's binding is carried unchanged.
        let (second, _) = s
            .run(
                vec![pl("p", None, "One", false, &["a", "c"])],
                &opts(PLAYLISTS, KEEP),
                &request(&first.report, &[]),
            )
            .unwrap();
        assert_eq!(s.binding(&second, "q"), s.binding(&first, "q"));
        assert_eq!(second.report.current.identity, IDENTITY);
        let natives = load(&s.connection, Kind::Playlist).unwrap();
        for binding in &second.report.current.bindings {
            if binding.source_id == "p" {
                let (kind, id) = parse_locator(&binding.locator).unwrap();
                assert_eq!(binding.fingerprint, natives[&id].print(kind).unwrap());
            }
        }
    }
}
