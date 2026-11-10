import http.server, json, os, pathlib, signal, subprocess, threading, time, sys, shutil
root = pathlib.Path.cwd()
phase = sys.argv[1]
identifier = 'com.stone.tTerm.p1-01-' + phase
out = root / 'target/p1-01' / phase
out.mkdir(parents=True, exist_ok=True)
legacy = out / 'empty-xdg'
legacy.mkdir(exist_ok=True)
result = {}
done = threading.Event()
class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        body = b'''<!doctype html><title>P1-01 isolated path check</title><script>
async function check() { try {
 const profiles = await window.__TAURI_INTERNALS__.invoke('list_profiles');
 await fetch('/result', {method:'POST',body:JSON.stringify({profiles})});
 document.body.textContent=JSON.stringify(profiles);
} catch(e) { await fetch('/result',{method:'POST',body:JSON.stringify({error:String(e)})}); }}
window.addEventListener('DOMContentLoaded',check);
</script><body>Checking isolated profiles</body>'''
        self.send_response(200); self.end_headers(); self.wfile.write(body)
    def do_POST(self):
        result.update(json.loads(self.rfile.read(int(self.headers['Content-Length']))))
        self.send_response(200); self.end_headers(); done.set()
    def log_message(self, *args): pass
server = http.server.HTTPServer(('127.0.0.1', 0), Handler)
threading.Thread(target=server.serve_forever, daemon=True).start()
config = out / 'tauri.json'
config.write_text(json.dumps({'identifier':identifier,'productName':'tTerm P1-01 '+phase,'build':{'beforeDevCommand':'','devUrl':f'http://127.0.0.1:{server.server_port}'}}))
env = dict(os.environ, XDG_CONFIG_HOME=str(legacy), CARGO_BUILD_JOBS='3')
log = (out / 'startup.log').open('w')
process = subprocess.Popen(['pnpm','tauri','dev','--no-watch','--no-dev-server','--config',str(config)], env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
try:
    deadline = time.monotonic()+900
    while not done.wait(1):
        if process.poll() is not None: raise RuntimeError('Tauri exited; see startup.log')
        if time.monotonic()>deadline: raise RuntimeError('Timed out awaiting list_profiles')
    if result != {'profiles': []}: raise RuntimeError(f'Isolated profiles were not empty: {result}')
    time.sleep(2)
    bases = [pathlib.Path.home()/'Library'/p/identifier for p in ['Application Support','Caches','Logs','WebKit','HTTPStorages','Saved Application State']]
    listing = {str(p): sorted(str(x.relative_to(p)) + ('/' if x.is_dir() else '') for x in p.rglob('*')) if p.exists() else None for p in bases}
    result.update(identifier=identifier, commit=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(), directories=listing)
    (out/'result.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result,indent=2), flush=True)
finally:
    os.killpg(process.pid, signal.SIGTERM)
    try: process.wait(timeout=15)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL); process.wait()
    server.shutdown(); log.close()
    cleanup=[]
    for part in ['Application Support','Caches','Logs','WebKit','HTTPStorages','Saved Application State']:
        path=pathlib.Path.home()/'Library'/part/identifier
        if path.exists(): shutil.rmtree(path); cleanup.append(str(path))
    for path in [pathlib.Path.home()/'Library/Preferences'/(identifier+'.plist'), pathlib.Path.home()/'Library/Saved Application State'/(identifier+'.savedState')]:
        if path.is_dir(): shutil.rmtree(path); cleanup.append(str(path))
        elif path.exists(): path.unlink(); cleanup.append(str(path))
    (out/'cleanup.json').write_text(json.dumps(cleanup,indent=2)+'\n')
    keyring_cleanup=[]
    for account in ['key::__secret_data_key__', '__probe__']:
        deleted=subprocess.run(['security','delete-generic-password','-s','tterm ('+identifier+')','-a',account],capture_output=True,text=True)
        keyring_cleanup.append({'account':account,'status':deleted.returncode,'output':(deleted.stdout+deleted.stderr).strip()})
    (out/'keyring-cleanup.json').write_text(json.dumps(keyring_cleanup,indent=2)+'\n')
    assert all(not (pathlib.Path.home()/'Library'/part/identifier).exists() for part in ['Application Support','Caches','Logs','WebKit','HTTPStorages'])
