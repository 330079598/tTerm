use std::path::PathBuf;

#[tauri::command]
pub fn read_clipboard_file_paths() -> Result<Vec<String>, String> {
    platform::read_clipboard_file_paths()
}

#[cfg(not(target_os = "windows"))]
fn path_from_file_uri(uri: &str) -> Option<String> {
    let trimmed = uri.trim();
    let path = trimmed
        .strip_prefix("file://localhost/")
        .or_else(|| trimmed.strip_prefix("file:///"))
        .map(|path| format!("/{path}"))
        .or_else(|| trimmed.strip_prefix("file://").map(ToOwned::to_owned))?;

    if !path.starts_with('/') {
        return None;
    }

    let decoded = percent_decode(path)?;

    Some(decoded)
}

#[cfg(not(target_os = "windows"))]
fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hi = bytes.get(index + 1).copied()?;
            let lo = bytes.get(index + 2).copied()?;
            output.push(from_hex_pair(hi, lo)?);
            index += 3;
            continue;
        }

        output.push(bytes[index]);
        index += 1;
    }

    String::from_utf8(output).ok()
}

#[cfg(not(target_os = "windows"))]
fn from_hex_pair(hi: u8, lo: u8) -> Option<u8> {
    Some(from_hex_digit(hi)? * 16 + from_hex_digit(lo)?)
}

#[cfg(not(target_os = "windows"))]
fn from_hex_digit(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[cfg(not(target_os = "windows"))]
fn parse_file_uri_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with('#')
                && *line != "copy"
                && *line != "cut"
                && *line != "1"
                && *line != "0"
        })
        .filter_map(path_from_file_uri)
        .filter(|path| !path.is_empty())
        .collect()
}

fn existing_paths(paths: Vec<String>) -> Vec<String> {
    paths
        .into_iter()
        .filter(|path| PathBuf::from(path).exists())
        .collect()
}

#[cfg(target_os = "windows")]
mod platform {
    use super::existing_paths;
    use windows::Win32::Foundation::MAX_PATH;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    };
    use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};

    const CF_HDROP_FORMAT: u32 = 15;
    const OPEN_CLIPBOARD_ATTEMPTS: usize = 8;
    const OPEN_CLIPBOARD_RETRY_MS: u64 = 25;

    struct ClipboardGuard;

    impl Drop for ClipboardGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseClipboard();
            }
        }
    }

    pub fn read_clipboard_file_paths() -> Result<Vec<String>, String> {
        unsafe {
            let mut last_error = None;
            for attempt in 0..OPEN_CLIPBOARD_ATTEMPTS {
                match OpenClipboard(None) {
                    Ok(()) => {
                        last_error = None;
                        break;
                    }
                    Err(err) => {
                        last_error = Some(err);
                        if attempt + 1 < OPEN_CLIPBOARD_ATTEMPTS {
                            std::thread::sleep(std::time::Duration::from_millis(
                                OPEN_CLIPBOARD_RETRY_MS,
                            ));
                        }
                    }
                }
            }

            if let Some(err) = last_error {
                return Err(format!("Failed to open clipboard: {err}"));
            }

            let _guard = ClipboardGuard;

            if IsClipboardFormatAvailable(CF_HDROP_FORMAT).is_err() {
                return Ok(Vec::new());
            }

            let handle = GetClipboardData(CF_HDROP_FORMAT)
                .map_err(|err| format!("Failed to read clipboard data: {err}"))?;
            if handle.0.is_null() {
                return Ok(Vec::new());
            }

            let drop_handle = HDROP(handle.0);

            let count = DragQueryFileW(drop_handle, u32::MAX, None);
            let mut paths = Vec::with_capacity(count as usize);
            for index in 0..count {
                let mut buffer = [0u16; MAX_PATH as usize];
                let length = DragQueryFileW(drop_handle, index, Some(&mut buffer));
                if length == 0 {
                    continue;
                }

                paths.push(String::from_utf16_lossy(&buffer[..length as usize]));
            }

            Ok(existing_paths(paths))
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::{existing_paths, parse_file_uri_lines};
    use std::process::Command;

    pub fn read_clipboard_file_paths() -> Result<Vec<String>, String> {
        let output = Command::new("osascript")
            .args([
                "-e",
                r#"use framework "AppKit""#,
                "-e",
                r#"set pasteboard to current application's NSPasteboard's generalPasteboard()"#,
                "-e",
                r#"set classes to current application's NSArray's arrayWithObject:(current application's NSURL)"#,
                "-e",
                r#"set options to current application's NSDictionary's dictionary()"#,
                "-e",
                r#"set urls to pasteboard's readObjectsForClasses:classes options:options"#,
                "-e",
                r#"set output to """#,
                "-e",
                r#"repeat with fileUrl in urls"#,
                "-e",
                r#"set output to output & (fileUrl's |path|() as text) & linefeed"#,
                "-e",
                r#"end repeat"#,
                "-e",
                r#"return output"#,
            ])
            .output()
            .map_err(|err| format!("Failed to read macOS clipboard: {err}"))?;

        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            let paths = text
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(ToOwned::to_owned)
                .collect();
            return Ok(existing_paths(paths));
        }

        let uri_output = Command::new("osascript")
            .args(["-e", r#"the clipboard as «class furl»"#])
            .output()
            .map_err(|err| format!("Failed to read macOS file URL clipboard: {err}"))?;

        if !uri_output.status.success() {
            return Ok(Vec::new());
        }

        let text = String::from_utf8_lossy(&uri_output.stdout);
        Ok(existing_paths(parse_file_uri_lines(&text)))
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::{existing_paths, parse_file_uri_lines};
    use std::process::Command;

    pub fn read_clipboard_file_paths() -> Result<Vec<String>, String> {
        for (program, args) in [
            ("wl-paste", vec!["--no-newline", "--type", "x-special/gnome-copied-files"]),
            ("wl-paste", vec!["--no-newline", "--type", "text/uri-list"]),
            ("wl-paste", vec!["--no-newline", "--type", "application/x-kde-cutselection"]),
            ("xclip", vec!["-selection", "clipboard", "-t", "x-special/gnome-copied-files", "-o"]),
            ("xclip", vec!["-selection", "clipboard", "-t", "text/uri-list", "-o"]),
            ("xclip", vec!["-selection", "clipboard", "-t", "application/x-kde-cutselection", "-o"]),
            ("xsel", vec!["--clipboard", "--output"]),
        ] {
            let Ok(output) = Command::new(program).args(args).output() else {
                continue;
            };

            if !output.status.success() {
                continue;
            }

            let text = String::from_utf8_lossy(&output.stdout);
            let paths = existing_paths(parse_file_uri_lines(&text));
            if !paths.is_empty() {
                return Ok(paths);
            }
        }

        Ok(Vec::new())
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
mod platform {
    pub fn read_clipboard_file_paths() -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }
}
