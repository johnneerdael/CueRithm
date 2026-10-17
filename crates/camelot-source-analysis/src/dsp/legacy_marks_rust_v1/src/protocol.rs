//! Own numeric transport; this binary protocol is not an original application API.
use legacy_marks_probe::{MAX_MARKS, Mark, OnsetMarks};
use std::io::Read;
const MAX_QUERIES: usize = 4096;
const MAX_SEARCH_WORK: usize = 20_000_000;
pub const MAX_INPUT_BYTES: usize = 24 + 16 * MAX_MARKS + 8 * MAX_QUERIES;
pub fn read_complete<R: Read>(reader: R, limit: usize) -> Result<Vec<u8>, String> {
    let probe = limit.checked_add(1).ok_or("input limit overflow")?;
    let mut bytes = Vec::new();
    reader
        .take(probe as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("complete numeric input exceeds byte limit".into());
    }
    Ok(bytes)
}
fn word(data: &[u8], offset: usize) -> u32 {
    // execute validates the complete fixed input geometry before any word/record slice.
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}
pub fn execute(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < 24 || &data[..4] != b"LGM1" {
        return Err("LGM1 numeric header required".into());
    }
    let capacity = word(data, 4) as usize;
    let count = word(data, 8) as usize;
    let queries = word(data, 12) as usize;
    let mode = word(data, 16);
    let threshold = f32::from_bits(word(data, 20));
    if capacity > MAX_MARKS
        || count > MAX_MARKS
        || queries > MAX_QUERIES
        || mode > 1
        || !threshold.is_finite()
    {
        return Err("numeric header outside bounded contract".into());
    }
    if count
        .checked_mul(queries)
        .is_none_or(|work| work > MAX_SEARCH_WORK)
    {
        return Err("bounded search work exceeds 20,000,000 visits".into());
    }
    let query_offset = 24_usize
        .checked_add(count.checked_mul(16).ok_or("record length overflow")?)
        .ok_or("record offset overflow")?;
    let expected = query_offset
        .checked_add(queries.checked_mul(8).ok_or("query length overflow")?)
        .ok_or("input length overflow")?;
    if data.len() != expected {
        return Err("truncated or trailing numeric input".into());
    }
    let mut marks = OnsetMarks::new();
    marks
        .generate_initial_buffers(capacity)
        .map_err(|e| e.to_string())?;
    for i in 0..count {
        let offset = 24 + i * 16;
        marks
            .add(Mark::from_le_bytes(
                data[offset..offset + 16].try_into().unwrap(),
            ))
            .map_err(|e| e.to_string())?;
    }
    if mode == 1 {
        marks
            .set_transition_threshold(threshold)
            .map_err(|e| e.to_string())?;
    }
    let mut results = Vec::new();
    results
        .try_reserve_exact(queries)
        .map_err(|e| e.to_string())?;
    for i in 0..queries {
        let offset = query_offset + i * 8;
        results.push(
            marks
                .base()
                .get_closest(word(data, offset) as i32, word(data, offset + 4) as i32)
                .map_err(|e| e.to_string())?,
        );
    }
    let out_length = 20 + count * 28 + marks.count() * 4 + queries * 4;
    let mut output = Vec::new();
    output
        .try_reserve_exact(out_length)
        .map_err(|e| e.to_string())?;
    output.extend_from_slice(b"LGO1");
    for n in [
        marks.base().count(),
        marks.base().capacity(),
        marks.count(),
        queries,
    ] {
        output.extend_from_slice(&(n as u32).to_le_bytes());
    }
    for &mark in marks.base().records() {
        output.extend_from_slice(&mark.to_le_bytes());
    }
    for i in 0..marks.count() {
        output.extend_from_slice(&marks.get_mark(i as i32).to_le_bytes());
    }
    for value in results {
        output.extend_from_slice(&value.to_le_bytes());
    }
    for &mark in marks.base().records() {
        output.extend_from_slice(&mark.export_le_bytes());
    }
    Ok(output)
}
#[cfg(test)]
mod tests {
    use super::read_complete;
    #[test]
    fn complete_reader_accepts_empty_and_exact_limit() {
        assert_eq!(read_complete(b"".as_slice(), 0).unwrap(), b"");
        assert_eq!(read_complete(b"LGM1".as_slice(), 4).unwrap(), b"LGM1");
    }
    #[test]
    fn complete_reader_rejects_extra_byte_and_overflow() {
        assert!(read_complete(b"LGM1X".as_slice(), 4).is_err());
        assert!(read_complete(b"".as_slice(), usize::MAX).is_err());
    }
}
