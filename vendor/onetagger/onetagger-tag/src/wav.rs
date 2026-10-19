use anyhow::Error;
use id3::{Frame, Tag, TagLike, Version};
use once_cell::sync::Lazy;
use riff::{Chunk, ChunkContents, ChunkId, LIST_ID, RIFF_ID};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Cursor, Seek, SeekFrom, Write};
use std::path::Path;

/// RIFF `id3 ` chunk
static ID3_ID_1: ChunkId = ChunkId {
    value: [0x69, 0x64, 0x33, 0x20],
};
static ID3_ID_2: ChunkId = ChunkId {
    value: [0x49, 0x44, 0x33, 0x20],
};
/// RIFF WAVE chunk
static WAVE_ID: ChunkId = ChunkId {
    value: [0x57, 0x41, 0x56, 0x45],
};
static INFO_ID: ChunkId = ChunkId {
    value: [0x49, 0x4E, 0x46, 0x4F],
};

/// ID3 Frame to RIFF ChunkID
static ID3_RIFF: Lazy<HashMap<&'static str, ChunkId>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert("TIT2", ChunkId::new("INAM").unwrap());
    m.insert("TALB", ChunkId::new("IPRD").unwrap());
    m.insert("TPE1", ChunkId::new("IART").unwrap());
    m.insert("COMM", ChunkId::new("ICMT").unwrap());
    m.insert("TCON", ChunkId::new("IGNR").unwrap());
    m.insert("TSRC", ChunkId::new("ISRC").unwrap());
    m.insert("TDRC", ChunkId::new("ICRD").unwrap());
    m.insert("TRCK", ChunkId::new("ITRK").unwrap());
    m
});

/// Resolve only the top-level WAVE container and its INFO lists. Other LIST
/// types (for example adtl cue labels) stay opaque and retain their type/data.
fn chunks(file: &mut BufReader<File>) -> Result<(Vec<Chunk>, Vec<Chunk>), Error> {
    let root = Chunk::read(file, 0)?;
    anyhow::ensure!(
        root.id() == RIFF_ID && root.read_type(file)? == WAVE_ID,
        "Unsupported WAV metadata container"
    );
    anyhow::ensure!(
        root.len() as u64 + 8 == file.get_ref().metadata()?.len(),
        "WAV contains truncated or trailing data; refusing metadata rewrite"
    );
    let children = root.iter(file).collect::<Result<Vec<_>, _>>()?;
    let mut data = vec![];
    let mut info = vec![];
    for child in children {
        if child.id() == LIST_ID && child.read_type(file)? == INFO_ID {
            info.extend(child.iter(file).collect::<Result<Vec<_>, _>>()?);
        } else {
            data.push(child);
        }
    }
    Ok((data, info))
}

/// Synchronize mirrored INFO fields with the ID3 tag, preserving every other
/// top-level chunk and all unrecognized INFO fields.
pub(crate) fn write_wav(path: impl AsRef<Path>, tag: Tag, version: Version) -> Result<(), Error> {
    let mut file = BufReader::new(File::open(&path)?);
    let (data, info) = chunks(&mut file)?;
    let mut riff_data = vec![];
    for chunk in data {
        if chunk.id() == ID3_ID_1 || chunk.id() == ID3_ID_2 {
            continue;
        }
        riff_data.push(ChunkContents::Data(
            chunk.id(),
            chunk.read_contents(&mut file)?,
        ));
    }
    let mut bytes = vec![];
    tag.write_to(Cursor::new(&mut bytes), version)?;
    riff_data.push(ChunkContents::Data(ID3_ID_1.clone(), bytes));
    let mut list_data = vec![];
    // ID3v2.3 splits its date between TYER and TDAT. INFO has one date field.
    let date = tag.date_recorded().map(|d| d.to_string()).or_else(|| {
        let year = tag.get("TYER")?.content().text()?.parse::<i32>().ok()?;
        let md = tag
            .get("TDAT")
            .and_then(|f| f.content().text())
            .filter(|s| s.len() == 4 && s.bytes().all(|c| c.is_ascii_digit()))
            .and_then(|s| {
                let day = s[..2].parse::<u32>().ok()?;
                let month = s[2..].parse::<u32>().ok()?;
                chrono::NaiveDate::from_ymd_opt(year, month, day)
            });
        Some(
            md.map(|d| d.to_string())
                .unwrap_or_else(|| format!("{year:04}")),
        )
    });
    if let Some(date) = date {
        list_data.push(ChunkContents::Data(
            ID3_RIFF["TDRC"].clone(),
            date.into_bytes(),
        ));
    }
    for frame in tag.frames() {
        if let Some(chunk_id) = ID3_RIFF.get(frame.id()) {
            if list_data
                .iter()
                .any(|c| matches!(c,ChunkContents::Data(id,_) if id==chunk_id))
            {
                continue;
            }
            let text = if frame.id() == "COMM" {
                frame
                    .content()
                    .comment()
                    .filter(|c| c.description.is_empty())
                    .map(|c| c.text.as_str())
            } else {
                frame.content().text()
            };
            if let Some(text) = text {
                list_data.push(ChunkContents::Data(
                    chunk_id.clone(),
                    text.as_bytes().to_vec(),
                ));
            }
        }
    }
    for chunk in info {
        // A missing mirrored ID3 field means it was cleared. Retaining its old
        // INFO value would resurrect it on the next metadata read.
        if ID3_RIFF.values().any(|id| id == &chunk.id()) {
            continue;
        }
        if list_data
            .iter()
            .any(|c| matches!(c,ChunkContents::Data(id,_) if id==&chunk.id()))
        {
            continue;
        }
        list_data.push(ChunkContents::Data(
            chunk.id(),
            chunk.read_contents(&mut file)?,
        ));
    }
    riff_data.push(ChunkContents::Children(
        LIST_ID.clone(),
        INFO_ID.clone(),
        list_data,
    ));
    let root = ChunkContents::Children(RIFF_ID.clone(), WAVE_ID.clone(), riff_data);
    let mut output = BufWriter::new(File::create(path)?);
    root.write(&mut output)?;
    output.flush()?;
    Ok(())
}

/// Read INFO fallbacks only when their corresponding ID3 field is absent.
pub(crate) fn read_wav(path: impl AsRef<Path>) -> Result<Tag, Error> {
    let mut file = BufReader::new(File::open(path)?);
    let (_, info) = chunks(&mut file)?;
    file.seek(SeekFrom::Start(0))?;
    let mut tag = Tag::read_from2(&mut file).unwrap_or(Tag::new());
    for (field, id) in ID3_RIFF.iter() {
        if tag.get(field).is_none() {
            if let Some(chunk) = info.iter().find(|c| &c.id() == id) {
                if let Ok(text) = String::from_utf8(chunk.read_contents(&mut file)?) {
                    let text = text.replace("\0", "");
                    if *field == "COMM" {
                        tag.add_frame(id3::frame::Comment {
                            lang: "eng".into(),
                            description: String::new(),
                            text,
                        });
                    } else {
                        tag.add_frame(Frame::text(field, text));
                    }
                }
            }
        }
    }
    Ok(tag)
}
