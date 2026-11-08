// Synthetic P0-11 output only. No session files, application paths or PTYs are read.
const output = (at, text) => ({ at, bytes: Buffer.from(text) })
const resize = (at, cols, rows) => ({ at, cols, rows })
const fixture = (name, rows, frames) => ({
  name,
  cols: 40,
  rows,
  scrollback: 12,
  frames: [resize(0, 40, rows), ...frames],
})

export const fixtures = [
  fixture("session", 8, [
    output(0, "$ synthetic-local-output\r\n"),
    output(100_000, "progress 10%"),
    output(250_000, "\r\x1b[2Kprogress 100%\r\n"),
    resize(400_000, 44, 9),
    // A UTF-8 character and a CSI are deliberately split between frames.
    { at: 450_000, bytes: Buffer.from([0xe4, 0xb8]) },
    { at: 460_000, bytes: Buffer.from([0xad, 0x1b, 0x5b, 0x33]) },
    output(600_000, "32m done\x1b[0m\r\n$ "),
  ]),
  fixture("unicode", 10, [
    output(0, "CJK: 中文，Ａ|\r\nComb: e\u0301 a\u0308|\r\nJamo: \u1112\u1161\u11ab|\r\n"),
    output(80_000, "Emoji: 😀|\r\nVS16: ❤️|\r\nZWJ: 👩‍💻|\r\nSkin: 👍🏽|\r\nFlag: 🇨🇳|\r\nNew: 🫠|"),
  ]),
  fixture("alternate-restored", 8, [
    output(0, "main preserved\x1b[3;5Hcursor"),
    output(100_000, "\x1b[?1049h\x1b[2J\x1b[H\x1b[31mtemporary alternate\x1b[0m"),
    output(200_000, "\x1b[?1049l!"),
  ]),
  fixture("alternate-active", 8, [
    output(0, "hidden main\x1b[?1049h\x1b[2J\x1b[H\x1b[32mvisible alternate\x1b[0m"),
  ]),
  fixture("scroll-region", 8, [
    output(0, Array.from({ length: 24 }, (_, i) => `L${String(i).padStart(2, "0")}\r\n`).join("")),
    output(100_000, "\x1b[2;6r\x1b[6;1Hregion A\r\nregion B\r\nregion C\x1b[r\x1b[8;1Hbottom"),
  ]),
  fixture("margin-motion", 8, [output(0, "\x1b[3;5r\x1b[5;5H\x1b[24AX\x1b[3;10H\x1b[24BY")]),
  fixture("attributes", 8, [
    output(0, "\x1b[1mB\x1b[0;2mD\x1b[0;3mI\x1b[0;7mV\x1b[0;8mH\x1b[0;9mS\x1b[0m\r\n"),
    output(40_000, "\x1b[31;44mANSI\x1b[0m \x1b[38;5;196;48;5;24m256\x1b[0m\r\n"),
    output(80_000, "\x1b[38;2;12;34;56;48;2;65;43;21mRGB\x1b[0m\r\n"),
    output(120_000, "\x1b[4:1m1\x1b[4:2m2\x1b[4:3m3\x1b[4:4m4\x1b[4:5m5\x1b[0m\r\n"),
    output(
      160_000,
      "\x1b[4;58;2;123;45;67mUL\x1b[0m\r\n\x1b]8;id=p0-11;https://example.test/replay\x1b\\link\x1b]8;;\x1b\\ plain"
    ),
  ]),
  fixture("charset-replies", 8, [
    output(0, "UK: \x1b(A#\x1b(B\r\nDEC: \x1b(0lqk\x1b(B\r\n\x1b[c\x1b[>c\x1b[5n\x1b[6n"),
  ]),
]
