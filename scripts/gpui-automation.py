"""Run the P0-23 GPUI automation smoke test on the local machine."""

from __future__ import annotations

import argparse
import os
import pathlib
import queue
import socket
import subprocess
import sys
import tempfile
import threading
import time


def request(host: str, port: int, token: str, command: str, timeout: float) -> tuple[bool, bytes]:
    with socket.create_connection((host, port), timeout=timeout) as connection:
        connection.settimeout(timeout)
        connection.sendall(f"token={token}\ncommand={command}\n".encode())
        connection.shutdown(socket.SHUT_WR)
        header = b""
        while not header.endswith(b"\n"):
            chunk = connection.recv(1)
            if not chunk:
                raise RuntimeError("automation endpoint closed before its response")
            header += chunk
        parts = header.decode().strip().split()
        if len(parts) < 2 or parts[0] not in {"OK", "ERR"}:
            raise RuntimeError(f"invalid automation response: {header!r}")
        length = int(parts[-1])
        body = b""
        while len(body) < length:
            body += connection.recv(length - len(body))
        return parts[0] == "OK", body


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=pathlib.Path, default=pathlib.Path("target/gpui-automation.png"))
    args = parser.parse_args()
    token_file = pathlib.Path(tempfile.gettempdir()) / f"tterm-gpui-{os.getpid()}.token"
    process = None
    deadline = time.monotonic() + 120
    try:
        token_file.unlink(missing_ok=True)
        env = os.environ.copy()
        env["TTERM_GPUI_AUTOMATION_TOKEN_FILE"] = str(token_file)
        executable = pathlib.Path(__file__).resolve().parents[1] / "target" / "debug" / "tterm-gpui.exe"
        if not executable.exists():
            raise FileNotFoundError(f"build the automation binary first: {executable}")
        process = subprocess.Popen(
            [str(executable)],
            cwd=pathlib.Path(__file__).resolve().parents[1],
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
        )
        lines: queue.Queue[str] = queue.Queue()
        assert process.stdout is not None
        threading.Thread(
            target=lambda: [lines.put(line) for line in process.stdout],
            daemon=True,
        ).start()
        port = None
        while time.monotonic() < deadline:
            if process.poll() is not None:
                raise RuntimeError(f"GPUI exited with status {process.returncode}")
            try:
                line = lines.get(timeout=min(1, deadline - time.monotonic()))
            except queue.Empty:
                continue
            print(line, end="")
            if "automation listening on 127.0.0.1:" in line:
                port = int(line.rsplit(":", 1)[1])
                break
        if port is None:
            raise TimeoutError("timed out waiting for the automation endpoint")
        remaining = max(1.0, deadline - time.monotonic())
        for _ in range(100):
            if token_file.exists():
                break
            time.sleep(0.05)
        token = token_file.read_text(encoding="ascii").strip()
        ok, state = request("127.0.0.1", port, token, "state", remaining)
        if not ok:
            raise RuntimeError(state.decode())
        print(f"state-before: {state.decode()}")
        remaining = max(1.0, deadline - time.monotonic())
        ok, _ = request("127.0.0.1", port, token, "action=toggle-title", remaining)
        if not ok:
            raise RuntimeError("toggle-title failed")
        remaining = max(1.0, deadline - time.monotonic())
        ok, state = request("127.0.0.1", port, token, "state", remaining)
        if not ok:
            raise RuntimeError(state.decode())
        print(f"state-after: {state.decode()}")
        args.output.parent.mkdir(parents=True, exist_ok=True)
        remaining = max(1.0, deadline - time.monotonic())
        ok, image = request("127.0.0.1", port, token, "screenshot", remaining)
        if not ok:
            raise RuntimeError(image.decode())
        args.output.write_bytes(image)
        print(f"screenshot: {args.output} ({len(image)} bytes)")
        return 0
    finally:
        if process is not None and process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
        token_file.unlink(missing_ok=True)


if __name__ == "__main__":
    sys.exit(main())
