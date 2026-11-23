"""Capture the production GPUI settings route at the 800x600 reference size.

The script starts the automation-enabled debug binary in a throwaway instance,
creates its SQLite database once, puts the reference window size into the
saved GPUI window state, then drives the real window with the loopback
automation endpoint. Every screenshot is the window image with the 36pt title
bar cropped off, so the settings content area is 800x600 CSS pixels at the
reference 2x scale.

Run from the repository root after building the automation binary:

  cargo build --workspace --bin tterm-gpui --features tterm-gpui/automation
  python3 scripts/migration/capture-settings-gpui.py --output target/parity/settings-gpui
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import queue
import runpy
import secrets
import shutil
import sqlite3
import subprocess
import sys
import threading
import time

from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parents[2]
REQUEST = runpy.run_path(str(ROOT / "scripts" / "gpui-automation.py"))["request"]
VIEWPORT = (800, 600)
TITLE_BAR = 36


def start_instance(executable: pathlib.Path, root: pathlib.Path, instance: int, timeout: float):
    temp = root / "tmp"
    temp.mkdir(parents=True, exist_ok=True)
    token_file = root / "automation.token"
    token_file.unlink(missing_ok=True)
    if os.name == "nt":
        shell = shutil.which("pwsh") or shutil.which("powershell")
        arguments = [shell, "-NoLogo", "-NoProfile"]
    else:
        shell = shutil.which("bash")
        arguments = [shell, "--noprofile", "--norc"]
    if not shell:
        raise RuntimeError("a shell is required to start the prototype")
    env = os.environ.copy()
    env["TTERM_GPUI_CHECK_INSTANCE"] = str(instance)
    env["TTERM_GPUI_AUTOMATION_TOKEN_FILE"] = str(token_file)
    env["TMPDIR"] = str(temp)
    env["TEMP"] = str(temp)
    env["TMP"] = str(temp)
    process = subprocess.Popen(
        [str(executable), "--exec", *arguments],
        cwd=ROOT,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        encoding="utf-8",
        errors="replace",
        bufsize=1,
    )
    lines: queue.Queue[str] = queue.Queue()
    output: list[str] = []

    def read_output() -> None:
        assert process.stdout is not None
        for line in process.stdout:
            lines.put(line)

    threading.Thread(target=read_output, daemon=True).start()
    deadline = time.monotonic() + timeout
    port = None
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(
                "prototype exited before automation was ready:\n" + "".join(output[-20:])
            )
        try:
            line = lines.get(timeout=max(0.01, min(0.25, deadline - time.monotonic())))
        except queue.Empty:
            continue
        output.append(line)
        print(line, end="", flush=True)
        if "automation disabled:" in line:
            raise RuntimeError(line.strip())
        if "automation listening on 127.0.0.1:" in line:
            port = int(line.rsplit(":", 1)[1])
        if port is not None and token_file.exists():
            break
    if port is None or not token_file.exists():
        raise TimeoutError("automation endpoint was not ready")
    token = token_file.read_text(encoding="ascii").strip()
    return process, port, token, token_file, temp


def wait_for_focus(port: int, token: str, deadline: float) -> None:
    while time.monotonic() < deadline:
        try:
            REQUEST(port, token, "action=focus", deadline)
            windows = REQUEST(port, token, "windows", deadline).decode()
        except OSError:
            time.sleep(0.05)
            continue
        if "active=true" in windows and "focused=true" in windows:
            return
        time.sleep(0.05)
    raise TimeoutError("the prototype window did not activate")


def stop(process: subprocess.Popen, token_file: pathlib.Path) -> None:
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)
    token_file.unlink(missing_ok=True)


def database(root: pathlib.Path, instance: int) -> pathlib.Path:
    return root / "tmp" / f"com.stone.tTerm.gpui-dev.check-{instance}" / "config" / "tterm.db"


def set_reference_window(root: pathlib.Path, instance: int) -> None:
    path = database(root, instance)
    deadline = time.monotonic() + 10
    while not path.exists():
        if time.monotonic() > deadline:
            raise TimeoutError(f"the database was not created: {path}")
        time.sleep(0.05)
    state = {
        "bounds": {"x": 120.0, "y": 120.0, "width": VIEWPORT[0], "height": VIEWPORT[1] + TITLE_BAR},
        "maximized": False,
    }
    connection = sqlite3.connect(path)
    try:
        connection.execute(
            "CREATE TABLE IF NOT EXISTS app_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)"
        )
        connection.execute(
            "INSERT INTO app_meta (key, value) VALUES (?, ?) "
            "ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            ("gpui_window_state", json.dumps(state, separators=(",", ":"))),
        )
        connection.commit()
    finally:
        connection.close()


def save_crop(payload: bytes, image_path: pathlib.Path) -> dict[str, object]:
    import io

    image = Image.open(io.BytesIO(payload))
    scale = image.width / VIEWPORT[0]
    width = round(VIEWPORT[0] * scale)
    height = round(VIEWPORT[1] * scale)
    top = round(TITLE_BAR * scale)
    if image.width < width or image.height < top + height:
        raise RuntimeError(f"unexpected screenshot size {image.size} at scale {scale}")
    page = image.crop((0, top, width, top + height))
    page.save(image_path)
    return {"width": page.width, "height": page.height}


def sha(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def drive(port: int, token: str, output: pathlib.Path, deadline: float) -> dict[str, object]:
    screens: dict[str, object] = {}

    def shot(name: str) -> None:
        print(f"settings capture: screenshot {name}", flush=True)
        # The reference hides the caret. The focused input draws one for the
        # first 500 ms after the last edit, so wait for its hidden phase.
        time.sleep(0.75)
        payload = REQUEST(port, token, "screenshot", deadline)
        if not payload.startswith(b"\x89PNG\r\n\x1a\n"):
            raise RuntimeError("screenshot is not PNG")
        path = output / f"{name}.png"
        screens[name] = save_crop(payload, path)

    wait_for_focus(port, token, deadline)

    def send(command: str) -> None:
        # Re-activate before every keystroke batch. A language repaint or a
        # screenshot must not leave the window behind another application.
        print(f"settings capture: {command}", flush=True)
        for attempt in range(5):
            REQUEST(port, token, "action=focus", deadline)
            try:
                REQUEST(port, token, command, deadline)
                return
            except RuntimeError as error:
                if "not active and focused" not in str(error) or attempt == 4:
                    raise
                time.sleep(0.2)

    modifier = "cmd" if sys.platform == "darwin" else "ctrl"
    send(f"key={modifier}-,")
    send("text=scrollback")
    shot("settings.search.scrollback")
    send("key=enter")
    shot("settings.terminal.scrollback")
    send("text=zzz")
    shot("settings.search.empty")
    # Clear the query, close the route, switch language and open again. The
    # reopen re-focuses the real search field, so a language repaint cannot
    # leave the next keystrokes on the shell behind the route.
    send("key=escape")
    send("key=escape")
    send("action=language=zh")
    time.sleep(0.25)
    send(f"key={modifier}-,")
    send("text=回滚")
    shot("settings.search.scrollback-zh")
    send("key=enter")
    shot("settings.terminal.scrollback-zh")
    return screens


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=pathlib.Path, default=pathlib.Path("target/parity/settings-gpui"))
    parser.add_argument("--executable", type=pathlib.Path)
    parser.add_argument("--timeout", type=float, default=120)
    args = parser.parse_args()
    if args.timeout <= 0:
        parser.error("--timeout must be positive")
    executable = (
        args.executable.resolve()
        if args.executable
        else ROOT / "target" / "debug" / ("tterm-gpui.exe" if os.name == "nt" else "tterm-gpui")
    )
    if not executable.exists():
        raise SystemExit(
            f"missing {executable}; build it with "
            "cargo build --workspace --bin tterm-gpui --features tterm-gpui/automation"
        )
    args.output.mkdir(parents=True, exist_ok=True)
    instance = secrets.randbits(31)
    root = args.output / "instance"
    shutil.rmtree(root, ignore_errors=True)
    root.mkdir(parents=True)
    deadline = time.monotonic() + args.timeout

    # The first run creates the isolated database; the saved geometry is then
    # written before the second run so the page opens at the exact reference
    # size without resizing a live window.
    process, _, _, token_file, _ = start_instance(executable, root, instance, args.timeout)
    stop(process, token_file)
    set_reference_window(root, instance)

    process, port, token, token_file, _ = start_instance(
        executable, root, instance, args.timeout
    )
    try:
        screens = drive(port, token, args.output, deadline)
        REQUEST(port, token, "action=close", deadline)
        process.wait(timeout=max(0.01, deadline - time.monotonic()))
    finally:
        stop(process, token_file)
    chinese = {"settings.search.scrollback-zh", "settings.terminal.scrollback-zh"}
    manifest = {
        "version": 1,
        "theme": "dark",
        "uiStyle": "classic",
        "uiScalePercent": 100,
        "viewport": {"width": VIEWPORT[0], "height": VIEWPORT[1], "deviceScaleFactor": 2},
        "titleBarHeight": TITLE_BAR,
        "instance": instance,
        "screens": {
            name: {
                "image": f"{name}.png",
                "width": size["width"],
                "height": size["height"],
                "locale": "zh-CN" if name in chinese else "en-US",
                "sha256": sha(args.output / f"{name}.png"),
            }
            for name, size in screens.items()
        },
    }
    (args.output / "capture.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(json.dumps(manifest, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
