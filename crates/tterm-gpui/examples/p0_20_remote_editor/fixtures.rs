//! Small language fixtures and a reproducible runtime-only 50 MiB document.

use std::io::Write;
use std::path::PathBuf;

/// The seven language families listed on the P0-20 task card.
pub const LANGUAGES: [&str; 7] = [
    "css",
    "html",
    "javascript",
    "json",
    "markdown",
    "rust",
    "yaml",
];

/// Supplies recognizable syntax and Chinese text without external files.
pub fn sample(language: &str) -> &'static str {
    match language {
        "css" => "/* 中文 */\nbody { color: red; margin: 12px; }\n",
        "html" => "<!-- 中文 -->\n<h1 class=\"title\">Hello</h1>\n",
        "javascript" => "// 中文\nconst hello = (name) => `Hello ${name}`;\n",
        "json" => "{\"name\": \"中文\", \"enabled\": true, \"count\": 12}\n",
        "markdown" => "# 中文\n**bold** and `code`\n- item\n",
        "yaml" => "# 中文\nname: hello\nenabled: true\nitems:\n  - one\n",
        _ => "// 中文\nfn main() { println!(\"Hello\"); }\n",
    }
}

/// Generates a disposable file under the system temporary directory, so a
/// failed run never leaves 50 MiB inside the repository.
pub fn large_file() -> Result<PathBuf, String> {
    let folder = std::env::temp_dir().join("tterm-gpui-dev-P0-20");
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    let path = folder.join("50MiB.txt");
    let mut file =
        std::io::BufWriter::new(std::fs::File::create(&path).map_err(|error| error.to_string())?);
    // Exactly 1024 bytes per line, 51,200 lines; no fixture is checked in.
    let line = format!("{}\n", "x".repeat(1023));
    for _ in 0..51_200 {
        file.write_all(line.as_bytes())
            .map_err(|error| error.to_string())?;
    }
    file.flush().map_err(|error| error.to_string())?;
    Ok(path)
}
