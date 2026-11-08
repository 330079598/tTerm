//! Local-only format and conflict experiment, not the production SFTP saver.

use std::path::Path;

#[derive(Clone, Copy, Debug)]
enum Encoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
}

/// A decoded local fixture together with the bytes needed for a save preflight.
pub struct Document {
    /// LF-normalized text presented to the editor.
    pub text: String,
    original: Vec<u8>,
    encoding: Encoding,
    newline: &'static str,
}

impl Document {
    /// Loads only the explicitly supplied local fixture.
    pub fn open(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
        Self::decode(bytes)
    }

    /// Rejects inputs whose encoding or per-line endings cannot be preserved.
    pub fn decode(original: Vec<u8>) -> Result<Self, String> {
        let (encoding, text) = if original.starts_with(&[0xff, 0xfe]) {
            (Encoding::Utf16Le, decode_utf16(&original[2..], true)?)
        } else if original.starts_with(&[0xfe, 0xff]) {
            (Encoding::Utf16Be, decode_utf16(&original[2..], false)?)
        } else {
            let bom = original.starts_with(&[0xef, 0xbb, 0xbf]);
            let body = if bom { &original[3..] } else { &original };
            let text = std::str::from_utf8(body).map_err(
                |_| "Unsupported encoding: choose an external editor; no lossy decoding.",
            )?;
            (
                if bom {
                    Encoding::Utf8Bom
                } else {
                    Encoding::Utf8
                },
                text.to_string(),
            )
        };
        let newline = if text.contains("\r\n") {
            "\r\n"
        } else if text.contains('\r') {
            "\r"
        } else {
            "\n"
        };
        let normalized = text.replace(newline, "\n");
        if normalized.contains('\r') || (newline == "\r" && text.contains('\n')) {
            return Err(
                "Mixed newlines: use an external editor to preserve per-line endings.".into(),
            );
        }
        // CRLF mixed with LF cannot be reconstructed from normalized editor text.
        if newline == "\r\n" && text.replace("\r\n", "").contains('\n') {
            return Err(
                "Mixed newlines: use an external editor to preserve per-line endings.".into(),
            );
        }
        Ok(Self {
            text: normalized,
            original,
            encoding,
            newline,
        })
    }

    /// Restores the detected encoding, BOM and uniform newline convention.
    pub fn encode(&self, text: &str) -> Vec<u8> {
        let text = text.replace('\n', self.newline);
        match self.encoding {
            Encoding::Utf8 => text.into_bytes(),
            Encoding::Utf8Bom => [vec![0xef, 0xbb, 0xbf], text.into_bytes()].concat(),
            Encoding::Utf16Le | Encoding::Utf16Be => {
                let little = matches!(self.encoding, Encoding::Utf16Le);
                let mut bytes = if little {
                    vec![0xff, 0xfe]
                } else {
                    vec![0xfe, 0xff]
                };
                for unit in text.encode_utf16() {
                    bytes.extend(if little {
                        unit.to_le_bytes()
                    } else {
                        unit.to_be_bytes()
                    });
                }
                bytes
            }
        }
    }

    /// A byte comparison detects even same-size edits with unchanged timestamps.
    /// This is a preflight check; remote compare-and-write needs server support.
    pub fn save(&mut self, path: &Path, text: &str) -> Result<(), String> {
        let current = std::fs::read(path).map_err(|error| error.to_string())?;
        if current != self.original {
            return Err("Conflict: file changed since opening; reload before saving.".into());
        }
        let encoded = self.encode(text);
        std::fs::write(path, &encoded).map_err(|error| error.to_string())?;
        self.original = encoded;
        self.text = text.to_string();
        Ok(())
    }
}

fn decode_utf16(bytes: &[u8], little: bool) -> Result<String, String> {
    if !bytes.len().is_multiple_of(2) {
        return Err("Invalid UTF-16: odd byte count.".into());
    }
    let units: Vec<_> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let pair = [pair[0], pair[1]];
            if little {
                u16::from_le_bytes(pair)
            } else {
                u16::from_be_bytes(pair)
            }
        })
        .collect();
    String::from_utf16(&units).map_err(|error| error.to_string())
}

/// Exercises only disposable files below the isolated GPUI probe directory.
pub fn local_check() -> Result<(), String> {
    let folder = std::path::PathBuf::from("target/com.stone.tTerm.gpui-dev/P0-20")
        .join(format!("formats-{}", std::process::id()));
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    let result = check_formats(&folder);
    std::fs::remove_dir_all(&folder).map_err(|error| error.to_string())?;
    result
}

fn check_formats(folder: &Path) -> Result<(), String> {
    for encoding in [
        Encoding::Utf8,
        Encoding::Utf8Bom,
        Encoding::Utf16Le,
        Encoding::Utf16Be,
    ] {
        for newline in ["\n", "\r\n", "\r"] {
            let template = Document {
                text: String::new(),
                original: vec![],
                encoding,
                newline,
            };
            let original = template.encode("中文😀\nsecond\n");
            let path = folder.join("fixture.txt");
            std::fs::write(&path, &original).map_err(|error| error.to_string())?;
            let mut document = Document::open(&path)?;
            if document.text != "中文😀\nsecond\n" || document.encode(&document.text) != original
            {
                return Err(format!("Roundtrip failed: {encoding:?}, {newline:?}"));
            }
            document.save(&path, "中文😀\nchanged\n")?;
            if std::fs::read(&path).map_err(|error| error.to_string())?
                != template.encode("中文😀\nchanged\n")
            {
                return Err("Edited format changed.".into());
            }
            // Same byte length, different contents: a size-only check would miss this.
            let external = template.encode("中文😀\noutside\n");
            std::fs::write(&path, &external).map_err(|error| error.to_string())?;
            if document.save(&path, "overwrite").is_ok()
                || std::fs::read(&path).map_err(|error| error.to_string())? != external
            {
                return Err("Conflict was not preserved.".into());
            }
            println!("format={encoding:?} newline={newline:?}: roundtrip/edit/conflict passed");
        }
    }
    for bytes in [
        vec![0x81, 0x40],
        vec![0xff, 0xfe, 0x00],
        vec![0xff, 0xfe, 0x00, 0xd8],
        b"a\r\nb\n".to_vec(),
    ] {
        if Document::decode(bytes).is_ok() {
            return Err("Unsupported input was accepted.".into());
        }
    }
    Ok(())
}
