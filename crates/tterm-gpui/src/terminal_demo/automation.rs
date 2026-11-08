//! Development-only loopback automation for the GPUI prototype.
//!
//! This module is compiled only with `--features automation` in a debug
//! build. The protocol is deliberately small: it has no command execution,
//! file reads, or arbitrary paths. Screenshots are returned as PNG bytes.

use std::any::type_name;
use std::fs::{self, OpenOptions};
use std::io::{BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::thread;

use gpui_kit::{AnyWindowHandle, App, Context, Entity, Keystroke};

use super::window::Prototype;

const BIND: &str = "127.0.0.1:0";
const TOKEN_ENV: &str = "TTERM_GPUI_AUTOMATION_TOKEN_FILE";

#[derive(Debug)]
pub(super) struct AutomationResponse {
    pub status: bool,
    pub body: Vec<u8>,
    pub content_type: &'static str,
}

impl AutomationResponse {
    pub(super) fn ok() -> Self {
        Self {
            status: true,
            body: Vec::new(),
            content_type: "text/plain",
        }
    }
}

struct Request {
    command: String,
    reply: Sender<AutomationResponse>,
}

/// Starts the loopback listener and the app-thread request pump.
pub(super) fn start(handle: AnyWindowHandle, prototype: Entity<Prototype>, cx: &mut App) {
    let token = match make_token() {
        Ok(token) => token,
        Err(error) => {
            eprintln!("tterm-gpui automation disabled: {error}");
            return;
        }
    };
    let token_path = token_path(&token);
    if let Err(error) = write_token(&token_path, &token) {
        eprintln!("tterm-gpui automation disabled: {error}");
        return;
    }
    let listener = match TcpListener::bind(BIND) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("tterm-gpui automation disabled: {error}");
            return;
        }
    };
    let address = listener
        .local_addr()
        .map(|address| address.to_string())
        .unwrap_or_else(|_| BIND.to_string());
    println!("tterm-gpui automation listening on {address}");
    println!("tterm-gpui automation token file: {}", token_path.display());
    let (requests, receiver) = async_channel::unbounded::<Request>();
    thread::spawn(move || accept_requests(listener, token, requests));
    cx.spawn(async move |cx| {
        while let Ok(request) = receiver.recv().await {
            let reply = request.reply;
            let response = cx.update(|app| {
                prototype.update(app, |prototype, cx| {
                    handle_request(&request.command, handle, prototype, cx)
                })
            });
            let _ = reply.send(response);
        }
    })
    .detach();
}

fn accept_requests(listener: TcpListener, token: String, requests: async_channel::Sender<Request>) {
    for stream in listener.incoming().flatten() {
        let token = token.clone();
        let requests = requests.clone();
        thread::spawn(move || serve(stream, &token, requests));
    }
}

fn serve(mut stream: TcpStream, token: &str, requests: async_channel::Sender<Request>) {
    let mut input = String::new();
    if BufReader::new(&mut stream)
        .read_to_string(&mut input)
        .is_err()
    {
        return;
    }
    let mut supplied = None;
    let mut command = None;
    for line in input.lines() {
        if let Some(value) = line.strip_prefix("token=") {
            supplied = Some(value);
        } else if let Some(value) = line.strip_prefix("command=") {
            command = Some(value.to_string());
        }
    }
    if supplied != Some(token) {
        let _ = write_response(&mut stream, AutomationResponse::error("invalid token"));
        return;
    }
    let Some(command) = command else {
        let _ = write_response(&mut stream, AutomationResponse::error("missing command"));
        return;
    };
    let (reply, response) = mpsc::channel();
    if requests.send_blocking(Request { command, reply }).is_err() {
        return;
    }
    if let Ok(response) = response.recv() {
        let _ = write_response(&mut stream, response);
    }
}

fn handle_request(
    command: &str,
    handle: AnyWindowHandle,
    prototype: &mut Prototype,
    cx: &mut Context<Prototype>,
) -> AutomationResponse {
    match command {
        "windows" => AutomationResponse::text(format!(
            "window_id={:?}\nfocused={}\nactive={}\nroot={}\n",
            handle.window_id(),
            window_is_focused(handle, cx),
            window_is_active(handle, cx),
            type_name::<Prototype>(),
        )),
        "state" => AutomationResponse::text(prototype.automation_state()),
        "toggle-title" | "action=toggle-title" => {
            prototype.automation_toggle_title(cx);
            AutomationResponse::ok()
        }
        command if command.strip_prefix("key=").is_some() => {
            if !window_is_active(handle, cx) {
                return AutomationResponse::error("prototype window is not active");
            }
            let key = command.strip_prefix("key=").unwrap_or_default().to_string();
            if key.len() > 64 || key.contains(['\r', '\n']) {
                return AutomationResponse::error("invalid keystroke");
            }
            let Some(key) = supported_keystroke(&key) else {
                return AutomationResponse::error("invalid keystroke");
            };
            let Ok(key) = Keystroke::parse(key) else {
                return AutomationResponse::error("invalid keystroke");
            };
            let result = handle.update(cx, |_, window, app| window.dispatch_keystroke(key, app));
            match result {
                Ok(true) => AutomationResponse::ok(),
                Ok(false) => AutomationResponse::error("keystroke was not handled"),
                Err(error) => AutomationResponse::error(error.to_string()),
            }
        }
        "screenshot" => match handle.update(cx, |_, window, _| window.render_to_image()) {
            Ok(Ok(image)) => {
                let mut bytes = Vec::new();
                if image::DynamicImage::ImageRgba8(image)
                    .write_to(
                        &mut std::io::Cursor::new(&mut bytes),
                        image::ImageFormat::Png,
                    )
                    .is_ok()
                {
                    AutomationResponse {
                        status: true,
                        body: bytes,
                        content_type: "image/png",
                    }
                } else {
                    AutomationResponse::error("PNG encoding failed")
                }
            }
            Ok(Err(error)) => AutomationResponse::error(error.to_string()),
            Err(error) => AutomationResponse::error(error.to_string()),
        },
        _ => AutomationResponse::error("unknown command"),
    }
}

fn supported_keystroke(value: &str) -> Option<&'static str> {
    [
        "enter",
        "escape",
        "tab",
        "backspace",
        "up",
        "down",
        "left",
        "right",
        "home",
        "end",
        "pageup",
        "pagedown",
        "ctrl-c",
        "ctrl-d",
        "ctrl-l",
        "ctrl-r",
        "cmd-q",
    ]
    .into_iter()
    .find(|candidate| *candidate == value)
}

fn window_is_active(handle: AnyWindowHandle, cx: &mut Context<Prototype>) -> bool {
    handle
        .update(cx, |_, window, _| window.is_window_active())
        .unwrap_or(false)
}

fn window_is_focused(handle: AnyWindowHandle, cx: &mut Context<Prototype>) -> bool {
    handle
        .update(cx, |_, window, app| window.focused(app).is_some())
        .unwrap_or(false)
}

fn write_response(stream: &mut TcpStream, response: AutomationResponse) -> std::io::Result<()> {
    if response.status {
        writeln!(
            stream,
            "OK {} {}",
            response.content_type,
            response.body.len()
        )?;
    } else {
        writeln!(stream, "ERR {}", response.body.len())?;
    }
    stream.write_all(&response.body)
}

impl AutomationResponse {
    fn text(body: String) -> Self {
        Self {
            status: true,
            body: body.into_bytes(),
            content_type: "text/plain",
        }
    }
    fn error(message: impl Into<String>) -> Self {
        Self {
            status: false,
            body: message.into().into_bytes(),
            content_type: "text/plain",
        }
    }
}

fn token_path(token: &str) -> PathBuf {
    if let Some(path) = std::env::var_os(TOKEN_ENV) {
        return PathBuf::from(path);
    }
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_STATE_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("com.stone.tTerm.gpui-dev")
        .join(format!("automation-{}.token", &token[..16]))
}

fn write_token(path: &PathBuf, token: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
    file.write_all(token.as_bytes())?;
    file.sync_all()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn make_token() -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    #[cfg(windows)]
    {
        use windows::Win32::Security::Cryptography::{
            BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom,
        };
        let status = unsafe { BCryptGenRandom(None, &mut bytes, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
        if status.is_ok() {
            return Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect());
        }
    }
    #[cfg(unix)]
    if let Ok(mut random) = std::fs::File::open("/dev/urandom")
        && random.read_exact(&mut bytes).is_ok()
    {
        return Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect());
    }
    Err("operating system random source unavailable".to_string())
}
