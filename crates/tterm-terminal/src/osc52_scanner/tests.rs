use super::Osc52Scanner;

#[test]
fn valid_targets_and_empty_targets_are_stores() {
    for input in [
        b"\x1b]52;c;dGFyZ2V0\x07".as_slice(),
        b"\x1b]52;p;dGFyZ2V0\x1b\\".as_slice(),
        b"\x1b]52;s;dGFyZ2V0\x07".as_slice(),
        b"\x1b]52;q;dGFyZ2V0\x07".as_slice(),
        b"\x1b]52;0;dGFyZ2V0\x07".as_slice(),
        b"\x1b]52;7;dGFyZ2V0\x07".as_slice(),
        b"\x1b]52;;dGFyZ2V0\x07".as_slice(),
    ] {
        let mut scanner = Osc52Scanner::new(true);
        let stores = scanner.scan(input);
        assert_eq!(stores.len(), 1, "{input:?}");
        assert_eq!(stores[0].0, input.len());
        assert_eq!(stores[0].1, "target");
    }
}

#[test]
fn invalid_payloads_and_disabled_scanners_do_not_decode() {
    for input in [
        b"\x1b]52;c;?\x07".as_slice(),
        b"\x1b]52;c;not base64!\x07".as_slice(),
        b"\x1b]52;x;dGFyZ2V0\x07".as_slice(),
    ] {
        assert!(Osc52Scanner::new(true).scan(input).is_empty(), "{input:?}");
    }
    assert!(
        Osc52Scanner::new(false)
            .scan(b"\x1b]52;c;dGFyZ2V0\x07")
            .is_empty()
    );
}

#[test]
fn stores_survive_chunk_splits_and_drop_the_whole_over_bound_sequence() {
    use crate::osc52::MAX_OSC52_PAYLOAD_BYTES;

    let input = b"\x1b]52;q;5L2g5aW977yMdmltCg==\x1b\\";
    for split in 0..=input.len() {
        let mut scanner = Osc52Scanner::new(true);
        let mut stores = scanner.scan(&input[..split]);
        stores.extend(scanner.scan(&input[split..]));
        assert_eq!(stores.len(), 1, "split {split}");
        assert_eq!(stores[0].1, "你好，vim\n");
    }

    let mut scanner = Osc52Scanner::new(true);
    scanner.scan(b"\x1b]52;c;");
    scanner.scan(&vec![b'A'; MAX_OSC52_PAYLOAD_BYTES + 1]);
    assert!(scanner.scan(b"\x1b\\").is_empty());
    assert_eq!(scanner.scan(b"\x1b]52;c;dGFyZ2V0\x07").len(), 1);
}

#[test]
fn model_stores_all_supported_targets_once_in_output_order() {
    use crate::event::{Clipboard, Intent};
    use crate::grid::{CellSize, GridSize};
    use crate::terminal::{Config, Terminal};

    for target in ["", "c", "p", "q", "s", "0", "7", "cpqs01234567"] {
        let input =
            format!("\x1b]0;before\x07\x1b]52;{target};aGVsbG8=\x07\x1b[5n\x1b]0;after\x07");
        for split in 0..=input.len() {
            let model = Terminal::new(
                Config::default(),
                GridSize::new(80, 24),
                CellSize::new(8, 16),
            );
            model.write(&input.as_bytes()[..split]);
            model.write(&input.as_bytes()[split..]);
            let intents = model.drain_intents();
            assert!(
                matches!(&intents[..], [
                Intent::Title(before),
                Intent::ClipboardStore { clipboard: Clipboard::Clipboard, text },
                Intent::WriteToPty(reply),
                Intent::Title(after)
            ] if before == "before" && text == "hello" && reply == b"\x1b[0n" && after == "after"),
                "target {target:?}, split {split}: {intents:?}"
            );
        }
    }
}

#[test]
fn cancelled_stores_do_not_copy_and_following_stores_survive() {
    for abort in [0x18, 0x1a] {
        let mut scanner = Osc52Scanner::new(true);
        let mut input = b"\x1b]52;c;aGVsbG8=".to_vec();
        input.push(abort);
        assert!(scanner.scan(&input).is_empty());
        assert_eq!(scanner.scan(b"\x1b]52;q;dGFyZ2V0\x07")[0].1, "target");
    }
}
