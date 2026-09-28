# ZMODEM File Transfer (rz / sz)

English | [简体中文](./zmodem.zh-CN.md)

When SFTP isn't available (network-equipment consoles, serial/embedded targets, bastion hosts that only expose an interactive shell), you can move files right inside the terminal with `rz` / `sz`, just like SecureCRT / Xshell. It works in both local terminals and SSH sessions with no extra setup.

> The remote side needs `lrzsz` installed (e.g. `apt install lrzsz`, `yum install lrzsz`; for local terminals on macOS, `brew install lrzsz`).

## Download (remote → local)

```bash
sz file.tar.gz            # one file
sz a.log b.log c.log      # several files, transferred in sequence
```

tTerm detects the transfer and starts receiving immediately, with no dialog. Files are saved to the ZMODEM download directory (the system Downloads folder by default). Existing files are never overwritten; a name clash becomes `file (1).tar.gz`.

## Upload (local → remote)

```bash
rz
```

When tTerm sees `rz` start, it opens a file picker (multi-select). The chosen files are uploaded into the remote working directory. Cancelling the picker tells the remote `rz` to exit cleanly.

## Progress and cancellation

- Progress, speed, and results appear in the Transfer Manager, alongside SFTP tasks.
- To cancel, click cancel in the Transfer Manager or press `Ctrl+C` in the terminal. Both send the standard abort sequence, so the remote `rz`/`sz` exits right away instead of timing out.
- While a transfer is running, keystrokes other than `Ctrl+C` are held back so they can't corrupt the data stream.
- If an SSH session reconnects, any unfinished transfer is cleaned up, since the remote process ended with the old connection.

## Settings and manual trigger

- **Settings → General → ZMODEM**:
  - **Auto-detect ZMODEM transfers** (on by default): when off, tTerm stops scanning terminal output for rz/sz trigger sequences.
  - **Download directory**: where received files are saved. Leave empty to use the system Downloads folder.
- **Manual trigger**: if auto-detect is off or doesn't fire, choose **"Send files with ZMODEM (rz)"** or **"Receive files with ZMODEM (sz)"** from the terminal context menu, then run `rz` or `sz <file>`. You can also bind shortcuts to both actions under **Settings → Keyboard shortcuts → ZMODEM** (unbound by default). A manual trigger applies only to the next transfer in the current tab.

## SFTP or ZMODEM?

They don't conflict, and there's no choice to make at connect time. SFTP runs on its own SSH subchannel and is better for browsing and for bulk or large transfers. ZMODEM rides the terminal stream itself, so it works where there's no SFTP subsystem, through multi-hop jumps, `su`, or `docker exec`, or when you just want to grab one file from the shell's current directory. Prefer SFTP when the server supports it.

## Implementation

The ZMODEM protocol lives entirely in the Rust backend (`src-tauri/src/zmodem/`). The frontend only handles settings, file picking, and progress display.

- **Custom push-based engine**: the existing `zmodem` crate isn't used (it does blocking synchronous I/O, supports a single file only, and is unmaintained). `protocol/engine.rs` is a pure state machine, `feed(bytes) -> { outgoing, actions, passthrough }`, that owns no I/O handle. That's required because `portable-pty` allows `take_writer()` only once, so each caller (the local PTY reader thread or the SSH reader task) writes `outgoing` back through the handle it already holds.
- **Protocol coverage**: CRC16 and CRC32 (CRC32 when sending), ZDLE escaping, hex and binary headers, `ZRPOS` resends, and multi-file batch receive. All wire formats were checked against real `lrzsz` captures.
- **Trigger detection**: `detect.rs` scans terminal output for `**\x18B00` (remote ran `sz`, so we receive) and `**\x18B01` (remote ran `rz`, so we send). It keeps a small carry buffer so a trigger split across two reads is still caught.
- **Data path**: the local PTY and SSH byte pipelines are already binary-safe, so ZMODEM data never goes through UTF-8 conversion. Once a transfer starts, terminal output goes to the engine instead of xterm. Leftover bytes after the transfer (such as the next shell prompt) still reach xterm.
- **Receive**: handled inline on the reader thread or task. Files go to the download directory, and filenames are sanitized to block path traversal.
- **Send**: runs on a dedicated OS thread (`send.rs`), because the sender has to stream chunks without waiting for a per-chunk ack.
  - The reader forwards peer bytes to that thread over `std::sync::mpsc`. When the thread finishes, the channel closes and the reader goes back to normal handling.
  - During local PTY sends, the terminal is switched to raw mode (`cfmakeraw`) so echo and XON/XOFF flow control can't corrupt binary frames.
  - As a fallback, the `ZFILE` header is resent on `ZNAK` or a repeated `ZRINIT`. Over SSH this is the only protection, since the remote terminal mode is outside our control.
- **State**: per-tab state lives in `ZmodemMap`, `ZmodemArmedSendMap`, and `ZmodemManualDetectMap` (`core/state.rs`), guarded by `session_nonce` so a stale session can't reuse it. These maps are touched from both OS threads and async tasks, so they use `std::sync` locks, not tokio locks' `blocking_*` methods.
- **Frontend**: `useZmodemTransfers.ts` listens for the `zmodem-send-requested-{tabId}` and `zmodem-transfer-start/progress/complete-{tabId}` events and feeds the existing Transfer Manager. `TransferTask.cancel` lets ZMODEM tasks use their own cancel path.

Besides unit tests, there are 8 integration tests that need real tools (`#[ignore]` by default). They run real `sz`/`rz` over plain pipes, a local PTY, and a throwaway userspace `sshd`, covering both upload and download:

```bash
brew install lrzsz   # or your platform's lrzsz package
cd src-tauri && cargo test --lib zmodem::real -- --ignored --test-threads=1
```

