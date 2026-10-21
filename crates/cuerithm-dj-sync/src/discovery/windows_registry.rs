//! Read only the native current user's selected VirtualDJ HomeFolder value.
use std::io;

const MAX_HOME_BYTES: u32 = 64 * 1024;

fn read_selected_string(
    mut query: impl FnMut(Option<&mut [u16]>, &mut u32) -> io::Result<()>,
) -> Option<String> {
    let mut bytes = 0;
    query(None, &mut bytes).ok()?;
    if !(2..=MAX_HOME_BYTES).contains(&bytes) || bytes % 2 != 0 {
        return None;
    }
    let capacity_bytes = bytes;
    let mut buffer = vec![0; (bytes / 2) as usize];
    // A value that grows between the size and data calls falls back to normal
    // discovery. Do not retry or allocate beyond the bounded first measurement.
    query(Some(&mut buffer), &mut bytes).ok()?;
    if !(2..=capacity_bytes).contains(&bytes) || bytes % 2 != 0 {
        return None;
    }
    let value = &buffer[..(bytes / 2) as usize];
    if value.last() != Some(&0) || value[..value.len() - 1].contains(&0) {
        return None;
    }
    let value = String::from_utf16(&value[..value.len() - 1]).ok()?;
    (!value.trim().is_empty()).then_some(value)
}

#[cfg(windows)]
pub(super) fn virtualdj_home() -> Option<std::path::PathBuf> {
    use windows_sys::Win32::{
        Foundation::ERROR_SUCCESS,
        System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ, RegGetValueW},
    };
    let key: Vec<_> = "Software\\VirtualDJ".encode_utf16().chain([0]).collect();
    let value: Vec<_> = "HomeFolder".encode_utf16().chain([0]).collect();
    read_selected_string(|buffer, bytes| {
        let data = buffer.map_or(std::ptr::null_mut(), |buffer| buffer.as_mut_ptr().cast());
        // RegGetValueW uses byte sizes, validates string termination and expands
        // REG_EXPAND_SZ by default. Query no other value or registry location.
        // https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-reggetvaluew
        // SAFETY: key/value are NUL-terminated; data is null for the size probe
        // or points to a u16 buffer of the byte capacity supplied through bytes.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ,
                std::ptr::null_mut(),
                data,
                bytes,
            )
        };
        if status == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(io::Error::from_raw_os_error(status as i32))
        }
    })
    .map(std::path::PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_value(units: &[u16], returned_bytes: Option<u32>) -> Option<String> {
        read_selected_string(|buffer, bytes| {
            if let Some(buffer) = buffer {
                buffer[..units.len()].copy_from_slice(units);
                *bytes = returned_bytes.unwrap_or((units.len() * 2) as u32);
            } else {
                *bytes = (units.len() * 2) as u32;
            }
            Ok(())
        })
    }

    #[test]
    fn registry_reads_unicode_with_exactly_two_bounded_wide_calls() {
        let text = r"D:\DJ 音楽\VirtualDJ";
        let units: Vec<_> = text.encode_utf16().chain([0]).collect();
        let mut calls = 0;
        let result = read_selected_string(|buffer, bytes| {
            calls += 1;
            if let Some(buffer) = buffer {
                assert_eq!(buffer.len(), units.len());
                buffer.copy_from_slice(&units);
            }
            *bytes = (units.len() * 2) as u32;
            Ok(())
        });
        assert_eq!(result.as_deref(), Some(text));
        assert_eq!(calls, 2);
    }

    #[test]
    fn registry_preserves_verbatim_path_with_trailing_spaces() {
        let path = r"\\?\C:\DJ Library\VirtualDJ ";
        let units: Vec<_> = path.encode_utf16().chain([0]).collect();
        assert_eq!(read_value(&units, None).as_deref(), Some(path));
    }

    #[test]
    fn registry_missing_errors_and_invalid_sizes_do_not_allocate_or_retry() {
        for size in [0, 1, MAX_HOME_BYTES + 2] {
            let mut calls = 0;
            assert!(
                read_selected_string(|_, bytes| {
                    calls += 1;
                    *bytes = size;
                    Ok(())
                })
                .is_none()
            );
            assert_eq!(calls, 1);
        }
        for kind in [io::ErrorKind::NotFound, io::ErrorKind::PermissionDenied] {
            let mut calls = 0;
            assert!(
                read_selected_string(|_, _| {
                    calls += 1;
                    Err(kind.into())
                })
                .is_none()
            );
            assert_eq!(calls, 1);
        }
        let mut calls = 0;
        assert!(
            read_selected_string(|buffer, bytes| {
                calls += 1;
                if buffer.is_some() {
                    Err(io::ErrorKind::InvalidData.into())
                } else {
                    *bytes = 4;
                    Ok(())
                }
            })
            .is_none()
        );
        assert_eq!(calls, 2);
    }

    #[test]
    fn registry_rejects_truncated_changed_or_ambiguous_strings() {
        for units in [
            &[65_u16, 66][..],
            &[65, 0, 66, 0],
            &[0xd800, 0],
            &[0],
            &[32, 0],
        ] {
            assert!(read_value(units, None).is_none());
        }
        assert!(read_value(&[65, 0], Some(6)).is_none());
        assert!(read_value(&[65, 0], Some(3)).is_none());
        assert!(read_value(&[65, 0], Some(0)).is_none());
    }
}
