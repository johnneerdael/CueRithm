use cuerithm_dj_sync::import::*;
use cuerithm_media::CancellationToken;
use sha2::{Digest, Sha256};
use std::fs;
fn frame(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = id.to_vec();
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(data);
    out
}
#[test]
fn file_tag_capture_preserves_repeated_native_identity_and_binary_bytes_without_dsp() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.mp3");
    let mut body = frame(b"TIT2", b"\0File title");
    body.extend(frame(b"COMM", b"\0engfirst\0Comment one"));
    body.extend(frame(b"COMM", b"\0deusecond\0Comment two"));
    body.extend(frame(b"XZZZ", &[0, 255, 7]));
    body.extend(frame(b"APIC", b"\0image/png\0\x03cover\0\x89PNG\r\n\x1a\n"));
    let n = body.len() as u32;
    let mut bytes = b"ID3\x03\0\0".to_vec();
    bytes.extend_from_slice(&[
        ((n >> 21) & 127) as u8,
        ((n >> 14) & 127) as u8,
        ((n >> 7) & 127) as u8,
        (n & 127) as u8,
    ]);
    bytes.extend(body);
    let media = include_bytes!("../src/mixxx/fixtures/no_tag.mp3");
    bytes.extend_from_slice(media);
    fs::write(&path, &bytes).unwrap();
    let before = format!("{:x}", Sha256::digest(&bytes));
    let result = read_file_tags(
        &path,
        &ImportLimits::default(),
        &CancellationToken::default(),
    )
    .unwrap();
    assert_eq!(result.source_sha256, before);
    assert_eq!(
        result
            .records
            .iter()
            .filter(|r| r.native_id == "COMM")
            .count(),
        2
    );
    let unknown = result
        .records
        .iter()
        .find(|r| r.native_id == "XZZZ")
        .unwrap();
    assert_eq!(
        result.blobs.get(&unknown.payload.sha256).unwrap(),
        &vec![0, 255, 7]
    );
    assert_eq!(
        result
            .records
            .iter()
            .filter(|r| r.native_id == "APIC")
            .count(),
        1
    );
    let first_comment = result
        .records
        .iter()
        .find(|r| r.native_id == "COMM")
        .unwrap();
    assert_eq!(
        first_comment.identity.get("language").map(String::as_str),
        Some("eng")
    );
    assert_eq!(
        first_comment
            .identity
            .get("description")
            .map(String::as_str),
        Some("first")
    );
    assert_eq!(result.artwork.len(), 1);
    assert_eq!(result.artwork[0].mime_type.as_deref(), Some("image/png"));
    assert_eq!(result.artwork[0].picture_type, Some(3));
    assert_eq!(result.artwork[0].description.as_deref(), Some("cover"));
    assert_eq!(
        result.blobs[&result.artwork[0].payload.sha256],
        b"\x89PNG\r\n\x1a\n"
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
}
#[test]
fn tag_limits_and_precedence_do_not_mutate_native_values() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.mp3");
    fs::write(&path, b"ID3\x04\0\0\x7f\x7f\x7f\x7f").unwrap();
    assert!(
        read_file_tags(
            &path,
            &ImportLimits::default(),
            &CancellationToken::default()
        )
        .is_err()
    );
    assert_eq!(
        select_imported_value(
            &Presence::Empty(serde_json::json!("")),
            Some(&Presence::Value(serde_json::json!("File")))
        ),
        Presence::Empty(serde_json::json!(""))
    );
    assert_eq!(
        select_imported_value(
            &Presence::Absent,
            Some(&Presence::Value(serde_json::json!("File")))
        ),
        Presence::Value(serde_json::json!("File"))
    );
}

fn vorbis_comments() -> Vec<u8> {
    let mut b = 4u32.to_le_bytes().to_vec();
    b.extend(b"test");
    b.extend(3u32.to_le_bytes());
    for value in ["TITLE=File title", "X-REPEAT=one", "X-REPEAT=two"] {
        b.extend((value.len() as u32).to_le_bytes());
        b.extend(value.as_bytes());
    }
    b
}
fn mp4_atom(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut b = ((data.len() + 8) as u32).to_be_bytes().to_vec();
    b.extend(id);
    b.extend(data);
    b
}
fn chunk(id: &[u8; 4], data: &[u8], be: bool) -> Vec<u8> {
    let mut b = id.to_vec();
    b.extend(if be {
        (data.len() as u32).to_be_bytes()
    } else {
        (data.len() as u32).to_le_bytes()
    });
    b.extend(data);
    if !data.len().is_multiple_of(2) {
        b.push(0);
    }
    b
}
#[test]
fn flac_and_ogg_keep_repeated_comments_and_raw_attachments() {
    let d = tempfile::tempdir().unwrap();
    let comments = vorbis_comments();
    let mut flac = b"fLaC".to_vec();
    flac.extend([4, 0, 0, comments.len() as u8]);
    flac.extend(&comments);
    flac.extend([0x86, 0, 0, 3, 1, 2, 3]);
    fs::write(d.path().join("song.flac"), flac).unwrap();
    let mut tags = b"OpusTags".to_vec();
    tags.extend(&comments);
    let mut ogg = b"OggS".to_vec();
    ogg.extend([0; 22]);
    ogg.push(2);
    ogg.extend([8, tags.len() as u8]);
    ogg.extend(b"OpusHead");
    ogg.extend(tags);
    fs::write(d.path().join("song.opus"), ogg).unwrap();
    for file in ["song.flac", "song.opus"] {
        let out = read_file_tags(
            &d.path().join(file),
            &ImportLimits::default(),
            &CancellationToken::default(),
        )
        .unwrap();
        assert_eq!(out.normalized["title"], vec!["File title"]);
        assert_eq!(
            out.records
                .iter()
                .filter(|r| r.native_id == "X-REPEAT")
                .count(),
            2
        );
        if file.ends_with("flac") {
            assert!(out.records.iter().any(|r| r.native_id == "PICTURE"));
        }
    }
}
#[test]
fn mp4_retains_unknown_repeated_atoms_and_cover_bytes() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("song.m4a");
    let mut text = vec![0, 0, 0, 1, 0, 0, 0, 0];
    text.extend(b"File title");
    let mut items = mp4_atom(&[0xa9, b'n', b'a', b'm'], &mp4_atom(b"data", &text));
    items.extend(mp4_atom(b"zzzz", b"first"));
    items.extend(mp4_atom(b"zzzz", b"second"));
    items.extend(mp4_atom(b"covr", b"opaque cover bytes"));
    let mut meta = vec![0; 4];
    meta.extend(mp4_atom(b"ilst", &items));
    let mut b = mp4_atom(b"ftyp", b"M4A \0\0\0\0");
    b.extend(mp4_atom(
        b"moov",
        &mp4_atom(b"udta", &mp4_atom(b"meta", &meta)),
    ));
    fs::write(&p, b).unwrap();
    let out = read_file_tags(&p, &ImportLimits::default(), &CancellationToken::default()).unwrap();
    assert_eq!(out.normalized["title"], vec!["File title"]);
    assert_eq!(
        out.records.iter().filter(|r| r.native_id == "zzzz").count(),
        2
    );
    let cover = out.records.iter().find(|r| r.native_id == "covr").unwrap();
    assert_eq!(out.blobs[&cover.payload.sha256], b"opaque cover bytes");
}
#[test]
fn wav_and_aiff_preserve_repeated_native_chunks() {
    let d = tempfile::tempdir().unwrap();
    let mut info = b"INFO".to_vec();
    info.extend(chunk(b"INAM", b"File title\0", false));
    info.extend(chunk(b"IXXX", b"one", false));
    info.extend(chunk(b"IXXX", b"two", false));
    let mut wave = b"WAVE".to_vec();
    wave.extend(chunk(b"LIST", &info, false));
    let mut wav = b"RIFF".to_vec();
    wav.extend((wave.len() as u32).to_le_bytes());
    wav.extend(wave);
    fs::write(d.path().join("song.wav"), wav).unwrap();
    let mut form = b"AIFF".to_vec();
    form.extend(chunk(b"NAME", b"File title", true));
    form.extend(chunk(b"ANNO", b"one", true));
    form.extend(chunk(b"ANNO", b"two", true));
    let mut aiff = b"FORM".to_vec();
    aiff.extend((form.len() as u32).to_be_bytes());
    aiff.extend(form);
    fs::write(d.path().join("song.aiff"), aiff).unwrap();
    for (file, key) in [("song.wav", "IXXX"), ("song.aiff", "ANNO")] {
        let out = read_file_tags(
            &d.path().join(file),
            &ImportLimits::default(),
            &CancellationToken::default(),
        )
        .unwrap();
        assert_eq!(out.normalized["title"], vec!["File title"]);
        assert_eq!(out.records.iter().filter(|r| r.native_id == key).count(), 2);
    }
}
#[test]
fn asf_preserves_typed_attribute_identity_and_multiplicity() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("song.wma");
    let mut attrs = 3u16.to_le_bytes().to_vec();
    for (name, value) in [
        ("Title", "File title"),
        ("X-Repeat", "one"),
        ("X-Repeat", "two"),
    ] {
        let name: Vec<u8> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let value: Vec<u8> = value
            .encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect();
        attrs.extend((name.len() as u16).to_le_bytes());
        attrs.extend(name);
        attrs.extend(0u16.to_le_bytes());
        attrs.extend((value.len() as u16).to_le_bytes());
        attrs.extend(value);
    }
    let mut obj = vec![
        0x40, 0xa4, 0xd0, 0xd2, 0x07, 0xe3, 0xd2, 0x11, 0x97, 0xf0, 0, 0xa0, 0xc9, 0x5e, 0xa8, 0x50,
    ];
    obj.extend(((attrs.len() + 24) as u64).to_le_bytes());
    obj.extend(attrs);
    let mut b = vec![
        0x30, 0x26, 0xb2, 0x75, 0x8e, 0x66, 0xcf, 0x11, 0xa6, 0xd9, 0, 0xaa, 0, 0x62, 0xce, 0x6c,
    ];
    b.extend(((obj.len() + 30) as u64).to_le_bytes());
    b.extend(1u32.to_le_bytes());
    b.extend([1, 2]);
    b.extend(obj);
    fs::write(&p, b).unwrap();
    let out = read_file_tags(&p, &ImportLimits::default(), &CancellationToken::default()).unwrap();
    assert_eq!(out.normalized["title"], vec!["File title"]);
    assert_eq!(
        out.records
            .iter()
            .filter(|r| r.native_id == "X-Repeat")
            .count(),
        2
    );
}

#[test]
fn identical_binary_payloads_share_storage_within_the_capture_budget() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("tag.mp3");
    let payload = vec![7; 256];
    let mut body = frame(b"XAAA", &payload);
    body.extend(frame(b"XBBB", &payload));
    let n = body.len() as u32;
    let mut bytes = b"ID3\x03\0\0".to_vec();
    bytes.extend([
        ((n >> 21) & 127) as u8,
        ((n >> 14) & 127) as u8,
        ((n >> 7) & 127) as u8,
        (n & 127) as u8,
    ]);
    bytes.extend(body);
    fs::write(&p, bytes).unwrap();
    let limits = ImportLimits {
        blob_bytes: 800,
        ..Default::default()
    };
    let out = read_file_tags(&p, &limits, &CancellationToken::default()).unwrap();
    assert_eq!(out.blobs.len(), 2, "envelope plus one deduplicated payload");
    assert_eq!(out.records[1].payload.sha256, out.records[2].payload.sha256);
}

#[test]
fn tag_record_counts_are_bounded_even_for_empty_binary_frames() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("tag.mp3");
    let mut body = Vec::new();
    for _ in 0..65537 {
        body.extend(frame(b"XAAA", b""));
    }
    let n = body.len() as u32;
    let mut bytes = b"ID3\x03\0\0".to_vec();
    bytes.extend([
        ((n >> 21) & 127) as u8,
        ((n >> 14) & 127) as u8,
        ((n >> 7) & 127) as u8,
        (n & 127) as u8,
    ]);
    bytes.extend(body);
    fs::write(&p, bytes).unwrap();
    let result = read_file_tags(&p, &ImportLimits::default(), &CancellationToken::default());
    assert!(
        result.is_err(),
        "many empty frames must not allocate an unbounded record inventory"
    );
}

#[test]
fn tagless_mpeg_and_adts_and_legacy_footer_tags_are_read_without_decoding() {
    let d = tempfile::tempdir().unwrap();
    let media = include_bytes!("../src/mixxx/fixtures/no_tag.mp3");
    let p = d.path().join("plain.mp3");
    fs::write(&p, media).unwrap();
    let empty =
        read_file_tags(&p, &ImportLimits::default(), &CancellationToken::default()).unwrap();
    assert!(empty.records.is_empty());
    let mut legacy = media.to_vec();
    let mut tag = [0u8; 128];
    tag[..3].copy_from_slice(b"TAG");
    tag[3..15].copy_from_slice(b"Legacy title");
    legacy.extend(tag);
    fs::write(&p, &legacy).unwrap();
    let out = read_file_tags(&p, &ImportLimits::default(), &CancellationToken::default()).unwrap();
    assert_eq!(out.normalized["title"], vec!["Legacy title"]);
    assert!(out.records.iter().any(|r| r.namespace == "id3v1"));
    let p = d.path().join("plain.aac");
    fs::write(&p, [0xff, 0xf1, 0x50, 0x80, 0, 0xff, 0xfc]).unwrap();
    assert!(
        read_file_tags(&p, &ImportLimits::default(), &CancellationToken::default())
            .unwrap()
            .records
            .is_empty()
    );
}
#[test]
fn id3v24_footer_bytes_and_ape_custom_items_are_retained() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("footer.aac");
    let header = b"ID3\x04\0\x10\0\0\0\0";
    let footer = b"3DI\x04\0\x10\0\0\0\0";
    let mut bytes = header.to_vec();
    bytes.extend(footer);
    fs::write(&p, &bytes).unwrap();
    let out = read_file_tags(&p, &ImportLimits::default(), &CancellationToken::default()).unwrap();
    assert_eq!(out.blobs[&out.records[0].payload.sha256], bytes);
    let p = d.path().join("ape.mp3");
    let mut bytes = include_bytes!("../src/mixxx/fixtures/no_tag.mp3").to_vec();
    let mut items = Vec::new();
    for value in [b"one", b"two"] {
        items.extend(3u32.to_le_bytes());
        items.extend(0u32.to_le_bytes());
        items.extend(b"X-Repeat\0");
        items.extend(value);
    }
    let mut footer = b"APETAGEX".to_vec();
    footer.extend(2000u32.to_le_bytes());
    footer.extend(((items.len() + 32) as u32).to_le_bytes());
    footer.extend(2u32.to_le_bytes());
    footer.extend(0u32.to_le_bytes());
    footer.extend([0; 8]);
    bytes.extend(items);
    bytes.extend(footer);
    fs::write(&p, bytes).unwrap();
    let out = read_file_tags(&p, &ImportLimits::default(), &CancellationToken::default()).unwrap();
    assert_eq!(
        out.records
            .iter()
            .filter(|r| r.native_id == "X-Repeat")
            .count(),
        2
    );
}

#[test]
fn flac_mp4_and_asf_artwork_keep_mime_type_description_and_original_image_bytes() {
    let d = tempfile::tempdir().unwrap();
    let image = b"\x89PNG\r\n\x1a\n";
    let mut picture = 3u32.to_be_bytes().to_vec();
    picture.extend(9u32.to_be_bytes());
    picture.extend(b"image/png");
    picture.extend(5u32.to_be_bytes());
    picture.extend(b"cover");
    for value in [1u32, 1, 8, 0, image.len() as u32] {
        picture.extend(value.to_be_bytes());
    }
    picture.extend(image);
    {
        use base64::Engine;
        let comment = format!(
            "METADATA_BLOCK_PICTURE={}",
            base64::engine::general_purpose::STANDARD.encode(&picture)
        );
        let mut tags = b"OpusTags".to_vec();
        tags.extend(0u32.to_le_bytes());
        tags.extend(1u32.to_le_bytes());
        tags.extend((comment.len() as u32).to_le_bytes());
        tags.extend(comment.as_bytes());
        assert!(tags.len() < 255);
        let mut ogg = b"OggS".to_vec();
        ogg.extend([0; 22]);
        ogg.push(1);
        ogg.push(tags.len() as u8);
        ogg.extend(tags);
        fs::write(d.path().join("cover.opus"), ogg).unwrap();
    }
    let n = picture.len();
    let mut flac = b"fLaC".to_vec();
    flac.extend([
        0x86,
        ((n >> 16) & 255) as u8,
        ((n >> 8) & 255) as u8,
        (n & 255) as u8,
    ]);
    flac.extend(&picture);
    fs::write(d.path().join("cover.flac"), flac).unwrap();
    let mut payload = 14u32.to_be_bytes().to_vec();
    payload.extend([0; 4]);
    payload.extend(image);
    let cover = mp4_atom(b"covr", &mp4_atom(b"data", &payload));
    let mut meta = vec![0; 4];
    meta.extend(mp4_atom(b"ilst", &cover));
    let mut mp4 = mp4_atom(b"ftyp", b"M4A \0\0\0\0");
    mp4.extend(mp4_atom(b"moov", &mp4_atom(b"meta", &meta)));
    fs::write(d.path().join("cover.m4a"), mp4).unwrap();
    let mut asf_picture = vec![3];
    asf_picture.extend((image.len() as u32).to_le_bytes());
    for s in ["image/png", "cover"] {
        asf_picture.extend(s.encode_utf16().chain([0]).flat_map(u16::to_le_bytes));
    }
    asf_picture.extend(image);
    let key: Vec<u8> = "WM/Picture"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let mut attrs = 1u16.to_le_bytes().to_vec();
    attrs.extend((key.len() as u16).to_le_bytes());
    attrs.extend(key);
    attrs.extend(1u16.to_le_bytes());
    attrs.extend((asf_picture.len() as u16).to_le_bytes());
    attrs.extend(asf_picture);
    let mut obj = vec![
        0x40, 0xa4, 0xd0, 0xd2, 0x07, 0xe3, 0xd2, 0x11, 0x97, 0xf0, 0, 0xa0, 0xc9, 0x5e, 0xa8, 0x50,
    ];
    obj.extend(((attrs.len() + 24) as u64).to_le_bytes());
    obj.extend(attrs);
    let mut asf = vec![
        0x30, 0x26, 0xb2, 0x75, 0x8e, 0x66, 0xcf, 0x11, 0xa6, 0xd9, 0, 0xaa, 0, 0x62, 0xce, 0x6c,
    ];
    asf.extend(((obj.len() + 30) as u64).to_le_bytes());
    asf.extend(1u32.to_le_bytes());
    asf.extend([1, 2]);
    asf.extend(obj);
    fs::write(d.path().join("cover.wma"), asf).unwrap();
    for file in ["cover.flac", "cover.m4a", "cover.wma", "cover.opus"] {
        let out = read_file_tags(
            &d.path().join(file),
            &ImportLimits::default(),
            &CancellationToken::default(),
        )
        .unwrap();
        assert_eq!(out.artwork.len(), 1, "{file}");
        let art = &out.artwork[0];
        assert_eq!(art.mime_type.as_deref(), Some("image/png"));
        assert_eq!(out.blobs[&art.payload.sha256], image);
        if file != "cover.m4a" {
            assert_eq!(art.picture_type, Some(3));
            assert_eq!(art.description.as_deref(), Some("cover"));
        }
    }
}
