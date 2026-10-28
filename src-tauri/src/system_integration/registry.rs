//! The current user's part of the registry, as tTerm reads and changes it.

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_VALUE_TYPE,
};

/// A key under `HKEY_CURRENT_USER`, closed when dropped.
pub struct Key {
    handle: HKEY,
    path: String,
}

impl Key {
    /// Opens `path` to read its values; `None` when it does not exist.
    pub fn open(path: &str) -> Result<Option<Self>, String> {
        let mut handle = HKEY::default();
        // SAFETY: `handle` outlives the call, which writes the opened key to it.
        let status = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                &HSTRING::from(path),
                None,
                KEY_QUERY_VALUE,
                &mut handle,
            )
        };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        status
            .ok()
            .map_err(|error| format!("Failed to open the registry key {path}: {error}"))?;
        Ok(Some(Self::new(handle, path)))
    }

    /// Opens `path` to change its values, creating it and its parents first
    /// when they do not exist.
    pub fn create(path: &str) -> Result<Self, String> {
        let mut handle = HKEY::default();
        // SAFETY: `handle` outlives the call, which writes the opened key to it.
        unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                &HSTRING::from(path),
                None,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_QUERY_VALUE | KEY_SET_VALUE,
                None,
                &mut handle,
                None,
            )
        }
        .ok()
        .map_err(|error| format!("Failed to create the registry key {path}: {error}"))?;
        Ok(Self::new(handle, path))
    }

    fn new(handle: HKEY, path: &str) -> Self {
        Self {
            handle,
            path: path.to_string(),
        }
    }

    /// The string value `name`, or the key's default value for `None`, with
    /// its registry type; `None` when the key has no such value.
    pub fn read_string(
        &self,
        name: Option<&str>,
    ) -> Result<Option<(String, REG_VALUE_TYPE)>, String> {
        let name_text = name.map(HSTRING::from);
        let name_ptr = value_name(name_text.as_ref());
        let failed = |error: windows::core::Error| {
            format!(
                "Failed to read {} in the registry: {error}",
                self.describe(name)
            )
        };
        let mut kind = REG_VALUE_TYPE::default();
        let mut size = 0u32;
        // SAFETY: `kind` and `size` outlive the call, which asks only for the size.
        let status = unsafe {
            RegQueryValueExW(
                self.handle,
                name_ptr,
                None,
                Some(&mut kind),
                None,
                Some(&mut size),
            )
        };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        status.ok().map_err(failed)?;
        let mut data = vec![0u16; (size as usize).div_ceil(2)];
        // SAFETY: `data` holds at least `size` bytes, and the call writes at most `size`.
        unsafe {
            RegQueryValueExW(
                self.handle,
                name_ptr,
                None,
                Some(&mut kind),
                Some(data.as_mut_ptr().cast()),
                Some(&mut size),
            )
        }
        .ok()
        .map_err(failed)?;
        data.truncate((size as usize) / 2);
        while data.last() == Some(&0) {
            data.pop();
        }
        // Writing back a lossy conversion would damage the value.
        let value = String::from_utf16(&data).map_err(|_| {
            format!(
                "{} holds text tTerm cannot read, so it was left alone.",
                self.describe(name)
            )
        })?;
        Ok(Some((value, kind)))
    }

    /// Sets the string value `name`, or the key's default value for `None`.
    pub fn write_string(
        &self,
        name: Option<&str>,
        value: &str,
        kind: REG_VALUE_TYPE,
    ) -> Result<(), String> {
        let name_text = name.map(HSTRING::from);
        let bytes: Vec<u8> = value
            .encode_utf16()
            .chain(std::iter::once(0))
            .flat_map(u16::to_le_bytes)
            .collect();
        // SAFETY: the key is open for writing, and the call only reads `bytes`.
        unsafe {
            RegSetValueExW(
                self.handle,
                value_name(name_text.as_ref()),
                None,
                kind,
                Some(&bytes),
            )
        }
        .ok()
        .map_err(|error| format!("Failed to change {}: {error}", self.describe(name)))
    }

    fn describe(&self, name: Option<&str>) -> String {
        match name {
            Some(name) => format!("{name} in {}", self.path),
            None => self.path.clone(),
        }
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: the handle came from opening or creating the key and is
        // closed only here.
        let _ = unsafe { RegCloseKey(self.handle) };
    }
}

/// A value's name for the registry functions, where null means the default value.
fn value_name(name: Option<&HSTRING>) -> PCWSTR {
    name.map_or(PCWSTR::null(), |name| PCWSTR(name.as_ptr()))
}
