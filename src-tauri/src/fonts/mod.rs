use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

#[tauri::command]
pub fn list_fonts() -> Vec<String> {
    let mut dirs: Vec<PathBuf> = Vec::new();

    #[cfg(target_os = "macos")]
    {
        dirs.push(PathBuf::from("/System/Library/Fonts"));
        dirs.push(PathBuf::from("/Library/Fonts"));
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

    let mut font_names: HashSet<String> = HashSet::new();

    for dir in &dirs {
        collect_fonts_from_dir(dir, &mut font_names);
    }

    let mut result: Vec<String> = font_names.into_iter().collect();
    result.sort();
    result
}

fn collect_fonts_from_dir(dir: &PathBuf, names: &mut HashSet<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_fonts_from_dir(&path, names);
        } else if let Some(ext) = path.extension() {
            let ext = ext.to_string_lossy().to_lowercase();
            if matches!(ext.as_str(), "ttf" | "otf" | "ttc") {
                if let Some(name) = extract_font_name(&path) {
                    names.insert(name);
                }
            }
        }
    }
}

fn extract_font_name(path: &PathBuf) -> Option<String> {
    let stem = path.file_stem()?.to_string_lossy().to_string();
    let suffixes = [
        "-Bold", "-Italic", "-BoldItalic", "-Regular", "-Light", "-Medium",
        "-Thin", "-Black", "-Heavy", "-SemiBold", "-ExtraBold", "-ExtraLight",
        "-Condensed", "-Oblique", "-Mono", "-NF", "-NerdFont", "Bold", "Italic",
        "Regular", "Light", "Medium",
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
