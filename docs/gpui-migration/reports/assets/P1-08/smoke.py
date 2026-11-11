"""Isolated real-IPC smoke; all application processes and task data are cleaned up."""
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
OUT = ROOT / 'target/p1-08-manual'
OUT.mkdir(parents=True, exist_ok=True)
IDENTIFIER = 'com.stone.tTerm.p1-08-after'
NAME = 'tTerm P1-08 After'
RESULT = {'identifier': IDENTIFIER, 'checks': []}
(OUT/'result.json').unlink(missing_ok=True)
FINISH = threading.Event()
PAGE_DONE = threading.Event()
CLIP = OUT / 'clipboard.swift'
CLIP.write_text('''import AppKit
import Foundation
let p = NSPasteboard.general
let args = CommandLine.arguments
if args[1] == "save" {
 let data = (p.pasteboardItems ?? []).map { item in
  Dictionary(uniqueKeysWithValues: item.types.compactMap { type in
   item.data(forType: type).map { (type.rawValue, $0.base64EncodedString()) }
  })
 }
 try JSONSerialization.data(withJSONObject: data).write(to: URL(fileURLWithPath: args[2]))
} else if args[1] == "file" {
 p.clearContents()
 p.writeObjects([NSURL(fileURLWithPath: args[2])])
} else {
 let values = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: args[2]))) as! [[String:String]]
 let items = values.map { value in
  let item = NSPasteboardItem()
  for (type, data) in value { item.setData(Data(base64Encoded: data)!, forType: NSPasteboard.PasteboardType(type)) }
  return item
 }
 p.clearContents()
 p.writeObjects(items)
}
''')
CLIP_BACKUP = OUT / 'clipboard.json'
TEST_FILE = OUT / '中文 file.txt'
TEST_FILE.write_text('P1-08 isolated clipboard fixture\n')

def clipboard(action, path):
    subprocess.run(['swift', str(CLIP), action, str(path)], check=True, capture_output=True, text=True)

PAGE = r'''<!doctype html><meta charset="utf-8"><title>P1-08 isolated verification</title>
<body><h1>P1-08 isolated platform verification</h1><pre id="status"></pre><script>
const invoke=(cmd,args={})=>window.__TAURI_INTERNALS__.invoke(cmd,args);
async function record(name,fn) { try { const value=await fn(); await fetch('/result',{method:'POST',body:JSON.stringify({name,value})}); status.textContent+=name+': '+JSON.stringify(value)+'\n'; return value; } catch(e) { await fetch('/result',{method:'POST',body:JSON.stringify({name,error:String(e)})}); } }
async function run() {
 const profiles=await invoke('list_profiles');
 if(profiles.length!==0) throw Error('isolation failed');
 await record('profiles',()=>profiles);
 await record('clipboard_text',async()=>{await invoke('plugin:clipboard-manager|write_text',{text:'P1-08 中文\nclipboard'}); const text=await invoke('plugin:clipboard-manager|read_text'); if(text!=='P1-08 中文\nclipboard') throw Error('text mismatch'); return text;});
 await record('clipboard_files',async()=>{await fetch('/set-file'); const files=await invoke('read_clipboard_file_paths'); if(files.length!==1||!files[0].endsWith('中文 file.txt')) throw Error(JSON.stringify(files)); return files;});
 await record('applications_icons',async()=>{const c=await invoke('sftp_external_open_with_choices',{fileName:'file.txt'}); const paths=c.apps.slice(0,2).map(x=>x.path); const icons=await invoke('sftp_external_app_icons',{paths}); if(!Object.values(icons).some(x=>x?.startsWith('data:image/png;base64,'))) throw Error('PNG DTO missing'); return {apps:c.apps.length,icons:Object.values(icons).map(x=>x?.slice(0,30))};});
 await record('open_directory',()=>invoke('open_terminal_log_directory'));
 await record('open_url',async()=>{await invoke('plugin:opener|open_url',{url:location.origin+'/opened'}); for(let i=0;i<30;i++){if((await (await fetch('/state')).json()).opened)return 'browser GET observed';await new Promise(r=>setTimeout(r,200));}throw Error('No browser request');});
 const permission=await record('notification_permission',()=>invoke('notification_permission'));
 if(['granted','badgeOff','fallback'].includes(permission)) await record('notification_delivery',()=>invoke('show_notification',{request:{title:'P1-08 isolated notification',subtitle:'Platform smoke',body:'Click to restore the isolated test window.',tabId:'p1-08-smoke',sound:false}}));
 await record('badge_set_clear',async()=>{await invoke('set_unseen_notifications',{count:2});await new Promise(r=>setTimeout(r,500));await invoke('set_unseen_notifications',{count:0});return 'both IPC calls succeeded';});
 await record('tray_rebuild',async()=>{const config=await invoke('load_config');for(const mode of ['quit','tray','ask']){await fetch('/result',{method:'POST',body:JSON.stringify({name:'tray_before_save',value:mode})});await invoke('save_config',{config:{...config,close_behavior:mode}});}return 'quit/tray/ask saved';});
 await record('hide_restore',async()=>{await invoke('resolve_close_request',{action:'tray'});await fetch('/restore');return 'hidden and second instance launched';});
 await fetch('/ready');
 while(!(await (await fetch('/state')).json()).finish) await new Promise(r=>setTimeout(r,500));
 await record('quit_listener',async()=>{const handler=window.__TAURI_INTERNALS__.transformCallback(async e=>{await fetch('/result',{method:'POST',body:JSON.stringify({name:'quit_ack',value:e.event})}); await invoke('ready_to_quit');});await invoke('plugin:event|listen',{event:'app-will-quit',target:{kind:'Any'},handler});await invoke('set_save_before_quit',{enabled:true});return true;});
 await invoke('resolve_close_request',{action:'quit'});
}
window.addEventListener('DOMContentLoaded',()=>run().catch(async e=>{await fetch('/result',{method:'POST',body:JSON.stringify({name:'fatal',error:String(e)})});await fetch('/ready');}));
</script>'''
class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == '/opened':
            RESULT['opened'] = True
            body = b'P1-08 local URL opened successfully.'
        elif self.path == '/state':
            body = json.dumps({'finish': FINISH.is_set(), 'opened': RESULT.get('opened', False)}).encode()
        elif self.path == '/finish':
            FINISH.set()
            body = b'finishing'
        elif self.path == '/ready':
            PAGE_DONE.set()
            body = b'ready'
        elif self.path == '/restore':
            exe = ROOT / f'target/debug/bundle/dev/{NAME}.app/Contents/MacOS/tTerm'
            second = subprocess.run([str(exe)], env=ENV, capture_output=True, text=True, timeout=15)
            RESULT['second_instance'] = {'exit': second.returncode, 'output': second.stdout + second.stderr}
            body = b'restored'
        elif self.path == '/set-file':
            clipboard('file', TEST_FILE)
            body = b'file clipboard set'
        else:
            body = PAGE.encode()
        self.send_response(200)
        self.end_headers()
        self.wfile.write(body)
    def do_POST(self):
        RESULT['checks'].append(json.loads(self.rfile.read(int(self.headers['Content-Length']))))
        (OUT / 'result.json').write_text(json.dumps(RESULT, indent=2, ensure_ascii=False)+'\n')
        self.send_response(200)
        self.end_headers()
    def log_message(self, *args):
        pass

server = http.server.HTTPServer(('127.0.0.1', 0), Handler)
threading.Thread(target=server.serve_forever, daemon=True).start()
(OUT/'server-port').write_text(str(server.server_port))
config = OUT/'tauri.json'
config.write_text(json.dumps({'identifier':IDENTIFIER,'productName':NAME,'build':{'beforeDevCommand':'','devUrl':f'http://127.0.0.1:{server.server_port}'}}))
xdg = OUT/'empty-xdg'
xdg.mkdir(exist_ok=True)
ENV = dict(os.environ, XDG_CONFIG_HOME=str(xdg), CARGO_BUILD_JOBS='3')
clipboard('save', CLIP_BACKUP)
log = (OUT/'startup.log').open('w')
process = subprocess.Popen(['pnpm','tauri','dev','--no-watch','--no-dev-server','--config',str(config)],env=ENV,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
try:
    deadline = time.monotonic()+900
    while not PAGE_DONE.wait(1):
        if process.poll() is not None:
            RESULT['unexpected_exit'] = process.returncode
            raise RuntimeError('Tauri exited: see startup.log')
        if time.monotonic()>deadline: raise RuntimeError('IPC smoke timed out')
    print('IPC ready; inspect target/p1-08-manual/result.json; finish via local /finish', flush=True)
    if not FINISH.wait(240): FINISH.set()
    try:
        process.wait(timeout=25)
        RESULT['process_exit'] = process.returncode
    except subprocess.TimeoutExpired:
        RESULT['process_exit'] = 'timeout'
finally:
    if process.poll() is None:
        os.killpg(process.pid,signal.SIGTERM)
        try: process.wait(timeout=15)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid,signal.SIGKILL)
            process.wait()
    server.shutdown()
    log.close()
    clipboard('restore', CLIP_BACKUP)
    CLIP_BACKUP.unlink()
    RESULT['clipboard_restored'] = True
    cleanup=[]
    for part in ['Application Support','Caches','Logs','WebKit','HTTPStorages','Saved Application State']:
        path=pathlib.Path.home()/'Library'/part/IDENTIFIER
        if path.exists(): shutil.rmtree(path);cleanup.append(str(path))
    for path in [pathlib.Path.home()/'Library/Preferences'/(IDENTIFIER+'.plist'),pathlib.Path.home()/'Library/Saved Application State'/(IDENTIFIER+'.savedState')]:
        if path.is_dir():shutil.rmtree(path);cleanup.append(str(path))
        elif path.exists():path.unlink();cleanup.append(str(path))
    for account in ['key::__secret_data_key__','__probe__']:
        deleted=subprocess.run(['security','delete-generic-password','-s','tterm ('+IDENTIFIER+')','-a',account],capture_output=True,text=True)
        cleanup.append({'keyring_account':account,'status':deleted.returncode})
    bundle=ROOT/f'target/debug/bundle/dev/{NAME}.app'
    if bundle.exists():
        subprocess.run(['/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister','-u',str(bundle)],check=True)
        shutil.rmtree(bundle)
        cleanup.append(str(bundle))
    RESULT['cleanup']=cleanup
    RESULT['commit']=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
    (OUT/'result.json').write_text(json.dumps(RESULT,indent=2,ensure_ascii=False)+'\n')
    print(json.dumps(RESULT,indent=2,ensure_ascii=False),flush=True)
