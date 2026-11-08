"""Drive only the GPUI prototype: type an OSC title command, read it, capture, exit."""

from __future__ import annotations

import argparse
import os
import pathlib
import queue
import shutil
import socket
import subprocess
import sys
import threading
import time


def request(port: int, token: str, command: str, deadline: float) -> bytes:
    def remaining() -> float:
        value = deadline - time.monotonic()
        if value <= 0:
            raise TimeoutError("120 second automation deadline expired")
        return value

    with socket.create_connection(("127.0.0.1", port), timeout=remaining()) as connection:
        connection.sendall(f"token={token}\ncommand={command}\n".encode())
        connection.shutdown(socket.SHUT_WR)
        with connection.makefile("rb") as response:
            connection.settimeout(remaining())
            header = response.readline(256).decode().strip().split()
            if not header or header[0] not in {"OK", "ERR"}:
                raise RuntimeError(f"invalid response: {header!r}")
            length = int(header[-1])
            if not 0 <= length <= 64 * 1024 * 1024:
                raise RuntimeError("invalid response size")
            connection.settimeout(remaining())
            body = response.read(length)
            if len(body) != length:
                raise RuntimeError("truncated response")
            if header[0] == "ERR":
                raise RuntimeError(body.decode())
            return body


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=pathlib.Path, default=pathlib.Path("target/gpui-automation.png"))
    parser.add_argument("--timeout", type=float, default=120)
    args = parser.parse_args()
    if args.timeout <= 0:
        parser.error("--timeout must be positive")
    root = pathlib.Path(__file__).resolve().parents[1]
    executable = root / "target" / "debug" / ("tterm-gpui.exe" if os.name == "nt" else "tterm-gpui")
    shell = (shutil.which("pwsh") or shutil.which("powershell")) if os.name == "nt" else shutil.which("bash")
    if not shell:
        raise RuntimeError("PowerShell (Windows) or bash (Unix) is required")
    arguments = [shell, "-NoLogo", "-NoProfile"] if os.name == "nt" else [shell, "--noprofile", "--norc"]
    env = os.environ.copy()
    env.pop("TTERM_GPUI_AUTOMATION_TOKEN_FILE", None)
    deadline = time.monotonic() + args.timeout
    process = subprocess.Popen([str(executable), "--exec", *arguments], cwd=root, env=env,
                               stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
                               encoding="utf-8", errors="replace", bufsize=1)
    print(f"prototype-pid: {process.pid}", flush=True)
    expired = threading.Event()

    def expire() -> None:
        expired.set()
        if process.poll() is None:
            process.terminate()

    watchdog = threading.Timer(args.timeout, expire)
    watchdog.daemon = True
    watchdog.start()
    token_file = None
    try:
        lines: queue.Queue[str] = queue.Queue()

        def read_output() -> None:
            assert process.stdout is not None
            for line in process.stdout:
                lines.put(line)

        threading.Thread(target=read_output, daemon=True).start()
        port = None
        while time.monotonic() < deadline:
            if process.poll() is not None:
                raise RuntimeError(f"prototype exited: {process.returncode}; timeout={expired.is_set()}")
            try:
                line = lines.get(timeout=max(0.01, min(0.25, deadline - time.monotonic())))
            except queue.Empty:
                continue
            print(line, end="", flush=True)
            if "automation disabled:" in line:
                raise RuntimeError(line.strip())
            if "automation listening on 127.0.0.1:" in line:
                port = int(line.rsplit(":", 1)[1])
            if "automation token file: " in line:
                token_file = pathlib.Path(line.split("automation token file: ", 1)[1].strip())
            if port is not None and token_file is not None:
                break
        if port is None or token_file is None:
            raise TimeoutError("automation endpoint was not ready")
        if os.name == "nt":
            subprocess.run(["icacls", str(token_file)], check=True, timeout=5)
        token = token_file.read_text(encoding="ascii").strip()
        while time.monotonic() < deadline:
            request(port, token, "action=focus", deadline)
            time.sleep(0.1)
            windows = request(port, token, "windows", deadline).decode()
            if "active=true" in windows and "focused=true" in windows:
                break
        else:
            raise TimeoutError("Windows did not activate the prototype window")
        print(windows, flush=True)
        before = request(port, token, "state", deadline).decode()
        print(f"state-before: {before}", flush=True)
        expected = "tTerm P0-23 OSC title"
        command = "$Host.UI.RawUI.WindowTitle = 'tTerm P0-23 OSC title'" if os.name == "nt" else "printf '\\033]0;tTerm P0-23 OSC title\\007'"
        request(port, token, f"text={command}", deadline)
        request(port, token, "key=enter", deadline)
        while time.monotonic() < deadline:
            after = request(port, token, "state", deadline).decode()
            if after == f"title={expected}":
                break
            time.sleep(0.05)
        else:
            raise TimeoutError("the shell never set the expected OSC title")
        assert after != before
        print(f"state-after: {after}", flush=True)
        image = request(port, token, "screenshot", deadline)
        if not image.startswith(b"\x89PNG\r\n\x1a\n"):
            raise RuntimeError("screenshot is not PNG")
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_bytes(image)
        print(f"screenshot: {args.output} ({len(image)} bytes)", flush=True)
        request(port, token, "text=exit", deadline)
        request(port, token, "key=enter", deadline)
        # GPUI leaves an exited terminal open. Close only its captured window.
        request(port, token, "action=close", deadline)
        process.wait(timeout=max(0.01, deadline - time.monotonic()))
        if token_file.exists():
            raise RuntimeError("token file survived normal window close")
        print("token-cleanup: removed by prototype", flush=True)
        return 0
    finally:
        watchdog.cancel()
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
        print(f"prototype-cleanup: exited ({process.returncode})", flush=True)
        if token_file is not None:
            token_file.unlink(missing_ok=True)


if __name__ == "__main__":
    sys.exit(main())
