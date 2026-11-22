"""Exercise GPUI workspace persistence across two isolated Windows processes."""

import contextlib
import json
import os
import pathlib
import queue
import runpy
import secrets
import sqlite3
import subprocess
import tempfile
import threading
import time


ROOT = pathlib.Path(__file__).resolve().parents[2]
REQUEST = runpy.run_path(str(ROOT / 'scripts/gpui-automation.py'))['request']


@contextlib.contextmanager
def instance(environment, directory):
    executable = ROOT / 'target/debug/tterm-gpui.exe'
    deadline = time.monotonic() + 90
    with subprocess.Popen([str(executable)], cwd=ROOT, env=environment,
                          stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                          encoding='utf-8', errors='replace') as process:
        try:
            lines = queue.Queue()

            def read_output():
                for line in process.stdout:
                    lines.put(line.strip())
                lines.put(None)

            threading.Thread(target=read_output, daemon=True).start()
            port = token = None
            while port is None or token is None:
                line = lines.get(timeout=max(0.1, deadline - time.monotonic()))
                if line is None:
                    raise RuntimeError('The isolated window exited before automation was ready')
                if 'automation listening on 127.0.0.1:' in line:
                    port = int(line.rsplit(':', 1)[1])
                if 'automation token file: ' in line:
                    token_path = pathlib.Path(line.split('automation token file: ', 1)[1]).resolve()
                    if not token_path.is_relative_to(directory):
                        raise ValueError('Refusing to read an automation token outside the test directory')
                    token = token_path.read_text(encoding='ascii').strip()

            def send(command):
                return REQUEST(port, token, command, time.monotonic() + 15)

            send('action=focus')
            yield send
            send('action=close')
            if process.wait(timeout=30) != 0:
                raise RuntimeError('Workspace did not exit cleanly')
        finally:
            if process.poll() is None:
                process.kill()


def snapshot(directory):
    databases = list(directory.rglob('tterm.db'))
    if len(databases) != 1:
        raise AssertionError(f'Expected one isolated database, found {len(databases)}')
    with contextlib.closing(sqlite3.connect(databases[0].as_uri() + '?mode=ro', uri=True)) as db:
        row = db.execute("SELECT value FROM app_meta WHERE key = 'session'").fetchone()
    if not row:
        raise AssertionError('Shutdown did not persist the workspace')
    return json.loads(row[0])


def main():
    if os.name != 'nt':
        raise SystemExit('This native smoke test currently drives the Windows keymap.')
    with tempfile.TemporaryDirectory(prefix='workspace-restart-', dir=ROOT / 'target') as temp:
        directory = pathlib.Path(temp).resolve()
        env = {**os.environ, 'TEMP': temp, 'TMP': temp, 'TMPDIR': temp,
               'TTERM_GPUI_CHECK_INSTANCE': str(secrets.randbits(32))}
        env['TTERM_GPUI_AUTOMATION_TOKEN_FILE'] = str(directory / 'automation.token')
        with instance(env, directory) as send:
            send('key=ctrl-\\')
            send('key=ctrl-alt-enter')
        before = snapshot(directory)
        original = [tab['id'] for tab in before['tabs']]
        assert len(original) == len(set(original)) == 2, 'Split did not create two distinct tabs'
        group, = before['layout']['groups']
        assert group['root']['type'] == 'split' and group['zoomedTabId'] == before['active_tab_id']
        with instance(env, directory) as send:
            send('key=ctrl-shift-t')
        after = snapshot(directory)
        ids = [tab['id'] for tab in after['tabs']]
        assert ids[:2] == original and len(set(ids)) == len(ids) == 3
        assert group in after['layout']['groups'], 'Restored group lost focus, zoom or geometry'
        assert after['active_tab_id'] == ids[2], 'The restored window did not open a new tab'
        print('PASS: two clean exits; stable IDs, split geometry, zoom and active tab survived restart.')


if __name__ == '__main__':
    main()
