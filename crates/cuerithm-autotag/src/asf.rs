//! Safe ownership wrapper around the source-bundled ASF metadata bridge.
use anyhow::{Result, bail};
use onetagger_tag::{CoverType, Field, Lyrics, Picture, TagDate, TagImpl};
use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::{CStr, c_char, c_void},
    path::Path,
    ptr::NonNull,
};
unsafe extern "C" {
    fn cw_asf_open(path: *const c_void, len: usize) -> *mut c_void;
    fn cw_asf_free(handle: *mut c_void);
    fn cw_asf_error(handle: *mut c_void) -> *const c_char;
    fn cw_asf_get(
        handle: *mut c_void,
        key: *const u8,
        kl: usize,
        index: usize,
        out: *mut *const u8,
        len: *mut usize,
    ) -> i32;
    fn cw_asf_keys(handle: *mut c_void, index: usize, out: *mut *const u8, len: *mut usize) -> i32;
    fn cw_asf_clear(handle: *mut c_void, key: *const u8, kl: usize) -> i32;
    fn cw_asf_add(
        handle: *mut c_void,
        key: *const u8,
        kl: usize,
        value: *const u8,
        vl: usize,
    ) -> i32;
    fn cw_asf_picture(
        handle: *mut c_void,
        index: usize,
        kind: *mut u32,
        data: *mut *const u8,
        len: *mut usize,
        mime: *mut *const u8,
        ml: *mut usize,
        description: *mut *const u8,
        dl: *mut usize,
    ) -> i32;
    fn cw_asf_remove_picture(handle: *mut c_void, kind: u32) -> i32;
    fn cw_asf_set_picture(
        handle: *mut c_void,
        kind: u32,
        mime: *const u8,
        ml: usize,
        description: *const u8,
        dl: usize,
        data: *const u8,
        len: usize,
    ) -> i32;
    fn cw_asf_edit_picture(
        handle: *mut c_void,
        selected: i64,
        kind: u32,
        mime: *const u8,
        ml: usize,
        description: *const u8,
        dl: usize,
        data: *const u8,
        len: usize,
    ) -> i32;
    fn cw_asf_save(handle: *mut c_void) -> i32;
}
pub(crate) struct AsfTag {
    handle: NonNull<c_void>,
    error: RefCell<Option<String>>,
    separator: String,
}
impl Drop for AsfTag {
    fn drop(&mut self) {
        unsafe {
            cw_asf_free(self.handle.as_ptr());
        }
    }
}
impl AsfTag {
    pub(crate) fn editor_picture(
        &mut self,
        selected: Option<usize>,
        image: Option<&crate::editing::ArtworkValue>,
    ) -> Result<()> {
        let (kind, mime, description, bytes) = if let Some(image) = image {
            let validated = crate::image::Artwork::validate(image.bytes.clone())?;
            (
                image.kind.unwrap_or(3),
                validated.mime.to_string(),
                image.description.clone().unwrap_or_default(),
                Some(validated.bytes),
            )
        } else {
            (0, String::new(), String::new(), None)
        };
        let (ptr, len) = bytes
            .as_ref()
            .map(|b| (b.as_ptr(), b.len()))
            .unwrap_or((std::ptr::null(), 0));
        if unsafe {
            cw_asf_edit_picture(
                self.handle.as_ptr(),
                selected.map(|i| i as i64).unwrap_or(-1),
                kind,
                mime.as_ptr(),
                mime.len(),
                description.as_ptr(),
                description.len(),
                ptr,
                len,
            )
        } == 0
        {
            self.remember();
            anyhow::bail!("Edit selected ASF artwork failed");
        }
        Ok(())
    }

    pub fn open(path: &Path) -> Result<Self> {
        #[cfg(unix)]
        let raw = {
            use std::os::unix::ffi::OsStrExt;
            let bytes = path.as_os_str().as_bytes();
            unsafe { cw_asf_open(bytes.as_ptr().cast(), bytes.len()) }
        };
        #[cfg(windows)]
        let raw = {
            use std::os::windows::ffi::OsStrExt;
            let units: Vec<u16> = path.as_os_str().encode_wide().collect();
            unsafe { cw_asf_open(units.as_ptr().cast(), units.len()) }
        };
        let Some(handle) = NonNull::new(raw) else {
            let message =
                unsafe { CStr::from_ptr(cw_asf_error(std::ptr::null_mut())) }.to_string_lossy();
            bail!("Open staged WMA: {message}");
        };
        Ok(Self {
            handle,
            error: RefCell::new(None),
            separator: "; ".into(),
        })
    }
    fn remember(&self) {
        let text = unsafe { CStr::from_ptr(cw_asf_error(self.handle.as_ptr())) }
            .to_string_lossy()
            .into_owned();
        *self.error.borrow_mut() = Some(text);
    }
    fn values(&self, key: &str) -> Option<Vec<String>> {
        let mut values = vec![];
        for i in 0..4096 {
            let mut bytes = std::ptr::null();
            let mut len = 0;
            let result = unsafe {
                cw_asf_get(
                    self.handle.as_ptr(),
                    key.as_ptr(),
                    key.len(),
                    i,
                    &mut bytes,
                    &mut len,
                )
            };
            if result < 0 {
                self.remember();
                return None;
            }
            if result == 0 {
                break;
            }
            if len > 1_048_576 {
                *self.error.borrow_mut() = Some("Metadata value exceeds limit".into());
                return None;
            }
            let value = unsafe { std::slice::from_raw_parts(bytes, len) };
            match String::from_utf8(value.to_vec()) {
                Ok(value) => values.push(value),
                Err(e) => {
                    *self.error.borrow_mut() = Some(e.to_string());
                    return None;
                }
            }
        }
        (!values.is_empty()).then_some(values)
    }
    fn date(&self, key: &str) -> Option<TagDate> {
        let values = self.values(key)?;
        let text = values.first()?;
        let mut p = text.split('-');
        let year = p.next()?.parse().ok()?;
        Some(TagDate {
            year,
            month: p.next().and_then(|s| s.parse().ok()),
            day: p.next().and_then(|s| s.parse().ok()),
        })
    }
    fn set_date_key(&mut self, key: &str, date: &TagDate, overwrite: bool) {
        let value = match (date.month, date.day) {
            (Some(m), Some(d)) => format!("{:04}-{m:02}-{d:02}", date.year),
            _ => date.year.to_string(),
        };
        self.set_raw(key, vec![value], overwrite);
    }
}
pub(crate) fn kind(kind: &CoverType) -> u32 {
    match kind {
        CoverType::Other => 0,
        CoverType::Icon => 1,
        CoverType::OtherIcon => 2,
        CoverType::CoverFront => 3,
        CoverType::CoverBack => 4,
        CoverType::Leaflet => 5,
        CoverType::Media => 6,
        CoverType::LeadArtist => 7,
        CoverType::Artist => 8,
        CoverType::Conductor => 9,
        CoverType::Band => 10,
        CoverType::Composer => 11,
        CoverType::Lyricist => 12,
        CoverType::RecordingLocation => 13,
        CoverType::DuringRecording => 14,
        CoverType::DuringPerformance => 15,
        CoverType::ScreenCapture => 16,
        CoverType::BrightFish => 17,
        CoverType::Illustration => 18,
        CoverType::BandLogo => 19,
        CoverType::PublisherLogo => 20,
        CoverType::Undefined => 255,
    }
}
fn cover(value: u32) -> CoverType {
    CoverType::types()
        .into_iter()
        .find(|c| kind(c) == value)
        .unwrap_or(CoverType::Undefined)
}
impl TagImpl for AsfTag {
    fn save_file(&mut self, _path: &Path) -> Result<()> {
        if let Some(error) = self.error.borrow().as_ref() {
            bail!("WMA metadata: {error}");
        }
        if unsafe { cw_asf_save(self.handle.as_ptr()) } == 0 {
            self.remember();
            bail!(
                "Save staged WMA metadata: {}",
                self.error.borrow().as_ref().unwrap()
            );
        }
        Ok(())
    }
    fn set_separator(&mut self, value: &str) {
        self.separator = value.into();
    }
    fn get_separator(&self) -> Option<String> {
        Some(self.separator.clone())
    }
    fn all_tags(&self) -> HashMap<String, Vec<String>> {
        let mut out = HashMap::new();
        for i in 0..4096 {
            let mut ptr = std::ptr::null();
            let mut len = 0;
            let result = unsafe { cw_asf_keys(self.handle.as_ptr(), i, &mut ptr, &mut len) };
            if result < 0 {
                self.remember();
                break;
            }
            if result == 0 {
                break;
            }
            let key = String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(ptr, len) })
                .into_owned();
            if let Some(values) = self.values(&key) {
                out.insert(key, values);
            }
        }
        out
    }
    fn get_date(&self) -> Option<TagDate> {
        self.date("RELEASETIME").or_else(|| self.date("WM/Year"))
    }
    fn set_date(&mut self, date: &TagDate, overwrite: bool) {
        if overwrite || self.get_date().is_none() {
            self.set_raw("WM/Year", vec![date.year.to_string()], true);
            if date.month.is_some() {
                self.set_date_key("RELEASETIME", date, true);
            } else {
                self.remove_raw("RELEASETIME");
            }
        }
    }
    fn set_publish_date(&mut self, date: &TagDate, overwrite: bool) {
        self.set_date_key("PUBLISHTIME", date, overwrite);
    }
    fn get_rating(&self) -> Option<u8> {
        let v: u16 = self.values("WM/SharedUserRating")?.first()?.parse().ok()?;
        Some(match v {
            0 => 0,
            1..=12 => 1,
            13..=37 => 2,
            38..=62 => 3,
            63..=87 => 4,
            _ => 5,
        })
    }
    fn set_rating(&mut self, rating: u8, overwrite: bool) {
        let value = [0, 1, 25, 50, 75, 99][usize::from(rating.min(5))];
        self.set_raw("WM/SharedUserRating", vec![value.to_string()], overwrite);
    }
    fn set_art(
        &mut self,
        kind_value: CoverType,
        mime: &str,
        description: Option<&str>,
        data: Vec<u8>,
    ) {
        let desc = description.unwrap_or("");
        if data.len() > 8 * 1024 * 1024 {
            *self.error.borrow_mut() = Some("Artwork exceeds limit".into());
            return;
        }
        if unsafe {
            cw_asf_set_picture(
                self.handle.as_ptr(),
                kind(&kind_value),
                mime.as_ptr(),
                mime.len(),
                desc.as_ptr(),
                desc.len(),
                data.as_ptr(),
                data.len(),
            )
        } == 0
        {
            self.remember();
        }
    }
    fn has_art(&self) -> bool {
        !self.get_art().is_empty()
    }
    fn get_art(&self) -> Vec<Picture> {
        let mut out = vec![];
        for i in 0..256 {
            let mut data = std::ptr::null();
            let mut mime = std::ptr::null();
            let mut len = 0;
            let mut ml = 0;
            let mut k = 0;
            let mut description = std::ptr::null();
            let mut dl = 0;
            let result = unsafe {
                cw_asf_picture(
                    self.handle.as_ptr(),
                    i,
                    &mut k,
                    &mut data,
                    &mut len,
                    &mut mime,
                    &mut ml,
                    &mut description,
                    &mut dl,
                )
            };
            if result < 0 {
                self.remember();
                break;
            }
            if result == 0 {
                break;
            }
            if result == 2 {
                continue;
            }
            if len > 8 * 1024 * 1024 {
                *self.error.borrow_mut() = Some("Artwork exceeds limit".into());
                break;
            }
            out.push(Picture {
                kind: cover(k),
                data: unsafe { std::slice::from_raw_parts(data, len) }.to_vec(),
                mime: String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(mime, ml) })
                    .into_owned(),
                description: String::from_utf8_lossy(unsafe {
                    std::slice::from_raw_parts(description, dl)
                })
                .into_owned(),
            });
        }
        out
    }
    fn remove_art(&mut self, value: CoverType) {
        if unsafe { cw_asf_remove_picture(self.handle.as_ptr(), kind(&value)) } == 0 {
            self.remember();
        }
    }
    fn set_field(&mut self, field: Field, values: Vec<String>, overwrite: bool) {
        if field == Field::Artist {
            if overwrite || (self.values("WM/Artist").is_none() && self.values("Author").is_none())
            {
                self.set_raw("WM/Artist", values.clone(), true);
                self.set_raw("Author", values, true);
            }
        } else {
            self.set_raw(field.asf(), values, overwrite);
        }
    }
    fn get_field(&self, field: Field) -> Option<Vec<String>> {
        if field == Field::Artist {
            self.values("WM/Artist").or_else(|| self.values("Author"))
        } else {
            self.get_raw(field.asf())
        }
    }
    fn set_raw(&mut self, key: &str, mut values: Vec<String>, overwrite: bool) {
        if !overwrite && self.values(key).is_some() {
            return;
        }
        if key == "Author" && values.len() > 1 {
            values = vec![values.join(&self.separator)];
        }
        self.remove_raw(key);
        for value in values {
            if unsafe {
                cw_asf_add(
                    self.handle.as_ptr(),
                    key.as_ptr(),
                    key.len(),
                    value.as_ptr(),
                    value.len(),
                )
            } == 0
            {
                self.remember();
                break;
            }
        }
    }
    fn get_raw(&self, key: &str) -> Option<Vec<String>> {
        self.values(key)
    }
    fn get_native(&self, key: &str) -> Option<Vec<String>> {
        self.values(key)
    }
    fn set_native(&mut self, key: &str, values: Vec<String>) {
        self.set_raw(key, values, true);
    }
    fn remove_raw(&mut self, key: &str) {
        if unsafe { cw_asf_clear(self.handle.as_ptr(), key.as_ptr(), key.len()) } == 0 {
            self.remember();
        }
    }
    fn set_lyrics(&mut self, _lyrics: &Lyrics, _synced: bool, _overwrite: bool) {
        *self.error.borrow_mut() = Some("Lyrics are outside automatic preparation scope".into());
    }
    fn set_track_number(&mut self, number: &str, total: Option<u16>, overwrite: bool) {
        self.set_field(Field::TrackNumber, vec![number.into()], overwrite);
        if let Some(total) = total {
            self.set_field(Field::TrackTotal, vec![total.to_string()], overwrite);
        }
    }
    fn set_explicit(&mut self, value: bool) {
        self.set_raw(
            "ITUNESADVISORY",
            vec![if value { "1" } else { "0" }.into()],
            true,
        );
    }
}
