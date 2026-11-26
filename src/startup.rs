//! HKCU Run. HKLM yok, yönetici yok.

const NAME: &str = "xscreen";

pub fn set(enabled: bool) -> std::io::Result<()> {
    unsafe {
        use std::os::windows::ffi::OsStrExt;
        use windows::{
            Win32::{
                Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
                System::Registry::{
                    HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegDeleteValueW,
                    RegOpenKeyExW, RegSetValueExW,
                },
            },
            core::{PCWSTR, w},
        };
        let mut key = Default::default();
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            None,
            KEY_SET_VALUE,
            &mut key,
        )
        .ok()
        .map_err(|e| std::io::Error::other(e.to_string()))?;
        let name: Vec<u16> = NAME.encode_utf16().chain(Some(0)).collect();
        let result = if enabled {
            let exe = std::env::current_exe()?;
            let wide: Vec<u16> = exe.as_os_str().encode_wide().chain(Some(0)).collect();
            RegSetValueExW(
                key,
                PCWSTR(name.as_ptr()),
                None,
                REG_SZ,
                Some(std::slice::from_raw_parts(
                    wide.as_ptr().cast(),
                    (wide.len() - 1) * 2,
                )),
            )
        } else {
            RegDeleteValueW(key, PCWSTR(name.as_ptr()))
        };
        let _ = RegCloseKey(key);
        if result == ERROR_SUCCESS || (!enabled && result == ERROR_FILE_NOT_FOUND) {
            Ok(())
        } else {
            Err(std::io::Error::other(result.to_hresult().message()))
        }
    }
}

pub fn is_set() -> bool {
    unsafe {
        use windows::{
            Win32::System::Registry::{
                HKEY_CURRENT_USER, KEY_QUERY_VALUE, RRF_RT_REG_SZ, RegCloseKey, RegGetValueW,
                RegOpenKeyExW,
            },
            core::w,
        };
        let mut key = Default::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            None,
            KEY_QUERY_VALUE,
            &mut key,
        )
        .is_err()
        {
            return false;
        }
        let name: Vec<u16> = NAME.encode_utf16().chain(Some(0)).collect();
        let mut kind = Default::default();
        let mut bytes = 0u32;
        let found = RegGetValueW(
            key,
            None,
            windows::core::PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ,
            Some(&mut kind),
            None,
            Some(&mut bytes),
        )
        .is_ok();
        let _ = RegCloseKey(key);
        found
    }
}
