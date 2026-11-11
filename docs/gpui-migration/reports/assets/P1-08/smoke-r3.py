"""Verify tray toggles and missing-HOME startup using only an isolated app identity."""
import http.server
import json
import os
import pathlib
import shutil
import signal
import subprocess
import threading
import time

ROOT = pathlib.Path.cwd()
OUT = ROOT / 'target/p1-08-r3-manual'
OUT.mkdir(parents=True, exist_ok=True)
IDENTIFIER = 'com.stone.tTerm.p1-08-r3'
NAME = 'tTerm P1-08 R3'
RESULT = {'identifier': IDENTIFIER, 'checks': []}
READY = threading.Event()
PHASE = 'normal'
PAGE = r'''<!doctype html><meta charset="utf-8"><title>P1-08 R3 isolated smoke</title><body><pre id="result"></pre><script>
const invoke=(cmd,args={})=>window.__TAURI_INTERNALS__.invoke(cmd,args);
async function report(name,value,error){await fetch('/result',{method:'POST',body:JSON.stringify({name,value,error})});}
async function run(){
 const profiles=await invoke('list_profiles'); if(profiles.length)throw Error('isolated profiles not empty');
 await report('profiles',profiles);
 const config=await invoke('load_config');
 for(const mode of ['quit','tray','quit','tray']){
  await invoke('save_config',{config:{...config,close_behavior:mode}});
  await report('tray_saved',mode);
 }
 // Queue two requests without yielding to a visual inspection or a user interaction.
 await Promise.all(['quit','tray'].map(mode=>invoke('save_config',{config:{...config,close_behavior:mode}})));
 await report('concurrent_pair','both completed (completion order not asserted)');
 await invoke('save_config',{config:{...config,close_behavior:'tray'}});
 for(const command of ['shell_command_status','folder_menu_status']){
  try{await report(command,await invoke(command));}catch(error){await report(command,null,String(error));}
 }
 const handler=window.__TAURI_INTERNALS__.transformCallback(async e=>{await report('quit_event',e.event);await invoke('ready_to_quit');});
 await invoke('plugin:event|listen',{event:'app-will-quit',target:{kind:'Any'},handler});
 await invoke('set_save_before_quit',{enabled:true});
 await report('ready_to_exit',true);
 await invoke('resolve_close_request',{action:'quit'});
}
window.addEventListener('DOMContentLoaded',()=>run().catch(async e=>{await report('fatal',null,String(e));await fetch('/failed');}));
</script>'''
class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == '/failed': READY.set()
        self.send_response(200)
        self.end_headers()
        self.wfile.write(PAGE.encode())
    def do_POST(self):
        record=json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        record['phase']=PHASE
        RESULT['checks'].append(record)
        (OUT/'result.json').write_text(json.dumps(RESULT,indent=2,ensure_ascii=False)+'\n')
        self.send_response(200)
        self.end_headers()
    def log_message(self,*args): pass

server=http.server.HTTPServer(('127.0.0.1',0),Handler)
threading.Thread(target=server.serve_forever,daemon=True).start()
config=OUT/'tauri.json'
config.write_text(json.dumps({'identifier':IDENTIFIER,'productName':NAME,'build':{'beforeDevCommand':'','devUrl':f'http://127.0.0.1:{server.server_port}'}}))
xdg=OUT/'empty-xdg'
xdg.mkdir(exist_ok=True)
missing_home=OUT/'this-home-does-not-exist'
assert not missing_home.exists()
env=dict(os.environ,XDG_CONFIG_HOME=str(xdg),CARGO_BUILD_JOBS='3')
process=None
bundle=ROOT/f'target/debug/bundle/dev/{NAME}.app'
try:
    for phase in ['normal','missing-home']:
        PHASE=phase
        READY.clear()
        phase_env=dict(env)
        if phase == 'normal':
            command=['pnpm','tauri','dev','--no-watch','--no-dev-server','--config',str(config)]
        else:
            # Override HOME only in the app child, after rustup/cargo have finished.
            phase_env['HOME']=str(missing_home)
            command=[str(bundle/'Contents/MacOS/tTerm')]
            RESULT['missing_home_before_launch']=not missing_home.exists()
        with (OUT/f'{phase}-startup.txt').open('w') as log:
            process=subprocess.Popen(command,env=phase_env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
            deadline=time.monotonic()+900
            while process.poll() is None:
                if READY.wait(0.5):raise RuntimeError('Smoke page failed: see result.json')
                if time.monotonic()>deadline:raise RuntimeError('Timed out: see startup log')
            RESULT[phase+'_exit']=process.returncode
            if process.returncode:raise RuntimeError(f'{phase} exited {process.returncode}')
        process=None
finally:
    if process is not None:
        try:os.killpg(process.pid,signal.SIGTERM)
        except ProcessLookupError:pass
        try:process.wait(timeout=15)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid,signal.SIGKILL)
            process.wait()
    server.shutdown()
    cleanup=[]
    for part in ['Application Support','Caches','Logs','WebKit','HTTPStorages','Saved Application State']:
        path=pathlib.Path.home()/'Library'/part/IDENTIFIER
        if path.exists():shutil.rmtree(path);cleanup.append(str(path))
    for path in [pathlib.Path.home()/'Library/Preferences'/(IDENTIFIER+'.plist'),pathlib.Path.home()/'Library/Saved Application State'/(IDENTIFIER+'.savedState')]:
        if path.is_dir():shutil.rmtree(path);cleanup.append(str(path))
        elif path.exists():path.unlink();cleanup.append(str(path))
    if missing_home.exists():shutil.rmtree(missing_home);cleanup.append(str(missing_home))
    for account in ['key::__secret_data_key__','__probe__']:
        result=subprocess.run(['security','delete-'+'generic-password','-s','tterm ('+IDENTIFIER+')','-a',account],capture_output=True,text=True)
        cleanup.append({'account':account,'keyring_exit':result.returncode})
    if bundle.exists():
        subprocess.run(['/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister','-u',str(bundle)],check=True)
        shutil.rmtree(bundle)
        cleanup.append(str(bundle))
    RESULT['cleanup']=cleanup
    RESULT['commit']=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
    (OUT/'result.json').write_text(json.dumps(RESULT,indent=2,ensure_ascii=False)+'\n')
    print(json.dumps(RESULT,indent=2,ensure_ascii=False),flush=True)
