use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use portable_pty::{CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[test]
fn conpty_forwards_theme_queries_and_delivers_replies_to_console_apps() {
    let pair = portable_pty::native_pty_system()
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("open ConPTY");
    let mut command = CommandBuilder::new("powershell.exe");
    // Codex queries with ST; other TUIs use BEL. The replies must become console key events,
    // which is how Windows TUIs read them without replacing their normal input mode.
    command.args([
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        r#"[Console]::Write("$([char]27)]10;?$([char]27)\$([char]27)]11;?$([char]7)");
        $reply = ''; $ends = 0; $previous = [char]0;
        while ($ends -lt 2) {
            $key = [Console]::ReadKey($true).KeyChar;
            $reply += $key;
            if ($key -eq '\' -and $previous -eq [char]27) { $ends++ }
            $previous = $key;
        }
        [Console]::Write('REPLY:' + [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($reply)))"#,
    ]);
    let mut child = pair
        .slave
        .spawn_command(command)
        .expect("start console app");
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().expect("PTY reader");
    let mut writer = pair.master.take_writer().expect("PTY writer");
    let (sender, receiver) = mpsc::channel();
    let reading = std::thread::spawn(move || {
        let mut chunk = [0; 4096];
        while let Ok(count) = reader.read(&mut chunk) {
            if count == 0 || sender.send(chunk[..count].to_vec()).is_err() {
                break;
            }
        }
    });

    let replies = b"\x1b]10;rgb:dada/dada/dada\x1b\\\x1b]11;rgb:1c1c/0e0e/2626\x1b\\";
    let expected = format!("REPLY:{}", STANDARD.encode(replies));
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut output = Vec::new();
    let mut cursor_replied = false;
    let mut attributes_replied = false;
    let mut colors_replied = false;
    while Instant::now() < deadline {
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(chunk) => output.extend(chunk),
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        if !cursor_replied && contains(&output, b"\x1b[6n") {
            writer
                .write_all(b"\x1b[1;1R")
                .expect("initial cursor reply");
            cursor_replied = true;
        }
        if !attributes_replied && contains(&output, b"\x1b[c") {
            writer
                .write_all(b"\x1b[?1;2c")
                .expect("device attributes reply");
            attributes_replied = true;
        }
        if !colors_replied
            && contains(&output, b"\x1b]10;?\x1b\\")
            && contains(&output, b"\x1b]11;?\x07")
        {
            writer.write_all(replies).expect("theme color replies");
            colors_replied = true;
        }
        writer.flush().expect("flush replies");
        if contains(&output, expected.as_bytes()) {
            break;
        }
    }
    // Clean up before asserting so a failed query cannot leave a shell waiting for input.
    let _ = child.kill();
    let _ = child.wait();
    drop(writer);
    drop(pair.master);
    drop(receiver);
    reading.join().expect("join PTY reader");
    assert!(colors_replied, "ConPTY swallowed OSC 10/11: {output:?}");
    assert!(
        contains(&output, expected.as_bytes()),
        "The console app did not receive its theme replies: {output:?}"
    );
}

fn contains(bytes: &[u8], needle: &[u8]) -> bool {
    bytes.windows(needle.len()).any(|part| part == needle)
}
