//! Native cue palette projection. Schema recognition does not establish that a
//! particular application/profile accepts these observed protocol values.
use crate::{NativeCueColor, NativeCueSlot};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Observed native hot-cue RGB lookup, indexed by SQL ColorTableIndex minus one.
/// Semantic native color labels use their own named indices; an exact RGB label
/// therefore need not select the same index as a similarly named color.
pub const NATIVE_HOT_CUE_RGB: [u32; 64] = [
    0x0000ff, 0x001cff, 0x0038ff, 0x0054ff, 0x0070ff, 0x008cff, 0x00a8ff, 0x00c4ff, 0x00e0ff,
    0x00ffff, 0x00ffe8, 0x00ffd1, 0x00ffba, 0x00ffa3, 0x00ff8c, 0x00ff75, 0x00ff5e, 0x00ff47,
    0x00ff30, 0x00ff1a, 0x00ff00, 0x1aff00, 0x33ff00, 0x4dff00, 0x66ff00, 0x80ff00, 0x99ff00,
    0xb3ff00, 0xccff00, 0xe6ff00, 0xffff00, 0xffe800, 0xffd100, 0xffba00, 0xffa300, 0xff8c00,
    0xff7500, 0xff5e00, 0xff4700, 0xff3000, 0xff1a00, 0xff0000, 0xff0017, 0xff002e, 0xff0045,
    0xff005c, 0xff0073, 0xff008a, 0xff00a1, 0xff00b8, 0xff00cf, 0xff00e6, 0xff00ff, 0xe600ff,
    0xcc00ff, 0xb300ff, 0x9900ff, 0x8000ff, 0x6600ff, 0x4d00ff, 0x3300ff, 0x1a00ff, 0x000000,
    0xffffff,
];

/// The eight memory colors, in SQL Color order (ANLZ memory_color minus one).
pub const NATIVE_MEMORY_CUE_RGB: [u32; 8] = [
    0xff00ff, 0xff0000, 0xff8c00, 0xffff00, 0x00ff00, 0x00ffff, 0x0000ff, 0xb300ff,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeCueColorProjection {
    pub sql: NativeCueColor,
    /// The native ANLZ color word: reserved byte, red, green, blue.
    pub rgba: [u8; 4],
    pub memory_color: u8,
    /// True only when explicit permission enabled a lossy palette conversion.
    pub converted: bool,
}

fn native_word(rgb: u32) -> [u8; 4] {
    [0, (rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]
}
fn uncolored(is_loop: bool) -> NativeCueColorProjection {
    NativeCueColorProjection {
        sql: NativeCueColor {
            color: if is_loop { 255 } else { -1 },
            color_table_index: Some(0),
        },
        rgba: [0; 4],
        memory_color: 0,
        converted: false,
    }
}
fn hot(index: usize, converted: bool) -> NativeCueColorProjection {
    NativeCueColorProjection {
        sql: NativeCueColor {
            color: -1,
            color_table_index: Some((index + 1) as i64),
        },
        rgba: native_word(NATIVE_HOT_CUE_RGB[index]),
        memory_color: 0,
        converted,
    }
}
fn memory(index: usize, converted: bool) -> NativeCueColorProjection {
    NativeCueColorProjection {
        sql: NativeCueColor {
            color: index as i64,
            color_table_index: None,
        },
        rgba: native_word(NATIVE_MEMORY_CUE_RGB[index]),
        memory_color: (index + 1) as u8,
        converted,
    }
}
fn distance(a: u32, b: u32) -> u32 {
    [16, 8, 0]
        .into_iter()
        .map(|shift| {
            let delta = ((a >> shift) & 255) as i32 - ((b >> shift) & 255) as i32;
            (delta * delta) as u32
        })
        .sum()
}
fn nearest(rgb: u32, palette: &[u32]) -> usize {
    palette
        .iter()
        .enumerate()
        .min_by_key(|(index, color)| (distance(rgb, **color), *index))
        .expect("nonempty native palette")
        .0
}
fn named(label: &str) -> Option<(usize, Option<usize>)> {
    let (hot, memory) = match label {
        "blue" => (1, Some(6)),
        "bluelight" | "lightblue" => (6, None),
        "aqua" | "cyan" => (10, Some(5)),
        "aquadark" | "darkaqua" => (15, None),
        "teal" => (18, None),
        "green" => (21, Some(4)),
        "lime" => (25, None),
        "yellow" => (30, Some(3)),
        "yellowdark" | "darkyellow" => (33, None),
        "orange" => (36, Some(2)),
        "red" => (41, Some(1)),
        "magentared" | "redmagenta" => (46, None),
        "magenta" | "pink" => (52, Some(0)),
        "violet" | "purple" => (56, Some(7)),
        "violetlight" | "lightviolet" => (60, None),
        "blueviolet" => (62, None),
        "black" => (63, None),
        "white" => (64, None),
        _ => return None,
    };
    Some((hot - 1, memory))
}

/// Preserve known semantic labels or exact RGB values. Reject an unsupported
/// color unless `allow_nearest` explicitly permits a stable RGB-distance match.
/// A supplied semantic label takes precedence over an adapter's approximate RGB.
pub fn project_native_cue_color(
    label: &str,
    rgb: Option<u32>,
    slot: NativeCueSlot,
    is_loop: bool,
    allow_nearest: bool,
) -> Result<NativeCueColorProjection> {
    ensure!(
        !matches!(slot,NativeCueSlot::Hot(index) if index>=8),
        "Invalid native hot cue slot"
    );
    ensure!(
        rgb.is_none_or(|rgb| rgb <= 0xffffff),
        "Native cue RGB exceeds 24 bits"
    );
    ensure!(
        label.len() <= 64 && !label.chars().any(char::is_control),
        "Invalid native cue color label"
    );
    let label = label.trim();
    let normalized = label
        .chars()
        .filter(|c| !matches!(c, ' ' | '-' | '_'))
        .flat_map(char::to_lowercase)
        .collect::<String>();
    if matches!(
        normalized.as_str(),
        "none" | "uncolored" | "default" | "transparent"
    ) || (normalized.is_empty() && rgb.is_none())
    {
        return Ok(uncolored(is_loop));
    }
    if let Some((hot_index, memory_index)) = named(&normalized) {
        return match slot {
            NativeCueSlot::Hot(_) => Ok(hot(hot_index, false)),
            NativeCueSlot::Memory => {
                if let Some(index) = memory_index {
                    Ok(memory(index, false))
                } else {
                    ensure!(
                        allow_nearest,
                        "Named cue color is unsupported by native memory cues; explicitly enable nearest cue colors to convert it"
                    );
                    Ok(memory(
                        nearest(NATIVE_HOT_CUE_RGB[hot_index], &NATIVE_MEMORY_CUE_RGB),
                        true,
                    ))
                }
            }
        };
    }
    let rgb = if let Some(hex) = label.strip_prefix('#') {
        ensure!(
            hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "Native cue hex color must contain six RGB digits"
        );
        u32::from_str_radix(hex, 16).context("Invalid native cue hex color")?
    } else {
        rgb.context("Unknown native cue color label without RGB")?
    };
    let palette: &[u32] = match slot {
        NativeCueSlot::Memory => &NATIVE_MEMORY_CUE_RGB,
        NativeCueSlot::Hot(_) => &NATIVE_HOT_CUE_RGB,
    };
    let exact = palette.iter().position(|color| *color == rgb);
    ensure!(
        exact.is_some() || allow_nearest,
        "Cue RGB is outside the native palette; explicitly enable nearest cue colors to convert it"
    );
    let index = exact.unwrap_or_else(|| nearest(rgb, palette));
    Ok(match slot {
        NativeCueSlot::Memory => memory(index, exact.is_none()),
        NativeCueSlot::Hot(_) => hot(index, exact.is_none()),
    })
}
