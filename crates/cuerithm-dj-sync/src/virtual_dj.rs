//! VirtualDJ 8.5 database and current MyLists playlists.
#[path = "virtual_dj/owned.rs"]
mod owned;
use crate::{SyncLibrary, SyncOptions, SyncTrack};
use anyhow::{Context, Result, bail, ensure};
pub use owned::write_owned;
use quick_xml::{
    Reader, Writer,
    events::{BytesEnd, BytesStart, BytesText, Event},
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
enum Part {
    Node(Node),
    Raw(Event<'static>),
}
#[derive(Clone, Debug)]
struct Node {
    name: String,
    attrs: BTreeMap<String, String>,
    children: Vec<Part>,
}
impl Node {
    fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            attrs: BTreeMap::new(),
            children: vec![],
        }
    }
    fn attr(&mut self, name: &str, value: impl ToString) {
        self.attrs.insert(name.into(), value.to_string());
    }
    fn child(&mut self, name: &str) -> &mut Node {
        let index = self
            .children
            .iter()
            .position(|p| matches!(p, Part::Node(n) if n.name == name));
        let index = index.unwrap_or_else(|| {
            self.children.push(Part::Node(Node::new(name)));
            self.children.len() - 1
        });
        match &mut self.children[index] {
            Part::Node(n) => n,
            _ => unreachable!(),
        }
    }
    fn emit(&self, writer: &mut Writer<Vec<u8>>) -> Result<()> {
        let mut tag = BytesStart::new(&self.name);
        for (k, v) in &self.attrs {
            tag.push_attribute((k.as_str(), v.as_str()));
        }
        if self.children.is_empty() {
            writer.write_event(Event::Empty(tag))?;
        } else {
            writer.write_event(Event::Start(tag))?;
            for child in &self.children {
                match child {
                    Part::Node(n) => n.emit(writer)?,
                    Part::Raw(e) => writer.write_event(e.borrow())?,
                }
            }
            writer.write_event(Event::End(BytesEnd::new(&self.name)))?;
        }
        Ok(())
    }
    fn bytes(&self) -> Result<Vec<u8>> {
        let mut writer = Writer::new(Vec::new());
        writer.write_event(Event::Decl(quick_xml::events::BytesDecl::new(
            "1.0",
            Some("UTF-8"),
            None,
        )))?;
        self.emit(&mut writer)?;
        Ok(writer.into_inner())
    }
}
fn parse(bytes: &[u8]) -> Result<Node> {
    let mut reader = Reader::from_reader(bytes);
    let mut stack: Vec<Node> = vec![];
    let mut root = None;
    loop {
        let event = reader.read_event().context("Invalid VirtualDJ XML")?;
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                ensure!(stack.len() < 64, "VirtualDJ XML nesting limit exceeded");
                let mut node = Node::new(e.name().as_ref());
                for attr in e.attributes() {
                    let attr = attr?;
                    node.attr(
                        attr.key.as_ref(),
                        attr.normalized_value(quick_xml::XmlVersion::Implicit1_0)?,
                    );
                }
                if matches!(event, Event::Start(_)) {
                    stack.push(node);
                } else if let Some(parent) = stack.last_mut() {
                    parent.children.push(Part::Node(node));
                } else {
                    ensure!(root.is_none(), "Multiple XML roots");
                    root = Some(node);
                }
            }
            Event::End(_) => {
                let node = stack.pop().context("Unbalanced XML")?;
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(Part::Node(node));
                } else {
                    ensure!(root.is_none(), "Multiple XML roots");
                    root = Some(node);
                }
            }
            Event::DocType(_) => bail!("VirtualDJ XML document types are unsupported"),
            Event::Decl(_) => {}
            Event::Eof => break,
            raw => {
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(Part::Raw(raw.into_owned()));
                }
            }
        }
    }
    ensure!(stack.is_empty(), "Incomplete XML");
    root.context("VirtualDJ database is empty")
}
fn path_key(path: &str) -> String {
    // VirtualDJ treats paths case-insensitively on its supported platforms.
    path.replace('\\', "/").to_lowercase()
}
fn track_xml(song: &mut Node, track: &SyncTrack, options: &SyncOptions) -> Result<()> {
    song.attr(
        "FilePath",
        track.path.to_str().context("Non-UTF-8 audio path")?,
    );
    if let Ok(meta) = fs::metadata(&track.path) {
        song.attr("FileSize", meta.len());
    }
    if let Some(number) = track.attribute_number("sizeBytes") {
        ensure!(
            number >= 0.0 && number.fract() == 0.0,
            "Invalid VirtualDJ file size"
        );
        song.attr("FileSize", format!("{number:.0}"));
    }
    let key = crate::formatting::format_key(&track.key, crate::KeyNotation::Musical)?;
    let base_bpm = track
        .beatgrid
        .first()
        .map_or(track.bpm, |segment| segment.bpm);
    let tags = song.child("Tags");
    for (k, v) in [
        ("Title", &track.title),
        ("Author", &track.artist),
        ("Album", &track.album),
        ("Genre", &track.genre),
        ("Label", &track.label),
        ("Key", &key),
    ] {
        tags.attr(k, v);
    }
    for (attribute, field) in [
        ("Composer", "composer"),
        ("Remixer", "remixer"),
        ("Remix", "mix"),
        ("Grouping", "grouping"),
        ("User1", "extra1"),
        ("User2", "extra2"),
    ] {
        if let Some(value) = track.attribute_text(field) {
            tags.attr(attribute, value);
        } else if track.attributes.contains_key(field) {
            tags.attrs.remove(attribute);
        }
    }
    if let Some(number) = track.attribute_number("trackNumber") {
        ensure!(
            number >= 0.0 && number.fract() == 0.0,
            "Invalid VirtualDJ track number"
        );
        tags.attr("TrackNumber", format!("{number:.0}"));
    } else if track.attributes.contains_key("trackNumber") {
        tags.attrs.remove("TrackNumber");
    }
    tags.attr("Stars", track.rating);
    if let Some(year) = track.year {
        tags.attr("Year", year);
    } else {
        tags.attrs.remove("Year");
    }
    if !options.skip_beatgrids && base_bpm > 0. {
        tags.attr("Bpm", format!("{:.6}", 60. / base_bpm));
    }
    let infos = song.child("Infos");
    if let Some(number) = track.attribute_number("bitrate") {
        ensure!(number >= 0.0, "Invalid VirtualDJ bitrate");
        infos.attr("Bitrate", number);
    } else if track.attributes.contains_key("bitrate") {
        infos.attrs.remove("Bitrate");
    }
    if let Some(date) = track.attribute_date("dateAdded") {
        infos.attr("FirstSeen", date.div_euclid(1000));
    } else if track.attributes.contains_key("dateAdded") {
        infos.attrs.remove("FirstSeen");
    }
    infos.attr("SongLength", format!("{:.6}", track.duration_seconds));
    if let Some(color) = track.color {
        infos.attr("UserColor", color | 0xff000000);
    } else {
        infos.attrs.remove("UserColor");
    }
    song.child("Comment").children = vec![Part::Raw(Event::Text(
        BytesText::new(&track.comment).into_owned(),
    ))];
    let scan = song.child("Scan");
    if !options.skip_beatgrids {
        scan.attr("Version", "801");
        if base_bpm > 0. {
            scan.attr("Bpm", format!("{:.6}", 60. / base_bpm));
        }
        // An old encoded grid must not supersede the replacement POIs.
        scan.attrs.remove("BeatGrid");
    }
    scan.attr("Key", &key);
    let mut previous_pois = BTreeMap::<(String, u16), Vec<Node>>::new();
    for part in std::mem::take(&mut song.children) {
        match part {
            Part::Node(node)
                if node.name == "Poi"
                    && matches!(
                        node.attrs.get("Type").map(String::as_str),
                        Some("cue" | "loop")
                    ) =>
            {
                if let Some(number) = node
                    .attrs
                    .get("Num")
                    .and_then(|value| value.parse::<u16>().ok())
                    .filter(|number| *number > 0)
                {
                    previous_pois
                        .entry((node.attrs["Type"].clone(), number))
                        .or_default()
                        .push(node);
                }
                // Unmatched and unnumbered cue/loop POIs keep the existing
                // complete cue-set replacement policy; no new ownership implied.
            }
            other => song.children.push(other),
        }
    }
    if !options.skip_beatgrids {
        song.children.retain(|part| !matches!(part, Part::Node(n) if n.name == "Poi" && n.attrs.get("Type").is_some_and(|v| v == "beatgrid")));
        // Every exported tempo marker establishes a grid position, in seconds.
        for beat in &track.beatgrid {
            let mut poi = Node::new("Poi");
            poi.attr("Type", "beatgrid");
            poi.attr("Pos", format!("{:.6}", beat.seconds));
            poi.attr("Bpm", format!("{:.6}", beat.bpm));
            song.children.push(Part::Node(poi));
        }
    }
    let mut selected_pois = BTreeSet::new();
    for cue in &track.cues {
        let kind = if cue.end_seconds.is_some() {
            "loop"
        } else {
            "cue"
        };
        let number = u16::from(cue.index) + 1;
        let identity = (kind.to_owned(), number);
        ensure!(
            selected_pois.insert(identity.clone()),
            "Duplicate VirtualDJ source POI identity"
        );
        let mut poi = if let Some(mut matches) = previous_pois.remove(&identity) {
            ensure!(
                matches.len() == 1,
                "Ambiguous VirtualDJ native POI identity"
            );
            matches.pop().unwrap()
        } else {
            Node::new("Poi")
        };
        // Reuse unknown attributes and children, replacing only controlled fields.
        for controlled in ["Color", "Size", "Slot", "AutoTrigger"] {
            poi.attrs.remove(controlled);
        }
        poi.attr("Type", kind);
        poi.attr("Num", number);
        poi.attr("Name", &cue.name);
        poi.attr("Pos", format!("{:.6}", cue.start_seconds));
        if let Some(color) = cue.color {
            poi.attr("Color", color | 0xff000000);
        }
        if let Some(end) = cue.end_seconds {
            ensure!(base_bpm > 0., "A loop requires a valid BPM");
            poi.attr(
                "Size",
                ((end - cue.start_seconds) * base_bpm / 60.).round() as u64,
            );
            poi.attr("Slot", u16::from(cue.index) + 1);
            if cue.active_loop {
                poi.attr("AutoTrigger", "yes");
            }
        }
        song.children.push(Part::Node(poi));
    }
    Ok(())
}
fn safe_name(name: &str, id: &str) -> String {
    let title: String = name
        .chars()
        .take(80)
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let title = title.trim_matches([' ', '.']);
    let hash = format!("{:x}", Sha256::digest(id.as_bytes()));
    format!(
        "{}-{}",
        if title.is_empty() { "Playlist" } else { title },
        &hash[..8]
    )
}
fn playlist_component(node: &crate::SyncPlaylist) -> String {
    format!(
        "{}.{}",
        safe_name(&node.name, &node.id),
        if node.folder {
            "subfolders"
        } else {
            "vdjfolder"
        }
    )
}
fn playlist_files(
    root: &Path,
    library: &SyncLibrary,
    options: &SyncOptions,
) -> Result<Vec<(PathBuf, Vec<u8>)>> {
    let mut paths = BTreeMap::new();
    let nodes: BTreeMap<_, _> = library
        .playlists
        .iter()
        .map(|p| (p.id.as_str(), p))
        .collect();
    for node in &library.playlists {
        let mut parts = vec![playlist_component(node)];
        let mut parent = node.parent_id.as_deref();
        let mut seen = BTreeSet::from([node.id.as_str()]);
        while let Some(id) = parent {
            ensure!(seen.insert(id), "Playlist hierarchy contains a cycle");
            let p = nodes.get(id).context("Playlist parent is missing")?;
            ensure!(p.folder, "Playlist parent must be a folder");
            parts.push(playlist_component(p));
            parent = p.parent_id.as_deref();
        }
        let mut path = root.join("MyLists/CueRithm.subfolders");
        for part in parts.into_iter().rev() {
            path.push(part);
        }
        paths.insert(node.id.as_str(), path);
    }
    let tracks: BTreeMap<_, _> = library.tracks.iter().map(|t| (t.id.as_str(), t)).collect();
    let mut output = vec![];
    let mut orders: BTreeMap<PathBuf, Vec<String>> = BTreeMap::new();
    for node in &library.playlists {
        let path = &paths[node.id.as_str()];
        orders
            .entry(path.parent().unwrap().into())
            .or_default()
            .push(path.file_stem().unwrap().to_string_lossy().into_owned());
        if node.folder {
            orders.entry(path.clone()).or_default();
            continue;
        }
        if let Ok(Some(crate::smart_rules::NativeSmart::VirtualDj(filter))) =
            crate::smart_rules::compile(node, library, options, crate::DjSoftware::VirtualDj)
        {
            let mut list = Node::new("FilterFolder");
            list.attr("scope", "database");
            list.attr("filter", filter);
            output.push((path.clone(), list.bytes()?));
            continue;
        }
        let mut list = Node::new("VirtualFolder");
        for (idx, id) in node.track_ids.iter().enumerate() {
            let track = tracks
                .get(id.as_str())
                .context("Playlist references a missing track")?;
            let mut song = Node::new("song");
            song.attr("path", track.path.to_str().context("Non-UTF-8 path")?);
            song.attr("idx", idx);
            list.children.push(Part::Node(song));
        }
        output.push((path.clone(), list.bytes()?));
    }
    for (dir, order) in orders {
        output.push((
            dir.join("order"),
            if order.is_empty() {
                Vec::new()
            } else {
                format!("{}\r\n", order.join("\r\n")).into_bytes()
            },
        ));
    }
    Ok(output)
}
/// `path` is the VirtualDJ home folder, or its `database.xml`.
/// Only matching tracks and CueRithm's MyLists namespace are updated.
pub fn write(path: &Path, library: &SyncLibrary, options: &SyncOptions) -> Result<()> {
    let database = if path.is_dir() {
        path.join("database.xml")
    } else {
        path.to_path_buf()
    };
    let home = database.parent().context("VirtualDJ home folder missing")?;
    let mut root = parse(&fs::read(&database)?)?;
    ensure!(
        root.name == "VirtualDJ_Database",
        "Not a VirtualDJ database"
    );
    ensure!(
        root.attrs.get("Version").is_some_and(|v| v == "8.5"),
        "Unsupported VirtualDJ database version"
    );
    let mut indices = BTreeMap::new();
    for (i, part) in root.children.iter().enumerate() {
        if let Part::Node(node) = part
            && node.name == "Song"
            && let Some(path) = node.attrs.get("FilePath")
        {
            ensure!(
                indices.insert(path_key(path), i).is_none(),
                "Duplicate VirtualDJ track paths"
            );
        }
    }
    for track in &library.tracks {
        let key = path_key(track.path.to_str().context("Non-UTF-8 path")?);
        if let Some(index) = indices.get(&key) {
            if let Part::Node(song) = &mut root.children[*index] {
                track_xml(song, track, options)?;
            }
        } else {
            let mut song = Node::new("Song");
            track_xml(&mut song, track, options)?;
            root.children.push(Part::Node(song));
        }
    }
    // Finish all serialization and validation before writing any target file.
    let mut files = playlist_files(home, library, options)?;
    files.push((database, root.bytes()?));
    for (path, bytes) in files {
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(path, bytes)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SyncBeat, SyncCue, SyncPlaylist};
    #[test]
    fn native_units_escaping_and_unknown_data_survive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("database.xml");
        fs::write(&path,r#"<VirtualDJ_Database Version="8.5"><Song FilePath="/music/A &amp; B.wav" Custom="keep"><Tags User1="retained"/><Scan Phase="1" BeatGrid="old"/><Poi Type="automix" Pos="9"/><Poi Type="cue" Pos="1"/></Song><Song FilePath="/other.wav"/></VirtualDJ_Database>"#).unwrap();
        let library = SyncLibrary {
            tracks: vec![SyncTrack {
                id: "a".into(),
                path: "/music/A & B.wav".into(),
                title: "<Hello> & friends".into(),
                bpm: 120.,
                duration_seconds: 60.,
                cues: vec![SyncCue {
                    active_loop: false,
                    index: 0,
                    name: "Drop & mix".into(),
                    start_seconds: 4.,
                    end_seconds: Some(8.),
                    color: Some(0xff0000),
                }],
                beatgrid: vec![SyncBeat {
                    seconds: 0.5,
                    bpm: 120.,
                    beat: 1,
                }],
                ..Default::default()
            }],
            playlists: vec![SyncPlaylist {
                id: "p".into(),
                name: "../Mix".into(),
                track_ids: vec!["a".into()],
                ..Default::default()
            }],
        };
        write(&path, &library, &SyncOptions::default()).unwrap();
        let xml = fs::read_to_string(&path).unwrap();
        assert!(xml.contains("Bpm=\"0.500000\""));
        assert!(xml.contains("Size=\"8\""));
        assert!(xml.contains("Num=\"1\""));
        assert!(xml.contains("&lt;Hello&gt; &amp; friends"));
        assert!(xml.contains("Type=\"automix\""));
        assert!(xml.contains("User1=\"retained\""));
        assert!(xml.contains("Custom=\"keep\""));
        assert!(xml.contains("/other.wav"));
        assert!(!xml.contains("BeatGrid="));
        write(&path, &library, &SyncOptions::default()).unwrap();
        assert_eq!(xml, fs::read_to_string(&path).unwrap());
        let files: Vec<_> = fs::read_dir(dir.path().join("MyLists/CueRithm.subfolders"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert!(
            files
                .iter()
                .any(|f| f.extension().is_some_and(|x| x == "vdjfolder"))
        );
    }
    #[test]
    fn only_active_loops_autotrigger_and_foreign_pois_survive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("database.xml");
        fs::write(&path, r#"<VirtualDJ_Database Version="8.5"><Song FilePath="/synthetic.wav"><Poi Type="automix" Pos="9" AutoTrigger="keep"/><Poi Type="loop" Name="old" AutoTrigger="yes"/></Song></VirtualDJ_Database>"#).unwrap();
        let mut library = SyncLibrary {
            tracks: vec![SyncTrack {
                id: "track".into(),
                path: "/synthetic.wav".into(),
                bpm: 120.,
                duration_seconds: 20.,
                cues: vec![
                    SyncCue {
                        index: 0,
                        name: "active".into(),
                        start_seconds: 1.,
                        end_seconds: Some(3.),
                        active_loop: true,
                        ..Default::default()
                    },
                    SyncCue {
                        index: 1,
                        name: "saved".into(),
                        start_seconds: 5.,
                        end_seconds: Some(7.),
                        ..Default::default()
                    },
                    SyncCue {
                        index: 2,
                        name: "cue".into(),
                        start_seconds: 8.,
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        };
        write(&path, &library, &SyncOptions::default()).unwrap();
        let xml = fs::read_to_string(&path).unwrap();
        assert_eq!(xml.matches("AutoTrigger=\"yes\"").count(), 1);
        assert!(xml.contains("AutoTrigger=\"keep\""));
        assert!(xml.contains("Type=\"automix\""));
        assert!(!xml.contains("Name=\"old\""));
        library.tracks[0].cues[0].active_loop = false;
        write(&path, &library, &SyncOptions::default()).unwrap();
        assert!(
            !fs::read_to_string(path)
                .unwrap()
                .contains("AutoTrigger=\"yes\"")
        );
    }
    #[test]
    fn preserve_grids_keeps_scan_and_grid_pois() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("database.xml");
        fs::write(&path,r#"<VirtualDJ_Database Version="8.5"><Song FilePath="/a"><Scan BeatGrid="original" Bpm="0.75" Phase="0.4"/><Poi Type="beatgrid" Pos="2.5" Bpm="80"/></Song></VirtualDJ_Database>"#).unwrap();
        let library = SyncLibrary {
            tracks: vec![SyncTrack {
                id: "a".into(),
                path: "/a".into(),
                bpm: 120.,
                ..Default::default()
            }],
            ..Default::default()
        };
        write(
            &path,
            &library,
            &SyncOptions {
                skip_beatgrids: true,
                ..Default::default()
            },
        )
        .unwrap();
        let xml = fs::read_to_string(path).unwrap();
        assert!(xml.contains("BeatGrid=\"original\""));
        assert!(xml.contains("Pos=\"2.5\""));
        assert!(xml.contains("Bpm=\"0.75\""));
    }
}

#[cfg(test)]
mod native_smart_tests {
    use super::*;
    use crate::SyncPlaylist;
    use cuerithm_smartlists::{Operator, RuleValue, SmartDefinition, SmartRule};
    #[test]
    fn native_filter_folder_toggle_and_empty_folder_are_real_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(
            root.join("database.xml"),
            b"<VirtualDJ_Database Version=\"8.5\"/>",
        )
        .unwrap();
        let library = SyncLibrary {
            playlists: vec![
                SyncPlaylist {
                    id: "empty".into(),
                    name: "Empty".into(),
                    folder: true,
                    ..Default::default()
                },
                SyncPlaylist {
                    id: "smart".into(),
                    name: "Native".into(),
                    smart: Some(SmartDefinition {
                        match_all: true,
                        rules: vec![SmartRule {
                            id: "artist".into(),
                            field: "artist".into(),
                            operator: Operator::StringContains,
                            value: RuleValue::Text("Fixture & mix".into()),
                            or: false,
                        }],
                    }),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        write(root, &library, &SyncOptions::default()).unwrap();
        let plans = playlist_files(root, &library, &SyncOptions::default()).unwrap();
        let (native, _) = plans
            .iter()
            .find(|(path, _)| {
                path.extension()
                    .is_some_and(|extension| extension == "vdjfolder")
            })
            .unwrap();
        let tree = parse(&fs::read(native).unwrap()).unwrap();
        assert_eq!(tree.name, "FilterFolder");
        assert_eq!(tree.attrs["filter"], "Artist contains \"Fixture & mix\"");
        let (order, bytes) = plans
            .iter()
            .find(|(path, _)| {
                path.parent()
                    .unwrap()
                    .extension()
                    .is_some_and(|extension| extension == "subfolders")
                    && path
                        .parent()
                        .unwrap()
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with("Empty")
            })
            .unwrap();
        assert_eq!(order.file_name().unwrap(), "order");
        assert!(bytes.is_empty());
        assert!(order.exists());
        write(
            root,
            &library,
            &SyncOptions {
                materialize_smartlists: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            parse(&fs::read(native).unwrap()).unwrap().name,
            "VirtualFolder"
        );
    }
}

#[cfg(test)]
mod prepared_attribute_tests {
    use super::*;
    use cuerithm_smartlists::FieldValue;
    #[test]
    fn exports_prepared_metadata_used_by_native_rules() {
        let mut track = SyncTrack {
            path: "/synthetic/missing.flac".into(),
            ..Default::default()
        };
        track.attributes = BTreeMap::from([
            ("composer".into(), FieldValue::Text("Composer".into())),
            ("remixer".into(), FieldValue::Text("Remixer".into())),
            ("mix".into(), FieldValue::Text("Mix".into())),
            ("grouping".into(), FieldValue::Text("Group".into())),
            ("extra1".into(), FieldValue::Text("One".into())),
            ("extra2".into(), FieldValue::Text("Two".into())),
            ("trackNumber".into(), FieldValue::Number(7.0)),
            ("bitrate".into(), FieldValue::Number(320.0)),
            ("sizeBytes".into(), FieldValue::Number(12345.0)),
            ("dateAdded".into(), FieldValue::Date(1792447200123)),
        ]);
        let mut song = Node::new("Song");
        track_xml(&mut song, &track, &SyncOptions::default()).unwrap();
        assert_eq!(song.attrs["FileSize"], "12345");
        let tags = song.child("Tags");
        for (field, value) in [
            ("Composer", "Composer"),
            ("Remixer", "Remixer"),
            ("Remix", "Mix"),
            ("Grouping", "Group"),
            ("User1", "One"),
            ("User2", "Two"),
            ("TrackNumber", "7"),
        ] {
            assert_eq!(tags.attrs[field], value);
        }
        let infos = song.child("Infos");
        assert_eq!(infos.attrs["Bitrate"], "320");
        assert_eq!(infos.attrs["FirstSeen"], "1792447200");
    }
}

pub(crate) fn reference_id(database: &Path, source: &Path) -> Result<String> {
    let source = source
        .to_str()
        .context("VirtualDJ requires a Unicode source path")?;
    let root = parse(&fs::read(database)?)?;
    let found = root
        .children
        .iter()
        .filter_map(|part| match part {
            Part::Node(node) if node.name == "Song" => node
                .attrs
                .get("FilePath")
                .filter(|path| path_key(path) == path_key(source)),
            _ => None,
        })
        .collect::<Vec<_>>();
    ensure!(
        found.len() == 1,
        "VirtualDJ source reference is missing or ambiguous"
    );
    if found[0] != source {
        ensure!(
            same_file::is_same_file(Path::new(found[0]), Path::new(source))?,
            "VirtualDJ normalized source path identifies another physical file"
        );
    }
    Ok(path_key(source))
}
pub(crate) fn repair_reference_bytes(
    bytes: &[u8],
    source: &Path,
    destination: &Path,
    database: bool,
) -> Result<Option<Vec<u8>>> {
    let source = source
        .to_str()
        .context("VirtualDJ requires a Unicode source path")?;
    let destination = destination
        .to_str()
        .context("VirtualDJ requires a Unicode destination path")?;
    let mut root = parse(bytes)?;
    if database {
        let target = path_key(destination);
        let old = path_key(source);
        ensure!(!root.children.iter().any(|part| matches!(part,Part::Node(node) if node.name=="Song" && node.attrs.get("FilePath").is_some_and(|p|path_key(p)==target && path_key(p)!=old))),"VirtualDJ destination already has a native track");
    }
    fn replace(node: &mut Node, source: &str, destination: &str, count: &mut usize) -> Result<()> {
        if node.name.eq_ignore_ascii_case("song") {
            for key in ["FilePath", "path"] {
                let matches = match node.attrs.get(key) {
                    Some(path) if path_key(path) == path_key(source) => {
                        path == source || same_file::is_same_file(Path::new(path), Path::new(source))
                            .context("Unresolvable normalized VirtualDJ reference; restore its source or repair this alias before organization")?
                    }
                    _ => false,
                };
                if matches {
                    node.attr(key, destination);
                    *count += 1;
                }
            }
        }
        for child in &mut node.children {
            if let Part::Node(child) = child {
                replace(child, source, destination, count)?;
            }
        }
        Ok(())
    }
    let mut count = 0;
    replace(&mut root, source, destination, &mut count)?;
    if database {
        ensure!(
            count == 1,
            "VirtualDJ source reference changed or is ambiguous"
        );
    }
    if count == 0 {
        Ok(None)
    } else {
        Ok(Some(root.bytes()?))
    }
}

#[cfg(test)]
mod path_reference_tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn a_proven_distinct_normalized_playlist_path_is_preserved() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("Music")).unwrap();
        let source = dir.path().join("Music/song.wav");
        let distinct = dir.path().join("Music\\song.wav");
        fs::write(&source, b"source").unwrap();
        fs::write(&distinct, b"distinct").unwrap();
        assert_eq!(
            path_key(source.to_str().unwrap()),
            path_key(distinct.to_str().unwrap())
        );
        let playlist = format!(
            r#"<VirtualFolder><song path="{}"/></VirtualFolder>"#,
            distinct.display()
        );
        assert!(
            repair_reference_bytes(
                playlist.as_bytes(),
                &source,
                &dir.path().join("managed.wav"),
                false
            )
            .unwrap()
            .is_none()
        );
    }
    #[test]
    fn an_unproven_case_alias_in_a_playlist_blocks_repair() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("missing-original.wav");
        let alias = dir.path().join("MISSING-ORIGINAL.wav");
        let playlist = format!(
            r#"<VirtualFolder><song path="{}"/></VirtualFolder>"#,
            alias.display()
        );
        assert!(
            repair_reference_bytes(
                playlist.as_bytes(),
                &source,
                &dir.path().join("managed.wav"),
                false
            )
            .unwrap_err()
            .to_string()
            .contains("Unresolvable normalized VirtualDJ reference")
        );
    }
}

#[cfg(test)]
mod poi_preservation_tests {
    use super::*;
    use crate::SyncCue;

    fn fixture(pois: &str, cues: Vec<SyncCue>) -> (tempfile::TempDir, PathBuf, SyncLibrary) {
        let folder = tempfile::tempdir().unwrap();
        let database = folder.path().join("database.xml");
        fs::write(
            &database,
            format!(r#"<VirtualDJ_Database Version="8.5"><Song FilePath="/synthetic.wav">{pois}<Poi Type="automix" Pos="9" Native="foreign"/></Song><Song FilePath="/foreign.wav" Native="unrelated"/></VirtualDJ_Database>"#),
        )
        .unwrap();
        let input = SyncLibrary {
            tracks: vec![SyncTrack {
                id: "synthetic".into(),
                path: "/synthetic.wav".into(),
                bpm: 120.,
                duration_seconds: 20.,
                cues,
                ..Default::default()
            }],
            ..Default::default()
        };
        (folder, database, input)
    }
    fn pois(database: &Path) -> Vec<Node> {
        let root = parse(&fs::read(database).unwrap()).unwrap();
        let song = root
            .children
            .into_iter()
            .find_map(|part| match part {
                Part::Node(node) if node.name == "Song" => Some(node),
                _ => None,
            })
            .unwrap();
        song.children
            .into_iter()
            .filter_map(|part| match part {
                Part::Node(node) if node.name == "Poi" => Some(node),
                _ => None,
            })
            .collect()
    }
    #[test]
    fn matched_cue_retains_native_attributes_and_nested_payload() {
        let (_folder, database, input) = fixture(
            r#"<Poi Type="cue" Num="1" Name="old" Pos="1" Color="123" Size="8" Slot="1" AutoTrigger="yes" Point="native-point" Extension="keep"><NativeData Mode="opaque"><Value Key="sentinel"/><!--native-comment--></NativeData></Poi>"#,
            vec![SyncCue {
                index: 0,
                name: "new".into(),
                start_seconds: 2.,
                ..Default::default()
            }],
        );
        write(&database, &input, &SyncOptions::default()).unwrap();
        let nodes = pois(&database);
        let cue = nodes
            .iter()
            .find(|node| node.attrs.get("Type").is_some_and(|ty| ty == "cue"))
            .unwrap();
        assert_eq!(cue.attrs["Name"], "new");
        assert_eq!(cue.attrs["Pos"], "2.000000");
        assert_eq!(cue.attrs["Extension"], "keep");
        assert_eq!(cue.attrs["Point"], "native-point");
        for controlled in ["Color", "Size", "Slot", "AutoTrigger"] {
            assert!(!cue.attrs.contains_key(controlled));
        }
        let Part::Node(payload) = &cue.children[0] else {
            panic!("native child lost")
        };
        assert_eq!(payload.name, "NativeData");
        assert_eq!(payload.attrs["Mode"], "opaque");
        let Part::Node(value) = &payload.children[0] else {
            panic!("nested child lost")
        };
        assert_eq!(value.attrs["Key"], "sentinel");
        assert!(
            matches!(&payload.children[1], Part::Raw(Event::Comment(comment)) if comment.as_ref() == "native-comment")
        );
        assert!(nodes.iter().any(|node| {
            node.attrs
                .get("Native")
                .is_some_and(|value| value == "foreign")
        }));
        let first = fs::read(&database).unwrap();
        assert!(String::from_utf8_lossy(&first).contains("Native=\"unrelated\""));
        write(&database, &input, &SyncOptions::default()).unwrap();
        assert_eq!(fs::read(database).unwrap(), first);
    }
    #[test]
    fn matched_loop_retains_payload_and_clears_stale_activation() {
        let (_folder, database, mut input) = fixture(
            r#"<Poi Type="loop" Num="2" Slot="2" Name="old" Pos="1" Size="8" AutoTrigger="yes" Color="123" Extension="loop-state"><NativeLoop Value="keep"/></Poi>"#,
            vec![SyncCue {
                index: 1,
                name: "saved".into(),
                start_seconds: 2.,
                end_seconds: Some(4.),
                ..Default::default()
            }],
        );
        write(&database, &input, &SyncOptions::default()).unwrap();
        let nodes = pois(&database);
        let saved = nodes
            .iter()
            .find(|node| node.attrs.get("Type").is_some_and(|ty| ty == "loop"))
            .unwrap();
        assert_eq!(saved.attrs["Pos"], "2.000000");
        assert_eq!(saved.attrs["Size"], "4");
        assert_eq!(saved.attrs["Slot"], "2");
        assert!(!saved.attrs.contains_key("AutoTrigger"));
        assert!(!saved.attrs.contains_key("Color"));
        assert_eq!(saved.attrs["Extension"], "loop-state");
        let Part::Node(payload) = &saved.children[0] else {
            panic!("native loop child lost")
        };
        assert_eq!(payload.attrs["Value"], "keep");
        input.tracks[0].cues[0].active_loop = true;
        write(&database, &input, &SyncOptions::default()).unwrap();
        let nodes = pois(&database);
        let active = nodes
            .iter()
            .find(|node| node.attrs.get("Type").is_some_and(|ty| ty == "loop"))
            .unwrap();
        assert_eq!(active.attrs["AutoTrigger"], "yes");
        assert_eq!(active.attrs["Extension"], "loop-state");
    }
    #[test]
    fn type_changes_do_not_transplant_unrelated_native_payload() {
        let (_folder, database, input) = fixture(
            r#"<Poi Type="cue" Num="3" Pos="1" CueOnly="old"><CuePayload/></Poi><Poi Type="loop" Num="4" Pos="3" LoopOnly="foreign"/>"#,
            vec![SyncCue {
                index: 2,
                start_seconds: 2.,
                end_seconds: Some(4.),
                ..Default::default()
            }],
        );
        write(&database, &input, &SyncOptions::default()).unwrap();
        let nodes = pois(&database);
        let native = nodes
            .iter()
            .filter(|node| {
                matches!(
                    node.attrs.get("Type").map(String::as_str),
                    Some("cue" | "loop")
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(native.len(), 1);
        assert_eq!(native[0].attrs["Type"], "loop");
        assert_eq!(native[0].attrs["Num"], "3");
        assert!(!native[0].attrs.contains_key("CueOnly"));
        assert!(!native[0].attrs.contains_key("LoopOnly"));
        assert!(native[0].children.is_empty());
    }
    #[test]
    fn ambiguous_poi_identity_preserves_source_and_target_bytes() {
        for duplicate_source in [false, true] {
            let native = if duplicate_source {
                r#"<Poi Type="cue" Num="1" Pos="1" Extension="first"/>"#
            } else {
                r#"<Poi Type="cue" Num="1" Pos="1" Extension="first"/><Poi Type="cue" Num="01" Pos="2" Extension="second"/>"#
            };
            let cue = SyncCue {
                index: 0,
                start_seconds: 2.,
                ..Default::default()
            };
            let (folder, database, mut input) = fixture(
                native,
                if duplicate_source {
                    vec![cue.clone(), cue]
                } else {
                    vec![cue]
                },
            );
            let source = folder.path().join("source.wav");
            fs::write(&source, b"synthetic source bytes").unwrap();
            input.tracks[0].path = source.clone();
            let original = fs::read_to_string(&database)
                .unwrap()
                .replace("/synthetic.wav", source.to_str().unwrap());
            fs::write(&database, &original).unwrap();
            assert!(write(&database, &input, &SyncOptions::default()).is_err());
            assert_eq!(fs::read(&database).unwrap(), original.as_bytes());
            assert_eq!(fs::read(&source).unwrap(), b"synthetic source bytes");
        }
    }
    #[test]
    fn unmatched_and_unnumbered_cues_keep_existing_replacement_scope() {
        let (_folder, database, input) = fixture(
            r#"<Poi Type="cue" Num="4" Name="stale"/><Poi Type="loop" Num="-1" Slot="3" Name="slot-only"/><Poi Type="loop" Name="unnumbered"/><Poi Type="action" Num="7" Action="native-action"/><Poi Type="load" Name="native-load"/>"#,
            vec![SyncCue {
                index: 0,
                name: "replacement".into(),
                start_seconds: 2.,
                ..Default::default()
            }],
        );
        write(&database, &input, &SyncOptions::default()).unwrap();
        let nodes = pois(&database);
        let replacements = nodes
            .iter()
            .filter(|node| {
                matches!(
                    node.attrs.get("Type").map(String::as_str),
                    Some("cue" | "loop")
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(replacements.len(), 1);
        assert_eq!(replacements[0].attrs["Name"], "replacement");
        assert!(nodes.iter().any(|node| {
            node.attrs
                .get("Action")
                .is_some_and(|value| value == "native-action")
        }));
        assert!(nodes.iter().any(|node| {
            node.attrs
                .get("Name")
                .is_some_and(|value| value == "native-load")
        }));
    }
}
