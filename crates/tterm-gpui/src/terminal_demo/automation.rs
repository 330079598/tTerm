//! Development-only loopback automation for the GPUI prototype.
//!
//! This module is compiled only with `--features automation` in a debug
//! build. The protocol is deliberately small: it has no command execution,
//! file reads, or arbitrary paths. Screenshots are returned as PNG bytes.

use std::any::type_name;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::Duration;

use gpui_kit::{AnyWindowHandle, App, Entity, EntityId, Keystroke};

use super::window::Prototype;

mod protocol;
use protocol::{Command, MAX_REQUEST_BYTES, parse_request};

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
    command: Command,
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
            let _ = fs::remove_file(&token_path);
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
    let close_path = token_path.clone();
    cx.on_window_closed(move |_, id| {
        if id == handle.window_id() {
            let _ = fs::remove_file(&close_path);
        }
    })
    .detach();
    cx.on_app_quit(move |_| {
        let _ = fs::remove_file(&token_path);
        async {}
    })
    .detach();
    let os = cx.platform_info().os;
    let (requests, receiver) = async_channel::bounded::<Request>(1);
    thread::spawn(move || accept_requests(listener, token, os, requests));
    let prototype = prototype.downgrade();
    cx.spawn(async move |cx| {
        while let Ok(request) = receiver.recv().await {
            let reply = request.reply;
            let response = cx.update(|app| match prototype.upgrade() {
                Some(prototype) => handle_request(request.command, handle, &prototype, app),
                None => AutomationResponse::error("prototype was closed"),
            });
            let _ = reply.send(response);
        }
    })
    .detach();
}

fn accept_requests(
    listener: TcpListener,
    token: String,
    os: gpui_kit::OperatingSystem,
    requests: async_channel::Sender<Request>,
) {
    // One worker bounds memory/thread use even when unauthenticated users connect.
    for stream in listener.incoming().flatten() {
        serve(stream, &token, os, &requests);
    }
}

fn serve(
    mut stream: TcpStream,
    token: &str,
    os: gpui_kit::OperatingSystem,
    requests: &async_channel::Sender<Request>,
) {
    let timeout = Some(Duration::from_secs(5));
    if stream.set_read_timeout(timeout).is_err() || stream.set_write_timeout(timeout).is_err() {
        return;
    }
    let mut input = Vec::new();
    // The sentinel byte distinguishes an exact-limit request from an oversized one.
    if (&mut stream)
        .take((MAX_REQUEST_BYTES + 1) as u64)
        .read_to_end(&mut input)
        .is_err()
    {
        let _ = write_response(
            &mut stream,
            AutomationResponse::error("request read timed out or failed"),
        );
        return;
    }
    let command = match parse_request(&input, token, os) {
        Ok(command) => command,
        Err(error) => {
            let _ = write_response(&mut stream, AutomationResponse::error(error));
            return;
        }
    };
    let (reply, response) = mpsc::channel();
    if requests.try_send(Request { command, reply }).is_err() {
        let _ = write_response(&mut stream, AutomationResponse::error("prototype is busy"));
        return;
    }
    if let Ok(response) = response.recv_timeout(Duration::from_secs(5)) {
        let _ = write_response(&mut stream, response);
    }
}

fn handle_request(
    command: Command,
    handle: AnyWindowHandle,
    prototype: &Entity<Prototype>,
    cx: &mut App,
) -> AutomationResponse {
    match command {
        Command::Windows => AutomationResponse::text(format!(
            "window_id={:?}\nfocused={}\nactive={}\nroot={}\n",
            handle.window_id(),
            window_is_focused(handle, cx),
            window_is_active(handle, cx),
            type_name::<Prototype>(),
        )),
        Command::State => AutomationResponse::text(prototype.read(cx).automation_state()),
        Command::Focus | Command::Close => {
            let result = handle.update(cx, |_, window, _| {
                if matches!(command, Command::Focus) {
                    window.activate_window();
                } else {
                    window.remove_window();
                }
            });
            match result {
                Ok(()) => AutomationResponse::ok(),
                Err(error) => AutomationResponse::error(error.to_string()),
            }
        }
        Command::Key(key) => dispatch_keys(handle, prototype.entity_id(), vec![key], cx),
        Command::Text(text) => {
            let keys = text
                .chars()
                .map(|character| Keystroke {
                    key: character.to_string(),
                    key_char: Some(character.to_string()),
                    ..Default::default()
                })
                .collect();
            dispatch_keys(handle, prototype.entity_id(), keys, cx)
        }
        Command::Screenshot => match handle.update(cx, |_, window, _| window.render_to_image()) {
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
    }
}

fn dispatch_keys(
    handle: AnyWindowHandle,
    prototype_id: EntityId,
    keys: Vec<Keystroke>,
    cx: &mut App,
) -> AutomationResponse {
    // gpui-kit wraps Prototype in its own Root; the captured handle is the
    // exact window that open_window created, rather than the active OS window.
    let result = handle.update(cx, |_, window, app| {
        let matches = window
            .root::<gpui_kit::base::Root>()
            .flatten()
            .is_some_and(|root| root.read(app).view().entity_id() == prototype_id);
        if !matches {
            return Err("target is not the prototype window");
        }
        if !window.is_window_active() || window.focused(app).is_none() {
            return Err("prototype window is not active and focused");
        }
        for key in keys {
            if !window.dispatch_keystroke(key, app) {
                return Err("keystroke was not handled");
            }
        }
        Ok(())
    });
    match result {
        Ok(Ok(())) => AutomationResponse::ok(),
        Ok(Err(error)) => AutomationResponse::error(error),
        Err(error) => AutomationResponse::error(error.to_string()),
    }
}

fn window_is_active(handle: AnyWindowHandle, cx: &mut App) -> bool {
    handle
        .update(cx, |_, window, _| window.is_window_active())
        .unwrap_or(false)
}

fn window_is_focused(handle: AnyWindowHandle, cx: &mut App) -> bool {
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
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    let result = file
        .write_all(token.as_bytes())
        .and_then(|()| file.sync_all());
    drop(file);
    if result.is_err() {
        let _ = fs::remove_file(path);
    }
    result
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
