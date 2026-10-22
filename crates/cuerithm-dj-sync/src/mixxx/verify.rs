//! Readback checks on the staged database, run before its transaction commits. They
//! prove that rows outside the sync are unchanged and that nothing Mixxx's loader
//! would reject or discard was written.
use super::{
    beats::{BEAT_GRID, BEAT_MAP},
    cues::{HOT_CUE, LOOP},
    proto,
    tracks::TrackOutcome,
};
use anyhow::{Context, Result, bail, ensure};
use rusqlite::{Connection, params, types::ValueRef};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

type Digest32 = [u8; 32];

fn feed(hasher: &mut Sha256, value: ValueRef<'_>) {
    match value {
        ValueRef::Null => hasher.update([0]),
        ValueRef::Integer(number) => {
            hasher.update([1]);
            hasher.update(number.to_le_bytes());
        }
        ValueRef::Real(number) => {
            hasher.update([2]);
            hasher.update(number.to_bits().to_le_bytes());
        }
        ValueRef::Text(text) => {
            hasher.update([3]);
            hasher.update((text.len() as u64).to_le_bytes());
            hasher.update(text);
        }
        ValueRef::Blob(bytes) => {
            hasher.update([4]);
            hasher.update((bytes.len() as u64).to_le_bytes());
            hasher.update(bytes);
        }
    }
}

/// Digest of every row of `sql`, keyed by the integer in column `key`.
fn grouped_digests(
    connection: &Connection,
    sql: &str,
    key: usize,
) -> Result<BTreeMap<i64, Digest32>> {
    let mut statement = connection.prepare(sql)?;
    let columns = statement.column_count();
    let mut rows = statement.query([])?;
    let mut hashers: BTreeMap<i64, Sha256> = BTreeMap::new();
    while let Some(row) = rows.next()? {
        let group: i64 = row.get(key)?;
        let hasher = hashers.entry(group).or_default();
        for column in 0..columns {
            feed(hasher, row.get_ref(column)?);
        }
    }
    Ok(hashers
        .into_iter()
        .map(|(group, hasher)| (group, hasher.finalize().into()))
        .collect())
}

fn table_digest(connection: &Connection, sql: &str) -> Result<Digest32> {
    let mut statement = connection.prepare(sql)?;
    let columns = statement.column_count();
    let mut rows = statement.query([])?;
    let mut hasher = Sha256::new();
    while let Some(row) = rows.next()? {
        for column in 0..columns {
            feed(&mut hasher, row.get_ref(column)?);
        }
    }
    Ok(hasher.finalize().into())
}

/// References that already point nowhere; the sync must not add to them.
fn dangling(connection: &Connection) -> Result<i64> {
    Ok(connection.query_row(
        "SELECT
           (SELECT COUNT(*) FROM PlaylistTracks p LEFT JOIN library l ON l.id = p.track_id WHERE l.id IS NULL)
         + (SELECT COUNT(*) FROM crate_tracks c LEFT JOIN library l ON l.id = c.track_id WHERE l.id IS NULL)
         + (SELECT COUNT(*) FROM cues c LEFT JOIN library l ON l.id = c.track_id WHERE l.id IS NULL)
         + (SELECT COUNT(*) FROM library l LEFT JOIN track_locations t ON t.id = l.location WHERE t.id IS NULL)",
        [],
        |row| row.get(0),
    )?)
}

/// What the database held before the sync wrote anything.
pub(super) struct Snapshot {
    library: BTreeMap<i64, Digest32>,
    cues: BTreeMap<i64, Digest32>,
    foreign: [Digest32; 3],
    /// The folders Mixxx scans. Only an empty list may be filled in.
    directories: Vec<String>,
    dangling: i64,
}

fn directories(connection: &Connection) -> Result<Vec<String>> {
    let mut statement =
        connection.prepare("SELECT directory FROM directories ORDER BY directory")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

impl Snapshot {
    pub(super) fn capture(connection: &Connection) -> Result<Self> {
        Ok(Self {
            library: grouped_digests(connection, "SELECT * FROM library ORDER BY id", 0)?,
            cues: grouped_digests(connection, "SELECT * FROM cues ORDER BY track_id, id", 1)?,
            foreign: Self::foreign(connection)?,
            directories: directories(connection)?,
            dangling: dangling(connection)?,
        })
    }

    /// Settings, and the Auto DJ and history playlists with their tracks.
    fn foreign(connection: &Connection) -> Result<[Digest32; 3]> {
        Ok([
            table_digest(connection, "SELECT * FROM settings ORDER BY name")?,
            table_digest(
                connection,
                "SELECT * FROM Playlists WHERE hidden != 0 ORDER BY id",
            )?,
            table_digest(
                connection,
                "SELECT pt.* FROM PlaylistTracks pt JOIN Playlists p ON p.id = pt.playlist_id
                 WHERE p.hidden != 0 ORDER BY pt.id",
            )?,
        ])
    }

    /// Fail unless the writes did exactly what the plan says.
    pub(super) fn check(&self, connection: &Connection, outcome: &TrackOutcome) -> Result<()> {
        let scope: BTreeSet<i64> = outcome.library_ids.values().copied().collect();
        let after = grouped_digests(connection, "SELECT * FROM library ORDER BY id", 0)?;
        ensure!(
            after.len() == self.library.len() + outcome.added,
            "Mixxx track count is {} but {} were expected",
            after.len(),
            self.library.len() + outcome.added
        );
        for (id, digest) in &self.library {
            if scope.contains(id) {
                continue;
            }
            ensure!(
                after.get(id) == Some(digest),
                "A Mixxx track outside this sync would change (library id {id})"
            );
        }
        let after_cues =
            grouped_digests(connection, "SELECT * FROM cues ORDER BY track_id, id", 1)?;
        for (track, digest) in &self.cues {
            if !scope.contains(track) {
                ensure!(
                    after_cues.get(track) == Some(digest),
                    "Cues of a Mixxx track outside this sync would change (library id {track})"
                );
            }
        }
        ensure!(
            after_cues
                .keys()
                .all(|track| scope.contains(track) || self.cues.contains_key(track)),
            "Cues would be added to a Mixxx track outside this sync"
        );
        ensure!(
            Self::foreign(connection)? == self.foreign,
            "Mixxx settings or its Auto DJ and history playlists would change"
        );
        ensure!(
            self.directories.is_empty() || directories(connection)? == self.directories,
            "A list of folders Mixxx already scans would change"
        );
        ensure!(
            dangling(connection)? <= self.dangling,
            "The sync would leave a Mixxx reference pointing at a missing row"
        );
        for id in &scope {
            let (deleted, header, synced): (Option<i64>, Option<i64>, Option<i64>) = connection
                .query_row(
                    "SELECT mixxx_deleted, header_parsed, source_synchronized_ms FROM library WHERE id = ?1",
                    [id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .with_context(|| format!("Mixxx track {id} is missing after the write"))?;
            ensure!(
                deleted.unwrap_or(0) == 0,
                "Mixxx track {id} would stay removed"
            );
            if outcome.created.contains(id) {
                ensure!(
                    header == Some(1) && synced.is_none(),
                    "A new Mixxx track would be re-read from its tags on first load"
                );
            }
            let slots: i64 = connection.query_row(
                "SELECT COUNT(*) FROM (SELECT hotcue FROM cues WHERE track_id = ?1 AND hotcue >= 0
                 AND type IN (?2, ?3) GROUP BY hotcue HAVING COUNT(*) > 1)",
                params![id, HOT_CUE, LOOP],
                |row| row.get(0),
            )?;
            ensure!(
                slots == 0,
                "Mixxx track {id} would have two cues in one slot"
            );
            let bad: i64 = connection.query_row(
                "SELECT COUNT(*) FROM cues WHERE track_id = ?1 AND
                 ((type = ?3 AND length <= 0) OR (type = ?2 AND position = 0 AND color = 0))",
                params![id, HOT_CUE, LOOP],
                |row| row.get(0),
            )?;
            ensure!(bad == 0, "Mixxx track {id} would have a cue Mixxx discards");
        }
        for id in &outcome.grids {
            check_grid(connection, *id)?;
        }
        for id in &outcome.keys {
            check_key(connection, *id)?;
        }
        Ok(())
    }
}

fn check_grid(connection: &Connection, id: i64) -> Result<()> {
    let (version, blob): (String, Vec<u8>) = connection.query_row(
        "SELECT beats_version, beats FROM library WHERE id = ?1",
        [id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    match version.as_str() {
        BEAT_GRID => {
            let (bpm, _) = proto::decode::beat_grid(&blob)
                .with_context(|| format!("Beat grid of Mixxx track {id} does not decode"))?;
            ensure!(
                bpm.is_finite() && bpm > 0.0,
                "Beat grid of Mixxx track {id} has no tempo"
            );
        }
        BEAT_MAP => {
            let frames = proto::decode::beat_map(&blob)
                .with_context(|| format!("Beat map of Mixxx track {id} does not decode"))?;
            ensure!(
                frames.len() >= 2 && frames.windows(2).all(|pair| pair[0] < pair[1]),
                "Beat map of Mixxx track {id} is not an increasing list of two or more beats"
            );
        }
        other => bail!("Mixxx track {id} has unexpected beats version {other:?}"),
    }
    Ok(())
}

fn check_key(connection: &Connection, id: i64) -> Result<()> {
    let (version, blob, key_id, text): (String, Vec<u8>, i64, String) = connection.query_row(
        "SELECT keys_version, keys, key_id, key FROM library WHERE id = ?1",
        [id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;
    ensure!(
        version == "KeyMap-1.0",
        "Mixxx track {id} has unexpected keys version {version:?}"
    );
    let (blob_key, blob_text, source) = proto::decode::key_map(&blob)
        .with_context(|| format!("Key of Mixxx track {id} does not decode"))?;
    ensure!(
        (1..=24).contains(&key_id) && i64::from(blob_key) == key_id && blob_text == text,
        "Key columns of Mixxx track {id} disagree with its key blob"
    );
    ensure!(source <= 2, "Key of Mixxx track {id} has an unknown source");
    Ok(())
}
