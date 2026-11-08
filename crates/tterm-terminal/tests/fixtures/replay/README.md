# P0-11 synthetic recordings

All bytes were constructed for this test. No application, PTY, real session log,
connection, credential or user data is used. `session.bin` is the permitted
synthetic replacement for a timed tTerm recording. It follows the replay IPC
format in `src/lib/terminalLogReplay.ts` and
`src-tauri/src/session_log/library.rs`, rather than the on-disk JSONL envelope:

- Output: kind `0`, little-endian `u64` elapsed microseconds, `u32` byte count, bytes.
- Resize: kind `1`, little-endian `u64` elapsed microseconds, `u16` columns, `u16` rows.
- First frame is the initial size. All fixtures use scrollback 12. No input frames.

From the repository root:

```sh
node scripts/terminal-reference/generate.mjs --check
cargo test --workspace --test replay -- --nocapture
cargo run --workspace --example terminal_replay -- session --original
cargo run --workspace --example terminal_replay -- attributes --fast --screenshot target/replay.png
```

`generate.mjs` without `--check` regenerates `.bin`, `.ansi` and `.screen` from
`scripts/terminal-reference/fixtures.mjs`. It requires the installed xterm 6.0.0,
SerializeAddon 0.14.0 and Unicode11Addon 0.9.0; versions are checked. It constructs
Terminal in Node, never calls `open()` and starts no application or DOM. Unicode
11 matches Tauri's `openTerminal.ts`. No dependency is installed or changed.

`.ansi` is SerializeAddon's final serialization (`scrollback: 0`). `.screen`
is the authoritative semantic sidecar: the serializer does not retain OSC 8
destinations. xterm 6's internal extended attributes and OSC link service are
read only to export underline style/color and link metadata absent from the
public buffer API. Package checks protect those version-specific accesses.
The generator never updates `.diff`.

Each `.screen` has grid, zero-based cursor, active screen, then one TSV record per
visible cell: row, column, UTF-8 hex text, width, foreground, background, six
flags (bold/dim/italic/inverse/hidden/strikeout), underline style (0–5), underline
color, UTF-8 hex OSC id and URI. `-` means empty. Empty cells normalize to space;
wide spacers have empty text and width 0. Named ANSI and indexed colors normalize
to `index:N`; RGB stays exact; default stays semantic, independent of themes.
Inactive underline colors normalize to default. Parser replies are concatenated
in order as a final hex record; they are never fed back as screen output.

`.diff` contains every differing record as adjacent `xterm` and `tterm` lines.
Empty files mean equality. These are exact reviewed differences, not counts or
wildcards: any change fails the test, including a fix that makes a known
difference disappear. All production behavior stays untouched. Diagnostic export
uses `TTERM_REPLAY_EXPORT` (prefer an absolute path under this worktree's
`target/`); it never updates checked-in expectations.

| Fixture | Coverage | Different records | Classification |
| --- | --- | --- | --- |
| session | Timestamped progress redraw, resize, fragmented UTF-8 and CSI | 0 | Equal |
| unicode | CJK/fullwidth, accents, Jamo, emoji/VS16/ZWJ/skin/flag, U+1FAE0 | 4 | Unicode version (U+1FAE0 width and resulting cursor/cells) |
| alternate-restored | Main screen and cursor restored after alternate screen | 0 | Equal |
| alternate-active | Alternate screen still active at the end | 0 | Equal |
| scroll-region | Bounded history overflow and partial scrolling region | 0 | Equal |
| margin-motion | CUU/CUD from inside a region without origin mode | 5 | Implementation defect: movement crosses margins |
| attributes | Six flags, ANSI/index/RGB, five underlines, underline RGB, OSC 8 | 0 | Equal |
| charset-replies | UK/DEC mapping, DA1/DA2/DSR/CPR | 2 | UK mapping defect; DA identity values are acceptable behavior differences |

The migration report itemizes these differences and the known Jamo painting
defect. Grid equality does not test font shaping, emoji composition or pixels.
The model test also replays every byte separately to catch parser state loss.

The GPUI example reuses this test decoder/exporter, parses on a worker thread,
then notifies/draws the existing TerminalView. Acknowledgment after each frame
keeps resize ordering deterministic. Original speed uses absolute timestamps
against `Instant`; overdue frames play immediately. Fast speed has no timestamp
waits but still waits for each frame's draw. This is a diagnostic player, not a
throughput benchmark. Screenshot mode verifies the same `.screen`/`.diff` pair
before saving via `Window::render_to_image` and exiting. No PTY, application data
directory or system input is involved; tterm-gpui's main program is unchanged.
