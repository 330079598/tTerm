use super::{
    DEFAULT_REPLAY_SIZE, ReplayBox, ReplayFrame, ReplaySize, build_replay_timeline,
    fit_replay_font_size, format_replay_time, frames_played_by, initial_replay_size,
    parse_replay_frames, replay_font_size,
};

struct Frame {
    at_us: u64,
    data: Option<&'static str>,
    cols: u16,
    rows: u16,
}

fn output(at_us: u64, data: &'static str) -> Frame {
    Frame {
        at_us,
        data: Some(data),
        cols: 0,
        rows: 0,
    }
}

fn resize(at_us: u64, cols: u16, rows: u16) -> Frame {
    Frame {
        at_us,
        data: None,
        cols,
        rows,
    }
}

fn encode(frames: &[Frame]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for frame in frames {
        match frame.data {
            Some(data) => {
                bytes.push(0);
                bytes.extend(frame.at_us.to_le_bytes());
                bytes.extend((data.len() as u32).to_le_bytes());
                bytes.extend(data.as_bytes());
            }
            None => {
                bytes.push(1);
                bytes.extend(frame.at_us.to_le_bytes());
                bytes.extend(frame.cols.to_le_bytes());
                bytes.extend(frame.rows.to_le_bytes());
            }
        }
    }
    bytes
}

fn output_frames() -> Vec<ReplayFrame> {
    vec![
        ReplayFrame::Output {
            at_ms: 1000.0,
            data: Vec::new(),
        },
        ReplayFrame::Output {
            at_ms: 1200.0,
            data: Vec::new(),
        },
        ReplayFrame::Output {
            at_ms: 61_200.0,
            data: Vec::new(),
        },
    ]
}

#[test]
fn parses_output_and_size_frames() {
    let frames = parse_replay_frames(&encode(&[resize(0, 100, 30), output(1500, "hi")]));
    assert_eq!(
        frames,
        vec![
            ReplayFrame::Resize {
                at_ms: 0.0,
                cols: 100,
                rows: 30,
            },
            ReplayFrame::Output {
                at_ms: 1.5,
                data: b"hi".to_vec(),
            },
        ]
    );
    assert_eq!(
        initial_replay_size(&frames),
        ReplaySize {
            cols: 100,
            rows: 30
        }
    );
}

#[test]
fn stops_at_a_truncated_frame() {
    let mut buffer = encode(&[output(0, "hello")]);
    buffer.truncate(buffer.len() - 2);
    assert!(parse_replay_frames(&buffer).is_empty());
}

#[test]
fn shortens_long_pauses_when_asked() {
    let frames = output_frames();
    assert_eq!(
        build_replay_timeline(&frames, None),
        vec![0.0, 200.0, 60_200.0]
    );
    assert_eq!(
        build_replay_timeline(&frames, Some(2000.0)),
        vec![0.0, 200.0, 2200.0]
    );
}

#[test]
fn counts_the_frames_played_by_a_time() {
    let timeline = [0.0, 200.0, 2200.0];
    assert_eq!(frames_played_by(&timeline, -1.0), 0);
    assert_eq!(frames_played_by(&timeline, 0.0), 1);
    assert_eq!(frames_played_by(&timeline, 1000.0), 2);
    assert_eq!(frames_played_by(&timeline, 5000.0), 3);
}

#[test]
fn formats_playback_time() {
    assert_eq!(format_replay_time(0.0), "0:00");
    assert_eq!(format_replay_time(-1.0), "0:00");
    assert_eq!(format_replay_time(65_000.0), "1:05");
    assert_eq!(format_replay_time(3_725_000.0), "1:02:05");
    assert_eq!(format_replay_time(59_999.0), "0:59");
}

#[test]
fn fits_the_font_to_one_dimension_of_the_panel() {
    // (1610 - 10) / 800 doubles it.
    assert_eq!(fit_replay_font_size(12.0, 800.0, 10.0, 1610.0), 24.0);
    // Rounds down to a quarter point, and shrinks too.
    assert_eq!(fit_replay_font_size(12.0, 800.0, 0.0, 1010.0), 15.0);
    assert_eq!(fit_replay_font_size(12.0, 800.0, 0.0, 600.0), 9.0);
    // Stays within the font size range.
    assert_eq!(fit_replay_font_size(12.0, 800.0, 10.0, 100.0), 6.0);
    assert_eq!(fit_replay_font_size(12.0, 800.0, 10.0, 1e5), 48.0);
    assert_eq!(fit_replay_font_size(12.0, 0.0, 10.0, 500.0), 12.0);
}

#[test]
fn never_lets_the_recording_overflow_the_panel() {
    let text = ReplayBox {
        width: 800.0,
        height: 400.0,
    };
    let chrome = ReplayBox {
        width: 14.0,
        height: 0.0,
    };
    // The preferred size fits.
    assert_eq!(
        replay_font_size(
            12.0,
            14.0,
            false,
            text,
            chrome,
            ReplayBox {
                width: 2000.0,
                height: 1000.0,
            },
        ),
        super::ReplayFontSize {
            font_size: 14.0,
            capped: false,
        }
    );
    // The height holds the preferred size back: 12 * 450 / 400 = 13.5.
    assert_eq!(
        replay_font_size(
            12.0,
            16.0,
            false,
            text,
            chrome,
            ReplayBox {
                width: 2000.0,
                height: 450.0,
            },
        ),
        super::ReplayFontSize {
            font_size: 13.5,
            capped: true,
        }
    );
    // So does the width: 12 * (1014 - 14) / 800 = 15.
    assert_eq!(
        replay_font_size(
            12.0,
            16.0,
            false,
            text,
            chrome,
            ReplayBox {
                width: 1014.0,
                height: 1000.0,
            },
        ),
        super::ReplayFontSize {
            font_size: 15.0,
            capped: true,
        }
    );
    // Fitting takes the width when the height allows it.
    assert_eq!(
        replay_font_size(
            12.0,
            12.0,
            true,
            text,
            chrome,
            ReplayBox {
                width: 1214.0,
                height: 1000.0,
            },
        ),
        super::ReplayFontSize {
            font_size: 18.0,
            capped: false,
        }
    );
    // And the height when it does not.
    assert_eq!(
        replay_font_size(
            12.0,
            12.0,
            true,
            text,
            chrome,
            ReplayBox {
                width: 1214.0,
                height: 500.0,
            },
        ),
        super::ReplayFontSize {
            font_size: 15.0,
            capped: false,
        }
    );
    assert_eq!(initial_replay_size(&[]), DEFAULT_REPLAY_SIZE);
}
