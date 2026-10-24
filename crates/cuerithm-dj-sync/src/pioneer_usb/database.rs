use super::{Names, TrackRecord, pdb};
use crate::{SyncLibrary, SyncOptions, UsbCueDestination};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
};

// Interoperability key of the removable-device format; unrelated to desktop or license credentials.
const FORMAT_KEY: &str = "r8gddnr4k847830ar6cqzbkk0el6qytmb3trbbx805jm74vez64i5o8fnrqryqls";
fn open(path: &Path, readonly: bool) -> Result<Connection> {
    let flags = if readonly {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
    };
    let db = Connection::open_with_flags(path, flags)?;
    db.pragma_update(None, "key", FORMAT_KEY)?;
    db.pragma_update(None, "cipher_compatibility", 4)?;
    let version: String = db.pragma_query_value(None, "cipher_version", |r| r.get(0))?;
    ensure!(
        !version.is_empty(),
        "Pioneer USB requires SQLCipher support"
    );
    Ok(db)
}
pub(super) fn prepare(original: &Path, staged: &Path) -> Result<Connection> {
    let mut db = open(staged, false)?;
    if original.is_file() && original != staged {
        let source = open(original, true)?;
        let backup = rusqlite::backup::Backup::new(&source, &mut db)?;
        backup.run_to_completion(128, std::time::Duration::from_millis(10), None)?;
    }
    db.execute_batch(SCHEMA)?;
    let version: Option<String> = db
        .query_row("SELECT dbVersion FROM property LIMIT 1", [], |r| r.get(0))
        .optional()?;
    ensure!(
        version.as_deref().is_none_or(|v| v == "1000"),
        "Unsupported Pioneer Device Library Plus database version"
    );
    Ok(db)
}
pub(super) fn track_ids(db: &Connection) -> Result<HashMap<String, u32>> {
    Ok(db
        .prepare("SELECT path,content_id FROM content")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?)
}
pub(super) fn artwork_id(db: &Connection, track: u32, next: &mut u32) -> Result<u32> {
    let existing: Option<u32> = db
        .query_row(
            "SELECT image_id FROM content WHERE content_id=?1",
            [track],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    if let Some(id) = existing.filter(|id| *id > 0) {
        let references: u32 = db.query_row(
            "SELECT COUNT(*) FROM content WHERE image_id=?1",
            [id],
            |r| r.get(0),
        )?;
        if references <= 1 {
            return Ok(id);
        }
    }
    let id = *next;
    *next = next.checked_add(1).context("USB artwork ID overflow")?;
    Ok(id)
}
pub(super) fn referenced_assets(db: &Connection) -> Result<Vec<String>> {
    let mut paths = Vec::new();
    let records = db
        .prepare("SELECT path,analysisDataFilePath FROM content")?
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (audio, analysis) in records {
        paths.push(audio);
        let base = analysis
            .strip_suffix(".DAT")
            .context("Unexpected existing USB analysis extension")?;
        for suffix in [".DAT", ".EXT", ".2EX"] {
            paths.push(format!("{base}{suffix}"));
        }
    }
    let images = db
        .prepare("SELECT path FROM image WHERE image_id IN(SELECT image_id FROM content)")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for image in images {
        let b = image.trim_end_matches(".jpg");
        paths.extend([
            image.clone(),
            format!("{b}_m.jpg"),
            image.replacen("/b", "/a", 1),
            format!("{b}_m.jpg").replacen("/b", "/a", 1),
        ]);
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}
pub(super) fn names(db: &Connection) -> Result<Names> {
    fn load(db: &Connection, table: &str) -> Result<BTreeMap<String, u32>> {
        Ok(db
            .prepare(&format!("SELECT name,{table}_id FROM \"{table}\""))?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?)
    }
    Ok(Names {
        artist: load(db, "artist")?,
        album: load(db, "album")?,
        genre: load(db, "genre")?,
        label: load(db, "label")?,
        key: load(db, "key")?,
    })
}
pub(super) fn import_pdb(db: &mut Connection, tables: &pdb::Tables) -> Result<()> {
    let tx = db.transaction()?;
    for (kind, table) in [
        (1, "genre"),
        (2, "artist"),
        (3, "album"),
        (4, "label"),
        (5, "key"),
    ] {
        for row in tables.get(&kind).into_iter().flatten() {
            let (id, offset) = match kind {
                2 => (
                    pdb::read32(row, 4)?,
                    if pdb::read16(row, 0)? == 96 {
                        *row.get(9).context("Truncated artist row")? as usize
                    } else {
                        pdb::read16(row, 10)? as usize
                    },
                ),
                3 => (
                    pdb::read32(row, 12)?,
                    if pdb::read16(row, 0)? == 128 {
                        *row.get(21).context("Truncated album row")? as usize
                    } else {
                        pdb::read16(row, 22)? as usize
                    },
                ),
                5 => (pdb::read32(row, 0)?, 8),
                _ => (pdb::read32(row, 0)?, 4),
            };
            let name = pdb::decode_string(row, offset)?;
            tx.execute(
                &format!("INSERT OR IGNORE INTO \"{table}\"({table}_id,name) VALUES(?1,?2)"),
                params![id, name],
            )?;
        }
    }
    for row in tables.get(&0).into_iter().flatten() {
        ensure!(row.len() >= 136, "Truncated existing PDB track");
        let field = |n: usize| pdb::decode_string(row, pdb::read16(row, 94 + n * 2)? as usize);
        let id = pdb::read32(row, 72)?;
        let path = field(20)?;
        ensure!(
            path.starts_with('/') && !path.split('/').any(|p| p == ".."),
            "Unsafe existing PDB media path"
        );
        tx.execute("INSERT INTO content(content_id,title,bpmx100,length,trackNo,discNo,artist_id_artist,artist_id_remixer,artist_id_originalArtist,
            artist_id_composer,album_id,genre_id,label_id,key_id,color_id,image_id,djComment,rating,releaseYear,dateCreated,dateAdded,path,fileName,fileSize,
            fileType,bitrate,bitDepth,samplingRate,djPlayCount,isHotCueAutoLoadOn,isKuvoDeliverStatusOn,masterDbId,masterContentId,analysisDataFilePath,analysedBits,contentLink,hasModified)
            VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?20,?21,?22,?23,?24,?25,?26,?27,?28,1,1,?29,?30,?31,41,853760,0)",
            params![id,field(17)?,pdb::read32(row,56)?,pdb::read16(row,84)?,pdb::read32(row,52)?,pdb::read16(row,76)?,
                pdb::read32(row,68)?,pdb::read32(row,44)?,pdb::read32(row,36)?,pdb::read32(row,12)?,pdb::read32(row,64)?,pdb::read32(row,60)?,pdb::read32(row,40)?,pdb::read32(row,32)?,
                row[88],pdb::read32(row,28)?,field(16)?,row[89],pdb::read16(row,80)?,field(10)?,path,field(19)?,pdb::read32(row,16)?,pdb::read16(row,90)?,
                pdb::read32(row,48)?,pdb::read16(row,82)?,pdb::read32(row,8)?,pdb::read16(row,78)?,pdb::read32(row,24)?,pdb::read32(row,20)?,field(14)?])?;
    }
    for row in tables.get(&7).into_iter().flatten() {
        tx.execute("INSERT INTO playlist(playlist_id,playlist_id_parent,sequenceNo,attribute,name) VALUES(?1,?2,?3,?4,?5)",
        params![pdb::read32(row,12)?,pdb::read32(row,0)?,pdb::read32(row,8)?,pdb::read32(row,16)?,pdb::decode_string(row,20)?])?;
    }
    for row in tables.get(&8).into_iter().flatten() {
        tx.execute(
            "INSERT INTO playlist_content(playlist_id,content_id,sequenceNo) VALUES(?1,?2,?3)",
            params![
                pdb::read32(row, 8)?,
                pdb::read32(row, 4)?,
                pdb::read32(row, 0)?
            ],
        )?;
    }
    for row in tables.get(&13).into_iter().flatten() {
        tx.execute(
            "INSERT INTO image(image_id,path) VALUES(?1,?2)",
            params![pdb::read32(row, 0)?, pdb::decode_string(row, 4)?],
        )?;
    }
    let name = tables
        .get(&19)
        .into_iter()
        .flatten()
        .find_map(|r| {
            r.windows(5)
                .position(|w| w == b"\x0b1000")
                .map(|at| pdb::decode_string(r, at + 5))
        })
        .transpose()?
        .unwrap_or_default();
    tx.execute("INSERT INTO property(deviceName,dbVersion,numberOfContents,createdDate,backGroundColorType,myTagMasterDBID) VALUES(?1,'1000',(SELECT COUNT(*) FROM content),date('now'),0,0)",[name])?;
    tx.commit()?;
    Ok(())
}
pub(super) struct WriteOptions<'a> {
    pub options: &'a SyncOptions,
    pub ownership: &'a crate::ownership::PlaylistOwnershipRequest,
}
pub(super) fn write(
    db: &mut Connection,
    library: &SyncLibrary,
    records: &[TrackRecord],
    names: &Names,
    ids: &HashMap<String, u32>,
    pioneer: &str,
    settings: WriteOptions<'_>,
) -> Result<crate::ownership::PlaylistOwnershipReport> {
    let WriteOptions { options, ownership } = settings;
    let tx = db.transaction()?;
    for (table, names) in [
        ("artist", &names.artist),
        ("album", &names.album),
        ("genre", &names.genre),
        ("label", &names.label),
        ("key", &names.key),
    ] {
        for (name, id) in names {
            tx.execute(
                &format!("INSERT OR IGNORE INTO \"{table}\"({table}_id,name) VALUES(?1,?2)"),
                params![id, name],
            )?;
        }
    }
    let master: u32 = tx.query_row(
        "SELECT COALESCE(MAX(masterDbId),377600301) FROM content",
        [],
        |r| r.get(0),
    )?;
    for record in records {
        let t = &record.track;
        tx.execute(
            "INSERT INTO content(content_id,path,bitrate,trackNo) VALUES(?1,?2,0,0) ON CONFLICT(content_id) DO NOTHING",
            params![record.id, record.path],
        )?;
        tx.execute("UPDATE content SET title=?2,titleForSearch=NULL,subtitle='',bpmx100=?3,length=?4,discNo=0,
            artist_id_artist=?5,artist_id_remixer=0,artist_id_originalArtist=0,artist_id_composer=0,artist_id_lyricist=0,
            album_id=?6,genre_id=?7,label_id=?8,key_id=?9,color_id=0,djComment=?10,rating=?11,releaseYear=?12,
            releaseDate='',dateCreated=COALESCE(dateCreated,date('now')),dateAdded=COALESCE(dateAdded,date('now')),fileName=?13,
            fileSize=?14,fileType=?15,bitDepth=16,samplingRate=?16,isrc='',djPlayCount=COALESCE(djPlayCount,0),
            isHotCueAutoLoadOn=1,isKuvoDeliverStatusOn=1,kuvoDeliveryComment='',masterDbId=?17,masterContentId=?1,
            analysisDataFilePath=?18,path=?19,color_id=?20,analysedBits=41,contentLink=853760,hasModified=0 WHERE content_id=?1",
            params![record.id,t.title,(t.bpm*100.0).round() as u32,t.duration_seconds.floor() as u32,record.artist,record.album,
                record.genre,record.label,record.key,t.comment,t.rating.min(5),t.year.unwrap_or(0),record.path.rsplit('/').next().unwrap_or_default(),
                record.file_size,record.file_type,t.sample_rate,master,record.analysis_path,record.path,super::color_id(t.color,options.nearest_color)])?;
        for (source, native) in [("bitrate", "bitrate"), ("trackNumber", "trackNo")] {
            let value = match t.attributes.get(source) {
                // Normal preview synthesizes Null for unavailable source facts.
                None | Some(cuerithm_smartlists::FieldValue::Null) => continue,
                Some(cuerithm_smartlists::FieldValue::Number(value)) => {
                    ensure!(
                        value.is_finite()
                            && *value >= 0.
                            && *value <= f64::from(u32::MAX)
                            && (source == "bitrate" || value.fract() == 0.),
                        "Invalid USB source {source}"
                    );
                    value.trunc() as u32
                }
                Some(_) => anyhow::bail!("Invalid USB source {source}"),
            };
            tx.execute(
                &format!("UPDATE content SET {native}=?1 WHERE content_id=?2"),
                params![value, record.id],
            )?;
        }
        if record.artwork > 0 {
            let path = format!(
                "/{pioneer}/Artwork/{:05}/b{}.jpg",
                record.artwork / 20 + 1,
                record.artwork
            );
            tx.execute(
                "INSERT OR REPLACE INTO image(image_id,path) VALUES(?1,?2)",
                params![record.artwork, path],
            )?;
            tx.execute(
                "UPDATE content SET image_id=?1 WHERE content_id=?2",
                params![record.artwork, record.id],
            )?;
        }
        tx.execute("DELETE FROM cue WHERE content_id=?1", [record.id])?;
        let hot = matches!(
            options.usb_cue_destination,
            UsbCueDestination::Hot | UsbCueDestination::Both
        );
        let memory = matches!(
            options.usb_cue_destination,
            UsbCueDestination::Memory | UsbCueDestination::Both
        );
        for cue in &t.cues {
            for kind in [hot.then_some(cue.index as u32 + 1), memory.then_some(0)]
                .into_iter()
                .flatten()
            {
                tx.execute("INSERT INTO cue(content_id,kind,colorTableIndex,cueComment,isActiveLoop,beatLoopNumerator,beatLoopDenominator,
                    inUsec,outUsec,in150FramePerSec,out150FramePerSec) VALUES(?1,?2,0,?3,?10,?4,?5,?6,?7,?8,?9)",
                    params![record.id,kind,cue.name,if cue.end_seconds.is_some(){4}else{0},if cue.end_seconds.is_some(){1}else{0},
                        (cue.start_seconds*1e6).floor() as u64,cue.end_seconds.map(|s|(s*1e6).floor() as u64),
                        (cue.start_seconds*150.0).floor() as u64,cue.end_seconds.map(|s|(s*150.0).floor() as u64),
                        cue.active_loop && cue.end_seconds.is_some()])?;
            }
        }
    }
    let session = super::ownership::Session::prepare(&tx, library, ownership)?;
    for playlist in &library.playlists {
        let id = session.ids[&playlist.id];
        tx.execute("DELETE FROM playlist_content WHERE playlist_id=?1", [id])?;
        for (index, source) in playlist.track_ids.iter().enumerate() {
            let track = ids
                .get(source)
                .context("USB playlist references a missing source track")?;
            tx.execute(
                "INSERT INTO playlist_content(playlist_id,content_id,sequenceNo) VALUES(?1,?2,?3)",
                params![id, track, index + 1],
            )?;
        }
    }
    let count: u32 = tx.query_row("SELECT COUNT(*) FROM content", [], |r| r.get(0))?;
    let property_count: u32 = tx.query_row("SELECT COUNT(*) FROM property", [], |r| r.get(0))?;
    if property_count == 0 {
        tx.execute("INSERT INTO property(deviceName,dbVersion,numberOfContents,createdDate,backGroundColorType,myTagMasterDBID) VALUES('','1000',?1,date('now'),0,0)",[count])?;
    } else {
        tx.execute("UPDATE property SET numberOfContents=?1", [count])?;
    }
    defaults(&tx)?;
    let report = session.finish(&tx, library)?;
    tx.commit()?;
    Ok(report)
}

const MENUS: [(&str, u16); 27] = [
    ("GENRE", 128),
    ("ARTIST", 129),
    ("ALBUM", 130),
    ("TRACK", 131),
    ("BPM", 133),
    ("RATING", 134),
    ("YEAR", 135),
    ("REMIXER", 136),
    ("LABEL", 137),
    ("ORIGINAL ARTIST", 138),
    ("KEY", 139),
    ("CUE", 141),
    ("COLOR", 142),
    ("TIME", 146),
    ("BITRATE", 147),
    ("FILE NAME", 148),
    ("PLAYLIST", 132),
    ("HOT CUE BANK", 152),
    ("HISTORY", 149),
    ("SEARCH", 145),
    ("COMMENTS", 150),
    ("DATE ADDED", 140),
    ("DJ PLAY COUNT", 151),
    ("FOLDER", 144),
    ("DEFAULT", 161),
    ("ALPHABET", 162),
    ("MATCHING", 170),
];
fn defaults(db: &Connection) -> Result<()> {
    if db.query_row("SELECT COUNT(*) FROM menuItem", [], |r| r.get::<_, u32>(0))? == 0 {
        for (n, (name, kind)) in MENUS.iter().enumerate() {
            db.execute(
                "INSERT INTO menuItem VALUES(?1,?2,?3)",
                params![n + 1, kind, format!("\u{fffa}{name}\u{fffb}")],
            )?;
        }
    }
    if db.query_row("SELECT COUNT(*) FROM category", [], |r| r.get::<_, u32>(0))? == 0 {
        for (order, id) in [2, 3, 4, 11, 17, 19, 20, 24, 22].iter().enumerate() {
            db.execute(
                "INSERT INTO category VALUES(?1,?1,?2,1)",
                params![id, order],
            )?;
        }
    }
    if db.query_row("SELECT COUNT(*) FROM sort", [], |r| r.get::<_, u32>(0))? == 0 {
        for (order, id) in [25, 26, 2, 3, 5, 6, 11].iter().enumerate() {
            db.execute(
                "INSERT INTO sort VALUES(?1,?2,?3,1,0)",
                params![order, id, order + 1],
            )?;
        }
    }
    for (id, name) in [
        (1, "Pink"),
        (2, "Red"),
        (3, "Orange"),
        (4, "Yellow"),
        (5, "Green"),
        (6, "Aqua"),
        (7, "Blue"),
        (8, "Purple"),
    ] {
        db.execute(
            "INSERT OR IGNORE INTO color VALUES(?1,?2)",
            params![id, name],
        )?;
    }
    Ok(())
}
pub(super) fn default_pdb_config(
    db: &Connection,
    tables: &mut pdb::Tables,
    count: u32,
) -> Result<()> {
    if let std::collections::btree_map::Entry::Vacant(entry) = tables.entry(16) {
        let mut rows = Vec::new();
        for (n, (name, kind)) in MENUS.iter().enumerate() {
            let mut row = vec![0; 4];
            pdb::u16le(&mut row, 0, n as u16 + 1);
            pdb::u16le(&mut row, 2, *kind);
            row.extend(pdb::string(&format!("\u{fffa}{name}\u{fffb}"))?);
            rows.push(pdb::pad(row));
        }
        entry.insert(rows);
    }
    if let std::collections::btree_map::Entry::Vacant(entry) = tables.entry(17) {
        let rows=db.prepare("SELECT menuItem_id,category_id,sequenceNo,isVisible FROM category ORDER BY category_id")?.query_map([],|r|Ok((r.get::<_,u16>(0)?,r.get::<_,u16>(1)?,r.get::<_,u16>(2)?,r.get::<_,bool>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        entry.insert(
            rows.iter()
                .map(|(menu, id, seq, visible)| {
                    let mut b = vec![0; 8];
                    pdb::u16le(&mut b, 0, *id);
                    pdb::u16le(&mut b, 2, *menu);
                    let flags = if *visible {
                        match menu {
                            2 => 2,
                            3 => 3,
                            4 => 1,
                            _ => 99,
                        }
                    } else {
                        355
                    };
                    pdb::u16le(&mut b, 4, flags);
                    pdb::u16le(&mut b, 6, *seq);
                    b
                })
                .collect(),
        );
    }
    if let std::collections::btree_map::Entry::Vacant(entry) = tables.entry(18) {
        let rows = db
            .prepare("SELECT menuItem_id,sort_id,sequenceNo,isVisible FROM sort ORDER BY sort_id")?
            .query_map([], |r| {
                Ok((
                    r.get::<_, u16>(0)?,
                    r.get::<_, u16>(1)?,
                    r.get::<_, u16>(2)?,
                    r.get::<_, bool>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        entry.insert(
            rows.iter()
                .map(|(menu, id, seq, visible)| {
                    let mut b = vec![0; 8];
                    pdb::u16le(&mut b, 0, *id);
                    pdb::u16le(&mut b, 2, *menu);
                    pdb::u16le(&mut b, 4, if *visible { 0 } else { 1 });
                    pdb::u16le(&mut b, 6, *seq);
                    b
                })
                .collect(),
        );
    }
    if let Some(rows) = tables.get_mut(&19) {
        for row in rows {
            if row.len() >= 8 && row.windows(5).any(|w| w == b"\x0b1000") {
                pdb::u32le(row, 4, count);
            }
        }
    } else {
        let name: String = db.query_row(
            "SELECT COALESCE(deviceName,'') FROM property LIMIT 1",
            [],
            |r| r.get(0),
        )?;
        let date: String = db.query_row("SELECT date('now')", [], |r| r.get(0))?;
        let mut row = vec![0; 12];
        pdb::u16le(&mut row, 0, 640);
        pdb::u16le(&mut row, 2, 160);
        pdb::u32le(&mut row, 4, count);
        row.extend(pdb::string(&date)?);
        row.extend([25, 30]);
        row.extend(pdb::string("1000")?);
        row.extend(pdb::string(&name)?);
        row.extend([0; 8]);
        tables.insert(19, vec![pdb::pad(row)]);
    }
    Ok(())
}

pub(super) fn pdb_tables(db: &Connection, _new: &[TrackRecord]) -> Result<pdb::Tables> {
    let mut tables = pdb::Tables::new();
    for (kind, table) in [
        (1, "genre"),
        (2, "artist"),
        (3, "album"),
        (4, "label"),
        (5, "key"),
    ] {
        let rows = db
            .prepare(&format!(
                "SELECT {table}_id,name FROM \"{table}\" ORDER BY {table}_id"
            ))?
            .query_map([], |r| Ok((r.get::<_, u32>(0)?, r.get::<_, String>(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        tables.insert(
            kind,
            rows.iter()
                .map(|(id, name)| match kind {
                    2 => pdb::artist(*id, name),
                    3 => pdb::album(*id, name),
                    5 => pdb::key(*id, name),
                    _ => pdb::named(*id, name),
                })
                .collect::<Result<_>>()?,
        );
    }
    let mut statement = db.prepare("SELECT * FROM content ORDER BY content_id")?;
    let mut rows = statement.query([])?;
    let mut tracks = Vec::new();
    while let Some(row) = rows.next()? {
        tracks.push(track_row(row)?);
    }
    tables.insert(0, tracks);
    let mut statement=db.prepare("SELECT playlist_id,COALESCE(playlist_id_parent,0),COALESCE(sequenceNo,0),COALESCE(attribute,0),name FROM playlist ORDER BY playlist_id")?;
    let rows = statement
        .query_map([], |r| {
            Ok((
                r.get::<_, u32>(0)?,
                r.get::<_, u32>(1)?,
                r.get::<_, u32>(2)?,
                r.get::<_, bool>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    tables.insert(
        7,
        rows.iter()
            .map(|(id, parent, seq, folder, name)| pdb::playlist(*id, *parent, *seq, *folder, name))
            .collect::<Result<_>>()?,
    );
    tables.insert(8,db.prepare("SELECT playlist_id,content_id,sequenceNo FROM playlist_content ORDER BY playlist_id,sequenceNo")?.query_map([],|r|Ok(pdb::playlist_entry(r.get(0)?,r.get(1)?,r.get(2)?)))?.collect::<rusqlite::Result<_>>()?);
    let rows = db
        .prepare("SELECT image_id,path FROM image ORDER BY image_id")?
        .query_map([], |r| Ok((r.get::<_, u32>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    tables.insert(
        13,
        rows.iter()
            .map(|(id, path)| {
                let mut r = id.to_le_bytes().to_vec();
                r.extend(pdb::string(&path.replacen("/b", "/a", 1))?);
                Ok(pdb::pad(r))
            })
            .collect::<Result<_>>()?,
    );
    let mut colors = Vec::new();
    for (id, name) in [
        (1, "Pink"),
        (2, "Red"),
        (3, "Orange"),
        (4, "Yellow"),
        (5, "Green"),
        (6, "Aqua"),
        (7, "Blue"),
        (8, "Purple"),
    ] {
        let mut b = vec![0; 8];
        b[4] = id;
        b[5] = id;
        b.extend(pdb::string(name)?);
        colors.push(pdb::pad(b));
    }
    tables.insert(6, colors);
    Ok(tables)
}
fn track_row(r: &rusqlite::Row<'_>) -> Result<Vec<u8>> {
    let number = |name: &str| {
        r.get::<_, Option<u64>>(name)
            .map(|v| v.unwrap_or(0).min(u32::MAX as u64) as u32)
    };
    let text = |name: &str| {
        r.get::<_, Option<String>>(name)
            .map(Option::unwrap_or_default)
    };
    let mut row = vec![0; 136];
    pdb::u16le(&mut row, 0, 36);
    pdb::u32le(&mut row, 4, 788224);
    for (offset, name) in [
        (8, "samplingRate"),
        (12, "artist_id_composer"),
        (16, "fileSize"),
        (20, "masterContentId"),
        (24, "masterDbId"),
        (28, "image_id"),
        (32, "key_id"),
        (36, "artist_id_originalArtist"),
        (40, "label_id"),
        (44, "artist_id_remixer"),
        (48, "bitrate"),
        (52, "trackNo"),
        (56, "bpmx100"),
        (60, "genre_id"),
        (64, "album_id"),
        (68, "artist_id_artist"),
        (72, "content_id"),
    ] {
        pdb::u32le(&mut row, offset, number(name)?);
    }
    for (offset, name) in [
        (76, "discNo"),
        (78, "djPlayCount"),
        (80, "releaseYear"),
        (82, "bitDepth"),
        (84, "length"),
        (90, "fileType"),
    ] {
        pdb::u16le(&mut row, offset, number(name)?.min(u16::MAX as u32) as u16);
    }
    pdb::u16le(&mut row, 86, 41);
    pdb::u16le(&mut row, 92, 3);
    row[88] = number("color_id")?.min(8) as u8;
    row[89] = number("rating")?.min(5) as u8;
    let mp3 = number("fileType")? == 1;
    let strings = [
        String::new(),
        String::new(),
        if mp3 { "2" } else { "7" }.into(),
        "2".into(),
        if mp3 { "1" } else { "" }.into(),
        String::new(),
        "ON".into(),
        "ON".into(),
        String::new(),
        String::new(),
        text("dateAdded")?,
        text("releaseDate")?,
        String::new(),
        String::new(),
        text("analysisDataFilePath")?,
        text("dateAdded")?,
        pdb::display(&text("djComment")?),
        pdb::display(&text("title")?),
        String::new(),
        text("fileName")?,
        text("path")?,
    ];
    for (n, text) in strings.iter().enumerate() {
        let bytes = pdb::string(text)?;
        if bytes[0] & 1 == 0 {
            row.resize(row.len().next_multiple_of(4), 0);
        }
        let offset = u16::try_from(row.len())?;
        pdb::u16le(&mut row, 94 + n * 2, offset);
        row.extend(bytes);
    }
    row.resize(row.len() + 4, 0);
    let row = pdb::pad(row);
    ensure!(
        row.len() + 6 <= 4096 - 40,
        "USB track metadata exceeds PDB page; shorten title/comment or path"
    );
    Ok(row)
}
const SCHEMA:&str="
CREATE TABLE IF NOT EXISTS content(content_id integer primary key,title text,titleForSearch text,subtitle text,bpmx100 integer,length integer,trackNo integer,discNo integer,artist_id_artist integer,artist_id_remixer integer,artist_id_originalArtist integer,artist_id_composer integer,artist_id_lyricist integer,album_id integer,genre_id integer,label_id integer,key_id integer,color_id integer,image_id integer,djComment text,rating integer,releaseYear integer,releaseDate text,dateCreated text,dateAdded text,path text,fileName text,fileSize integer,fileType integer,bitrate integer,bitDepth integer,samplingRate integer,isrc text,djPlayCount integer,isHotCueAutoLoadOn integer,isKuvoDeliverStatusOn integer,kuvoDeliveryComment text,masterDbId integer,masterContentId integer,analysisDataFilePath text,analysedBits integer,contentLink integer,hasModified integer,cueUpdateCount integer,analysisDataUpdateCount integer,informationUpdateCount integer);
CREATE TABLE IF NOT EXISTS genre(genre_id integer primary key,name text);
CREATE TABLE IF NOT EXISTS artist(artist_id integer primary key,name text,nameForSearch text);
CREATE TABLE IF NOT EXISTS album(album_id integer primary key,name text,artist_id integer,image_id integer,isComplation integer,nameForSearch text);
CREATE TABLE IF NOT EXISTS label(label_id integer primary key,name text);
CREATE TABLE IF NOT EXISTS key(key_id integer primary key,name text);
CREATE TABLE IF NOT EXISTS color(color_id integer primary key,name text);
CREATE TABLE IF NOT EXISTS playlist(playlist_id integer primary key,sequenceNo integer,name text,image_id integer,attribute integer,playlist_id_parent integer);
CREATE TABLE IF NOT EXISTS playlist_content(playlist_id integer,content_id integer,sequenceNo integer);
CREATE TABLE IF NOT EXISTS image(image_id integer primary key,path text);
CREATE TABLE IF NOT EXISTS cue(cue_id integer primary key,content_id integer,kind integer,colorTableIndex integer,cueComment text,isActiveLoop integer,beatLoopNumerator integer,beatLoopDenominator integer,inUsec integer,outUsec integer,in150FramePerSec integer,out150FramePerSec integer,inMpegFrameNumber integer,outMpegFrameNumber integer,inMpegAbs integer,outMpegAbs integer,inDecodingStartFramePosition integer,outDecodingStartFramePosition integer,inFileOffsetInBlock integer,OutFileOffsetInBlock integer,inNumberOfSampleInBlock integer,outNumberOfSampleInBlock integer);
CREATE TABLE IF NOT EXISTS menuItem(menuItem_id integer primary key,kind integer,name text);
CREATE TABLE IF NOT EXISTS category(category_id integer primary key,menuItem_id integer,sequenceNo integer,isVisible integer);
CREATE TABLE IF NOT EXISTS sort(sort_id integer primary key,menuItem_id integer,sequenceNo integer,isVisible integer,isSelectedAsSubColumn integer);
CREATE TABLE IF NOT EXISTS property(deviceName text,dbVersion text,numberOfContents integer,createdDate text,backGroundColorType integer,myTagMasterDBID integer);
CREATE TABLE IF NOT EXISTS history(history_id integer primary key,sequenceNo integer,name text,attribute integer,history_id_parent integer);
CREATE TABLE IF NOT EXISTS history_content(history_id integer,content_id integer,sequenceNo integer);
CREATE TABLE IF NOT EXISTS hotCueBankList(hotCueBankList_id integer primary key,sequenceNo integer,name text,image_id integer,attribute integer,hotCueBankList_id_parent integer);
CREATE TABLE IF NOT EXISTS hotCueBankList_cue(hotCueBankList_id integer,cue_id integer,sequenceNo integer);
CREATE TABLE IF NOT EXISTS recommendedLike(content_id_1 integer,content_id_2 integer,rating integer,createdDate integer);
CREATE TABLE IF NOT EXISTS myTag(myTag_id integer primary key,sequenceNo integer,name text,attribute integer,myTag_id_parent integer);
CREATE TABLE IF NOT EXISTS myTag_content(myTag_id integer,content_id integer);
CREATE INDEX IF NOT EXISTS index_playlist_content_playlist_id ON playlist_content(playlist_id);
CREATE INDEX IF NOT EXISTS index_hotCueBankList_cue_hotCueBankList_id ON hotCueBankList_cue(hotCueBankList_id);
CREATE INDEX IF NOT EXISTS index_myTag_content_myTag_id ON myTag_content(myTag_id);
CREATE INDEX IF NOT EXISTS index_myTag_content_content_id ON myTag_content(content_id);";

/// Existing format policy, read-only access only; never call prepare on a source.
pub(crate) fn open_readonly(path: &Path) -> Result<Connection> {
    let db = open(path, true)?;
    db.set_limit(
        rusqlite::limits::Limit::SQLITE_LIMIT_LENGTH,
        64 * 1024 * 1024,
    )?;
    db.pragma_update(None, "query_only", true)?;
    db.busy_timeout(std::time::Duration::from_secs(2))?;
    let (kind, sql): (String, String) = db.query_row(
        "SELECT type,sql FROM sqlite_master WHERE name='property'",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    ensure!(
        kind == "table" && !sql.to_ascii_uppercase().contains("VIRTUAL TABLE"),
        "Unsupported device property table"
    );
    let versions = db
        .prepare("SELECT CASE WHEN length(dbVersion)<=64 THEN dbVersion END FROM property LIMIT 2")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ensure!(
        versions.len() == 1 && versions[0] == "1000",
        "Unsupported OneLibrary database version"
    );
    Ok(db)
}
pub(crate) fn capture_readonly(
    input: &Path,
    output: &Path,
    max_bytes: u64,
    cancel: &cuerithm_media::CancellationToken,
) -> Result<()> {
    use rusqlite::backup::StepResult;
    cancel.check()?;
    let source = open_readonly(input)?;
    let mut destination = open(output, false)?;
    {
        let backup = rusqlite::backup::Backup::new(&source, &mut destination)?;
        let start = std::time::Instant::now();
        loop {
            cancel.check()?;
            ensure!(
                start.elapsed() < std::time::Duration::from_secs(120),
                "Device snapshot timed out"
            );
            let state = backup.step(64)?;
            ensure!(
                std::fs::metadata(output)?.len() <= max_bytes,
                "Device snapshot byte limit"
            );
            match state {
                StepResult::Done => break,
                StepResult::More => {}
                StepResult::Busy | StepResult::Locked => {
                    std::thread::sleep(std::time::Duration::from_millis(10))
                }
                _ => anyhow::bail!("Unknown device snapshot status"),
            }
        }
    }
    destination.pragma_update(None, "journal_mode", "DELETE")?;
    cancel.check()?;
    std::fs::File::options()
        .write(true)
        .open(output)?
        .sync_all()?;
    Ok(())
}
