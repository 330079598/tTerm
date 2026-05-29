use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[tauri::command]
pub async fn list_fonts() -> Vec<String> {
    tokio::task::spawn_blocking(list_fonts_sync)
        .await
        .unwrap_or_default()
}

fn list_fonts_sync() -> Vec<String> {
    let mut font_names: HashSet<String> = HashSet::new();

    // Try to get fonts from system API first (most reliable)
    #[cfg(target_os = "windows")]
    {
        if let Ok(system_fonts) = get_windows_fonts() {
            font_names.extend(system_fonts);
        }
    }

    // Fallback: scan font directories and parse font files
    let dirs = get_font_directories();
    for dir in &dirs {
        collect_fonts_from_dir(dir, &mut font_names);
    }

    let mut result: Vec<String> = font_names.into_iter().collect();
    result.sort();
    result
}

fn get_font_directories() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();

    #[cfg(target_os = "macos")]
    {
        dirs.push(PathBuf::from("/System/Library/Fonts"));
        dirs.push(PathBuf::from("/System/Library/Fonts/Supplemental"));
        dirs.push(PathBuf::from("/Library/Fonts"));
        dirs.push(PathBuf::from("/Library/Fonts/Supplemental"));
        if let Ok(home) = std::env::var("HOME") {
            dirs.push(PathBuf::from(&home).join("Library/Fonts"));
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(windir) = std::env::var("WINDIR") {
            dirs.push(PathBuf::from(&windir).join("Fonts"));
        } else {
            dirs.push(PathBuf::from("C:/Windows/Fonts"));
        }
        if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
            dirs.push(PathBuf::from(&localappdata).join("Microsoft/Windows/Fonts"));
        }
        if let Ok(appdata) = std::env::var("APPDATA") {
            dirs.push(PathBuf::from(&appdata).join("Microsoft/Windows/Fonts"));
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        dirs.push(PathBuf::from("/usr/share/fonts"));
        dirs.push(PathBuf::from("/usr/local/share/fonts"));
        if let Ok(home) = std::env::var("HOME") {
            dirs.push(PathBuf::from(&home).join(".fonts"));
            dirs.push(PathBuf::from(&home).join(".local/share/fonts"));
        }
    }

    dirs
}

#[cfg(target_os = "windows")]
fn get_windows_fonts() -> Result<HashSet<String>, Box<dyn std::error::Error>> {
    use std::sync::Mutex;
    use windows::Win32::Foundation::LPARAM;
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, EnumFontFamiliesExW, DEFAULT_CHARSET, FONT_CHARSET, LOGFONTW,
    };

    let fonts = Mutex::new(HashSet::new());

    unsafe {
        let hdc = CreateCompatibleDC(None);
        if hdc.is_invalid() {
            return Err("Failed to create device context".into());
        }

        let mut logfont: LOGFONTW = std::mem::zeroed();
        logfont.lfCharSet = FONT_CHARSET(DEFAULT_CHARSET.0);

        let fonts_ptr = &fonts as *const Mutex<HashSet<String>> as isize;

        unsafe extern "system" fn enum_font_callback(
            lpelfe: *const LOGFONTW,
            _lpntme: *const windows::Win32::Graphics::Gdi::TEXTMETRICW,
            _font_type: u32,
            lparam: LPARAM,
        ) -> i32 {
            let fonts = &*(lparam.0 as *const Mutex<HashSet<String>>);
            if let Some(logfont) = lpelfe.as_ref() {
                let name = String::from_utf16_lossy(&logfont.lfFaceName);
                let name = name.trim_end_matches('\0').to_string();
                if !name.is_empty() && !name.starts_with('@') {
                    fonts.lock().unwrap().insert(name);
                }
            }
            1 // Continue enumeration
        }

        EnumFontFamiliesExW(
            hdc,
            &logfont,
            Some(enum_font_callback),
            LPARAM(fonts_ptr),
            0,
        );

        let _ = DeleteDC(hdc);
    }

    Ok(fonts.into_inner().unwrap())
}

fn collect_fonts_from_dir(dir: &Path, names: &mut HashSet<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let path = entry.path();

        // Avoid following symlinked directories, which can create loops and keep
        // the font picker stuck in a perpetual loading state.
        if file_type.is_dir() {
            collect_fonts_from_dir(&path, names);
        } else if file_type.is_file() {
            let Some(ext) = path.extension() else {
                continue;
            };
            let ext = ext.to_string_lossy().to_lowercase();
            if matches!(ext.as_str(), "ttf" | "otf" | "ttc") {
                // Try to parse font file to get real font name
                if let Some(name) = parse_font_name(&path) {
                    names.insert(name);
                } else if let Some(name) = extract_font_name_from_filename(&path) {
                    // Fallback to filename-based extraction
                    names.insert(name);
                }
            }
        }
    }
}

fn parse_font_name(path: &Path) -> Option<String> {
    let data = fs::read(path).ok()?;

    // Handle TTC (TrueType Collection) files
    if path.extension()?.to_string_lossy().to_lowercase() == "ttc" {
        // For TTC files, try to parse the first font in the collection
        if let Ok(face) = ttf_parser::Face::parse(&data, 0) {
            return get_font_family_name(&face);
        }
        return None;
    }

    // Handle regular TTF/OTF files
    let face = ttf_parser::Face::parse(&data, 0).ok()?;
    get_font_family_name(&face)
}

fn get_font_family_name(face: &ttf_parser::Face) -> Option<String> {
    // Try to get the font family name from the name table
    // Prefer English names (language ID 0x0409 = en-US)
    for name in face.names() {
        if name.name_id == ttf_parser::name_id::FAMILY {
            // Prefer English (US) names
            if name.language_id == 0x0409 {
                if let Some(name_str) = name.to_string() {
                    return Some(name_str);
                }
            }
        }
    }

    // Fallback: try any language for family name
    for name in face.names() {
        if name.name_id == ttf_parser::name_id::FAMILY {
            if let Some(name_str) = name.to_string() {
                return Some(name_str);
            }
        }
    }

    None
}

fn extract_font_name_from_filename(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_string_lossy().to_string();
    let suffixes = [
        "-Bold",
        "-Italic",
        "-BoldItalic",
        "-Regular",
        "-Light",
        "-Medium",
        "-Thin",
        "-Black",
        "-Heavy",
        "-SemiBold",
        "-ExtraBold",
        "-ExtraLight",
        "-Condensed",
        "-Oblique",
        "-Mono",
        "-NF",
        "-NerdFont",
        "Bold",
        "Italic",
        "Regular",
        "Light",
        "Medium",
    ];
    let mut name = stem.clone();
    for suffix in &suffixes {
        if let Some(stripped) = name.strip_suffix(suffix) {
            name = stripped
                .trim_end_matches('-')
                .trim_end_matches(' ')
                .to_string();
            break;
        }
    }

    let display = name.replace('_', " ");
    if display.is_empty() {
        None
    } else {
        Some(display)
    }
}
