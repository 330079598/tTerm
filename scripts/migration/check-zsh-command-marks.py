#!/usr/bin/env python3
"""Check production command marks on a real macOS PTY, without a user profile."""

import os
from pathlib import Path
import pty
import re
import select
import shutil
import signal
import sys
import tempfile
import time


HOOKS = (Path(__file__).resolve().parents[2] / "crates/tterm-core/src/terminal"
         / "shell_integration/zsh")
MARK = re.compile(rb"\x1b\]((?:133|633);[^\x1b\x07]*)(?:\x1b\\|\x07)")


def until_prompt(master, deadline):
    data = bytearray()
    while True:
        remaining = deadline - time.monotonic()
        if remaining <= 0 or not select.select([master], [], [], remaining)[0]:
            raise TimeoutError("zsh did not produce its next prompt mark within ten seconds")
        chunk = os.read(master, 4096)
        if not chunk:
            raise RuntimeError("zsh exited before its next prompt mark")
        data.extend(chunk)
        if len(data) > 65536:
            raise RuntimeError("unexpected output volume before the next prompt")
        marks = MARK.findall(data)
        if b"133;A" in marks:
            return marks


def check(root):
    home = root / "home"
    home.mkdir()
    hooks = root / "hooks"
    hooks.mkdir()
    for name in ("zshenv", "zshrc"):
        shutil.copyfile(HOOKS / name, hooks / ("." + name))
    env = {"HOME": str(home), "ZDOTDIR": str(hooks), "PATH": "/usr/bin:/bin",
           "TERM": "xterm", "LC_ALL": "C", "TTERM_COMMAND_MARKS": "1"}
    pid, master = pty.fork()
    if pid == 0:
        try:
            os.chdir(home)
            os.execve("/bin/zsh", ["/bin/zsh", "-di"], env)
        finally:
            os._exit(127)
    try:
        deadline = time.monotonic() + 10
        until_prompt(master, deadline)
        for command, status in ((b"true", b"0"), (b"false", b"1")):
            os.write(master, command + b"\r")
            marks = until_prompt(master, deadline)
            expected = [b"633;E;" + command, b"133;C", b"133;D;" + status, b"133;A"]
            if marks != expected:
                raise RuntimeError(f"{command!r}: expected {expected!r}, received {marks!r}")
            print(" -> ".join(mark.decode("ascii") for mark in marks))
    finally:
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        os.close(master)
        os.waitpid(pid, 0)


if __name__ == "__main__":
    if sys.platform != "darwin" or os.path.lexists("/etc/zshenv"):
        sys.exit("Requires macOS /bin/zsh with no unconditional /etc/zshenv startup file")
    with tempfile.TemporaryDirectory(prefix="tterm-zsh-marks-") as directory:
        check(Path(directory))
