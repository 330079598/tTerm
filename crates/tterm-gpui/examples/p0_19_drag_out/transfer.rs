//! Bounded-memory simulated download, isolated in a per-drag temporary directory.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

pub const BYTES: u64 = 200_000_000;
pub const BLOCK: usize = 64 * 1024;
pub const NAME: &str = "generated-200MB.bin";

/// An exclusively created directory; only this directory is cleaned up.
pub struct Folder(pub PathBuf);

impl Folder {
    pub fn new() -> Result<Arc<Self>, String> {
        let base = std::env::temp_dir()
            .join("com.stone.tTerm.gpui-dev")
            .join("P0-19");
        fs::create_dir_all(&base).map_err(|e| e.to_string())?;
        for sequence in 0..1000 {
            let path = base.join(format!("drag-{}-{sequence}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Arc::new(Self(path))),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
        Err("Temporary drag directories exhausted; remove stale P0-19 directories".into())
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("Could not clean {}: {error}", self.0.display());
        }
    }
}

pub struct Generated {
    pub folder: Arc<Folder>,
    pub path: PathBuf,
}

/// Writes actual content, not a sparse length, using one fixed 64 KiB buffer.
pub fn generate(folder: &Arc<Folder>) -> Result<Generated, String> {
    let start = Instant::now();
    let path = folder.0.join(NAME);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    let block: Vec<u8> = (0..BLOCK).map(|offset| (offset % 251) as u8).collect();
    println!("generate_started bytes={BYTES} buffer={BLOCK}");
    let mut remaining = BYTES;
    while remaining != 0 {
        let count = remaining.min(BLOCK as u64) as usize;
        file.write_all(&block[..count]).map_err(|e| e.to_string())?;
        remaining -= count as u64;
    }
    file.sync_all().map_err(|e| e.to_string())?;
    println!(
        "generate_finished bytes={BYTES} elapsed_ms={}",
        start.elapsed().as_millis()
    );
    Ok(Generated {
        folder: Arc::clone(folder),
        path,
    })
}

/// Never replaces an existing destination, even if the directory changed after startup.
pub fn publish(file: &Generated, target: &Path) -> Result<String, String> {
    let destination = target.join(NAME);
    let mut input = File::open(&file.path).map_err(|e| e.to_string())?;
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&destination)
        .map_err(|e| e.to_string())?;
    let result = std::io::copy(&mut input, &mut output).and_then(|bytes| {
        output.sync_all()?;
        Ok(bytes)
    });
    drop(output);
    let bytes = match result {
        Ok(bytes) => bytes,
        Err(error) => {
            let _ = fs::remove_file(&destination);
            return Err(error.to_string());
        }
    };
    verify(&destination)?;
    Ok(format!(
        "Published and verified bytes={bytes} path={} staging={}",
        destination.display(),
        file.folder.0.display()
    ))
}

/// Checks every byte in the generated pattern with fixed memory.
pub fn verify(path: &Path) -> Result<(), String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() != BYTES {
        return Err("Generated file has the wrong size".into());
    }
    let mut buffer = vec![0; BLOCK];
    let mut offset = 0;
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            return Ok(());
        }
        for (index, byte) in buffer[..count].iter().enumerate() {
            if *byte != (((offset + index) % BLOCK) % 251) as u8 {
                return Err(format!("Content differs at byte {}", offset + index));
            }
        }
        offset += count;
    }
}

pub fn validate_target(path: &Path) -> Result<PathBuf, String> {
    let target = fs::canonicalize(path).map_err(|e| e.to_string())?;
    let temp = fs::canonicalize(std::env::temp_dir()).map_err(|e| e.to_string())?;
    if !target.starts_with(&temp) || target == temp || !target.is_dir() {
        return Err("Target must be a new empty directory beneath TEMP".into());
    }
    if fs::read_dir(&target)
        .map_err(|e| e.to_string())?
        .next()
        .is_some()
    {
        return Err("Target is not empty; create another directory beneath TEMP".into());
    }
    Ok(target)
}

pub fn local_check() -> Result<(), String> {
    let folder = Folder::new()?;
    if folder.0.join(NAME).exists() {
        return Err("Content exists before trigger".into());
    }
    println!("before_trigger content_exists=false");
    let generated = generate(&folder)?;
    let target = folder.0.join("destination");
    fs::create_dir(&target).map_err(|e| e.to_string())?;
    let target = validate_target(&target)?;
    println!("{}", publish(&generated, &target)?);
    if publish(&generated, &target).is_ok() {
        return Err("Destination was overwritten".into());
    }
    println!("existing_destination preserved=true; local-check passed (not a native drag)");
    Ok(())
}
