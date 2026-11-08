#[path = "support/recording.rs"]
mod recording;
#[path = "support/screen.rs"]
mod screen;

use std::path::PathBuf;
use std::time::Duration;

use tterm_terminal::grid::{CellSize, GridSize};
use tterm_terminal::terminal::{Config, Terminal};

const NAMES: &[&str] = &[
    "session",
    "unicode",
    "alternate-restored",
    "alternate-active",
    "scroll-region",
    "margin-motion",
    "attributes",
    "charset-replies",
];

fn directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/replay")
}

fn replay(bytes: &[u8], split: bool) -> String {
    let terminal = Terminal::new(
        Config {
            scrollback: 12,
            ..Config::default()
        },
        GridSize::new(40, 8),
        CellSize::new(8, 19),
    );
    for frame in recording::decode(bytes).unwrap() {
        if split && let recording::Action::Output(bytes) = &frame.action {
            for byte in bytes {
                terminal.write(&[*byte]);
            }
        } else {
            frame.apply(&terminal);
        }
    }
    screen::screen(&terminal)
}

#[test]
fn replay_matches_xterm_or_the_exact_documented_differences() {
    let mut unexpected = Vec::new();
    for name in NAMES {
        let bytes = std::fs::read(directory().join(format!("{name}.bin"))).unwrap();
        let reference =
            std::fs::read_to_string(directory().join(format!("{name}.screen"))).unwrap();
        let actual = replay(&bytes, false);
        let diff = screen::differences(&reference, &actual);
        // An explicit diagnostic export never updates the reviewed fixture expectations.
        if let Ok(export) = std::env::var("TTERM_REPLAY_EXPORT") {
            let export = PathBuf::from(export);
            std::fs::create_dir_all(&export).unwrap();
            std::fs::write(export.join(format!("{name}.diff")), &diff).unwrap();
            std::fs::write(export.join(format!("{name}.screen")), &actual).unwrap();
        }
        let expected = std::fs::read_to_string(directory().join(format!("{name}.diff"))).unwrap();
        if diff != expected {
            unexpected.push(format!("{name}:\n{diff}"));
        }
        println!(
            "replay {name}: {} changed records",
            diff.lines().count() / 2
        );
        assert_eq!(
            actual,
            replay(&bytes, true),
            "{name}: UTF-8/CSI chunk boundaries changed the result"
        );
    }
    assert!(
        unexpected.is_empty(),
        "Review individual differences:\n{}",
        unexpected.join("\n")
    );
}

#[test]
fn recording_preserves_microseconds_and_rejects_invalid_frames() {
    let bytes = std::fs::read(directory().join("session.bin")).unwrap();
    let frames = recording::decode(&bytes).unwrap();
    assert_eq!(frames.len(), 8);
    assert_eq!(frames[6].at, Duration::from_micros(460_000));
    assert_eq!(frames.last().unwrap().at, Duration::from_micros(600_000));
    assert!(recording::decode(&bytes[..bytes.len() - 1]).is_err());
    let mut bad = bytes.clone();
    bad[0] = 2;
    assert!(recording::decode(&bad).is_err());
    bad = bytes.clone();
    bad[9..11].copy_from_slice(&1_u16.to_le_bytes());
    assert!(recording::decode(&bad).is_err());
    bad = bytes;
    bad[1..9].copy_from_slice(&1_000_000_u64.to_le_bytes());
    assert!(recording::decode(&bad).is_err());
}
