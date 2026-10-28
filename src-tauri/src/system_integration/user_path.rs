//! The user's PATH in the registry, which every new process starts with.

use windows::core::w;
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, LPARAM, WPARAM};
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_QUERY_VALUE, KEY_SET_VALUE, REG_EXPAND_SZ, REG_SAM_FLAGS, REG_VALUE_TYPE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
};

/// How long one window may take to take in the new environment.
const BROADCAST_TIMEOUT_MS: u32 = 2000;

/// Whether the user's PATH lists `dir`.
pub fn contains(dir: &str) -> Result<bool, String> {
    Ok(read()?.is_some_and(|(value, _)| has_entry(&value, dir)))
}

/// Adds `dir` to the end of the user's PATH, or removes it. Shells started
/// from now on see the change, including the ones tTerm starts.
pub fn set(dir: &str, present: bool) -> Result<(), String> {
    let (value, kind) = read()?.unwrap_or((String::new(), REG_EXPAND_SZ));
    if has_entry(&value, dir) != present {
        write(&changed(&value, dir, present), kind)?;
        broadcast_change();
    }
    // tTerm's shells start with its own environment, read when it started.
    let current = std::env::var("PATH").unwrap_or_default();
    if has_entry(&current, dir) != present {
        std::env::set_var("PATH", changed(&current, dir, present));
    }
    Ok(())
}

fn changed(value: &str, dir: &str, present: bool) -> String {
    if present {
        with_entry(value, dir)
    } else {
        without_entry(value, dir)
    }
}

/// Whether `entry` names `dir`. Windows compares paths without regard to
/// case, and a PATH entry may be quoted or end in a separator.
fn same_dir(entry: &str, dir: &str) -> bool {
    let normal = |path: &str| {
        path.trim()
            .trim_matches('"')
            .trim_end_matches(['\\', '/'])
            .to_lowercase()
    };
    normal(entry) == normal(dir)
}

fn has_entry(value: &str, dir: &str) -> bool {
    value.split(';').any(|entry| same_dir(entry, dir))
}

fn with_entry(value: &str, dir: &str) -> String {
    let value = value.trim_end_matches(';');
    if value.is_empty() {
        dir.to_string()
    } else {
        format!("{value};{dir}")
    }
}

fn without_entry(value: &str, dir: &str) -> String {
    value
        .split(';')
        .filter(|entry| !same_dir(entry, dir))
        .collect::<Vec<_>>()
        .join(";")
}

/// The `Environment` key of the current user, closed when dropped.
struct EnvironmentKey(HKEY);

impl EnvironmentKey {
    fn open(access: REG_SAM_FLAGS) -> Result<Self, String> {
        let mut key = HKEY::default();
        // SAFETY: `key` outlives the call, which writes the opened handle to it.
        unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, w!("Environment"), None, access, &mut key) }
            .ok()
            .map_err(|error| format!("Failed to open your environment variables: {error}"))?;
        Ok(Self(key))
    }
}

impl Drop for EnvironmentKey {
    fn drop(&mut self) {
        // SAFETY: the handle came from `RegOpenKeyExW` and is closed only here.
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

/// The user's PATH and its registry type, or `None` when it has none.
fn read() -> Result<Option<(String, REG_VALUE_TYPE)>, String> {
    let key = EnvironmentKey::open(KEY_QUERY_VALUE)?;
    let failed = |error: windows::core::Error| format!("Failed to read your PATH: {error}");
    let mut kind = REG_VALUE_TYPE::default();
    let mut size = 0u32;
    // SAFETY: `kind` and `size` outlive the call, which asks only for the size.
    let status = unsafe {
        RegQueryValueExW(
            key.0,
            w!("Path"),
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
            key.0,
            w!("Path"),
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
    // Writing back a lossy conversion would damage the other entries.
    let value = String::from_utf16(&data)
        .map_err(|_| "Your PATH holds text tTerm cannot read, so it was left alone.".to_string())?;
    Ok(Some((value, kind)))
}

fn write(value: &str, kind: REG_VALUE_TYPE) -> Result<(), String> {
    let key = EnvironmentKey::open(KEY_SET_VALUE)?;
    let bytes: Vec<u8> = value
        .encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(u16::to_le_bytes)
        .collect();
    // SAFETY: the key is open for writing, and the call only reads `bytes`.
    unsafe { RegSetValueExW(key.0, w!("Path"), None, kind, Some(&bytes)) }
        .ok()
        .map_err(|error| format!("Failed to change your PATH: {error}"))
}

/// Tells running programs, Explorer above all, that the environment changed,
/// so the programs they start next see it.
fn broadcast_change() {
    // SAFETY: the message carries a pointer to a static string, which the
    // receivers only read, and the call returns once they have.
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(w!("Environment").as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            BROADCAST_TIMEOUT_MS,
            None,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIR: &str = r"C:\Users\me\AppData\Local\com.stone.tTerm\bin";

    #[test]
    fn finds_the_directory_however_it_is_written() {
        assert!(has_entry(&format!(r"C:\Windows;{DIR}"), DIR));
        assert!(has_entry(&format!(r#"C:\Windows;"{DIR}\""#), DIR));
        assert!(has_entry(&DIR.to_uppercase(), DIR));
        assert!(!has_entry(
            r"C:\Windows;C:\Users\me\AppData\Local\tTerm",
            DIR
        ));
        assert!(!has_entry("", DIR));
    }

    #[test]
    fn adds_the_directory_at_the_end() {
        assert_eq!(with_entry("", DIR), DIR);
        assert_eq!(
            with_entry(r"C:\Windows;", DIR),
            format!(r"C:\Windows;{DIR}")
        );
        assert_eq!(
            with_entry(r"%USERPROFILE%\bin", DIR),
            format!(r"%USERPROFILE%\bin;{DIR}")
        );
    }

    #[test]
    fn removes_only_the_directory() {
        let value = format!(r"C:\Windows;{DIR};%USERPROFILE%\bin;{DIR}\");
        assert_eq!(without_entry(&value, DIR), r"C:\Windows;%USERPROFILE%\bin");
        assert_eq!(without_entry(r"C:\Windows", DIR), r"C:\Windows");
    }
}
