#!/usr/bin/env python3
"""Probe terminal input protocols from Linux, WSL, or an SSH session.

Usage examples:
  python3 terminal-probe.py --mouse 1000
  python3 terminal-probe.py --mouse 1002,1006
  python3 terminal-probe.py --alternate-screen
  python3 terminal-probe.py --bracketed-paste
  python3 terminal-probe.py --application-cursor
  python3 terminal-probe.py --focus
  python3 terminal-probe.py --unicode

The probe enables the requested mode, prints every received byte as hex and
readable text, and restores terminal modes when it receives Ctrl+C (byte
0x03). It is intentionally Linux-only; run it through WSL or on a Linux/SSH
host. Mouse input, paste, cursor keys, and focus changes must be performed by
a person.
"""

from __future__ import annotations

import argparse
import os
import select
import sys
import termios
import time
import tty
import unicodedata
from contextlib import contextmanager


ESC = b"\x1b"


def write(data: bytes) -> None:
    os.write(sys.stdout.fileno(), data)


def readable(data: bytes) -> str:
    return "".join(chr(byte) if 0x20 <= byte < 0x7f else "." for byte in data)


def report(data: bytes) -> None:
    print(f"RX {data.hex(' ')} | {readable(data)!r}", flush=True)


def modifier_text(button: int) -> str:
    names = []
    if button & 4:
        names.append("shift")
    if button & 8:
        names.append("alt")
    if button & 16:
        names.append("ctrl")
    return "+".join(names) if names else "none"


def button_text(button: int, release: bool) -> str:
    if button & 64:
        return "wheel-down" if button & 1 else "wheel-up"
    if button & 32:
        return "motion"
    base = button & 3
    if release or base == 3:
        return "release"
    return {0: "left", 1: "middle", 2: "right"}.get(base, f"button-{base}")


class MouseDecoder:
    """Decode SGR (1006) or legacy X10 mouse reports from a byte stream."""

    def __init__(self, sgr: bool) -> None:
        self.sgr = sgr
        self.buffer = bytearray()

    def feed(self, data: bytes) -> None:
        self.buffer.extend(data)
        while True:
            event = self._take_event()
            if event is None:
                return
            raw, button, x, y, release = event
            report(raw)
            print(
                f"MOUSE button={button_text(button, release)} modifiers={modifier_text(button)} "
                f"x={x} y={y} action={'release' if release else 'press'}",
                flush=True,
            )

    def _take_event(self) -> tuple[bytes, int, int, int, bool] | None:
        prefix = b"\x1b[M"
        if self.sgr:
            start = self.buffer.find(b"\x1b[<")
            if start < 0:
                if len(self.buffer) > 2:
                    del self.buffer[:-2]
                return None
            if start:
                del self.buffer[:start]
            end = next((index for index, value in enumerate(self.buffer[3:], 3) if value in (ord("M"), ord("m"))), None)
            if end is None:
                return None
            raw = bytes(self.buffer[: end + 1])
            try:
                fields = raw[3:-1].decode("ascii").split(";")
                button, x, y = (int(value) for value in fields)
            except (ValueError, UnicodeDecodeError):
                del self.buffer[:1]
                return None
            del self.buffer[: end + 1]
            return raw, button, x, y, raw[-1:] == b"m"

        start = self.buffer.find(prefix)
        if start < 0:
            if len(self.buffer) > len(prefix) - 1:
                del self.buffer[: -(len(prefix) - 1)]
            return None
        if start:
            del self.buffer[:start]
        if len(self.buffer) < 6:
            return None
        raw = bytes(self.buffer[:6])
        button, x, y = (value - 32 for value in raw[3:6])
        del self.buffer[:6]
        return raw, button, x, y, (button & 3) == 3


def set_mode(mode: str, enabled: bool) -> None:
    suffix = "h" if enabled else "l"
    write(f"\x1b[?{mode}{suffix}".encode())


def restore(modes: list[str], alternate: bool) -> None:
    if alternate:
        write(b"\x1b[?1049l")
    for mode in reversed(modes):
        set_mode(mode, False)
    write(b"\x1b[?25h\x1b[0m\r\n")


@contextmanager
def raw_terminal():
    fd = sys.stdin.fileno()
    saved = termios.tcgetattr(fd)
    tty.setraw(fd)
    current = termios.tcgetattr(fd)
    current[1] |= termios.OPOST | termios.ONLCR
    termios.tcsetattr(fd, termios.TCSADRAIN, current)
    try:
        yield
    finally:
        termios.tcsetattr(fd, termios.TCSADRAIN, saved)


def unicode_sample() -> None:
    target_column = 40
    rows = [
        "CJK: 中文 日本語 한국어",
        "Fullwidth: ＡＢＣ！？，．（）【】",
        "Combining: e\u0301 a\u0308 한글한",
        "Emoji: 🙂 🚀 ☕️",
        "ZWJ: 🧑\u200d💻 👩\u200d🚀",
        "Skin tone: 👍🏽 🙏🏿",
        "Flags: 🇺🇸 🇨🇳 🇪🇺",
        "Box: ┌─┬─┐ ││ │ └─┴─┘ ╭─╮╰─╯",
        "Tab: A\tB\tC",
    ]
    print("Unicode width sample (the ruler is fixed at column 40)", flush=True)
    print("         1         2         3         4", flush=True)
    print("1234567890123456789012345678901234567890", flush=True)
    for row in rows:
        width = terminal_width(row)
        print(f"{row}{' ' * max(1, target_column - width - 1)}|", flush=True)


def terminal_width(text: str) -> int:
    """Return the probe's expected terminal cell width for its fixed samples."""
    width = 0
    regional_pending = False
    zwj_pending = False
    for char in text.expandtabs(8):
        codepoint = ord(char)
        if zwj_pending or unicodedata.combining(char) or codepoint in (0xFE0E, 0xFE0F) or 0x1F3FB <= codepoint <= 0x1F3FF:
            zwj_pending = char == "\u200d"
            continue
        if char == "\u200d":
            zwj_pending = True
            continue
        if 0x1F1E6 <= codepoint <= 0x1F1FF:
            width += 0 if regional_pending else 2
            regional_pending = not regional_pending
            continue
        regional_pending = False
        if 0x1160 <= codepoint <= 0x11FF:
            continue
        width += 2 if unicodedata.east_asian_width(char) in ("W", "F") else 1
    return width


def alternate_screen() -> None:
    print("MAIN SCREEN BEFORE ALTERNATE", flush=True)
    print("This line must be restored after leaving the alternate screen.", flush=True)
    time.sleep(3)
    write(b"\x1b[?1049h\x1b[2J\x1b[H")
    print("ALTERNATE SCREEN", flush=True)
    for index in range(1, 16):
        print(f"alternate row {index:02d} 0123456789 abcdefghijklmnopqrstuvwxyz", flush=True)
    time.sleep(5)
    write(b"\x1b[?1049l")
    print("RETURNED TO MAIN SCREEN", flush=True)
    time.sleep(3)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mouse", metavar="MODES", help="comma-separated modes such as 1000 or 1002,1006")
    parser.add_argument("--alternate-screen", action="store_true")
    parser.add_argument("--bracketed-paste", action="store_true")
    parser.add_argument("--application-cursor", action="store_true")
    parser.add_argument("--focus", action="store_true")
    parser.add_argument("--unicode", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    modes: list[str] = []
    if args.mouse:
        modes.extend(args.mouse.split(","))
    if args.bracketed_paste:
        modes.append("2004")
    if args.application_cursor:
        modes.append("1")
    if args.focus:
        modes.append("1004")
    if not any((args.mouse, args.alternate_screen, args.bracketed_paste, args.application_cursor, args.focus, args.unicode)):
        raise SystemExit("choose a probe mode; see --help")

    with raw_terminal():
        mouse_decoder = MouseDecoder("1006" in modes) if args.mouse else None
        try:
            for mode in modes:
                set_mode(mode, True)
            print(f"Probe modes enabled: {', '.join(modes) or 'none'}", flush=True)
            if args.mouse:
                print("Perform mouse actions manually; bytes will appear below.", flush=True)
            if args.bracketed_paste:
                print("Paste text manually; expect ESC[200~ and ESC[201~ delimiters.", flush=True)
            if args.application_cursor:
                print("Press cursor keys manually; DECCKM is enabled.", flush=True)
            if args.focus:
                print("Move focus away and back manually; expect ESC[I and ESC[O.", flush=True)
            if args.alternate_screen:
                alternate_screen()
            if args.unicode:
                unicode_sample()
            if args.alternate_screen or args.unicode:
                print("Press Ctrl+C to finish.", flush=True)
            while True:
                ready, _, _ = select.select([sys.stdin], [], [], 0.5)
                if ready:
                    data = os.read(sys.stdin.fileno(), 4096)
                    if not data:
                        break
                    if b"\x03" in data:
                        before_ctrl_c, _, _ = data.partition(b"\x03")
                        if before_ctrl_c:
                            if mouse_decoder:
                                mouse_decoder.feed(before_ctrl_c)
                            else:
                                report(before_ctrl_c)
                        print("\r\nProbe interrupted; restoring modes.", flush=True)
                        break
                    if mouse_decoder:
                        mouse_decoder.feed(data)
                    else:
                        report(data)
        finally:
            restore(modes, args.alternate_screen)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
