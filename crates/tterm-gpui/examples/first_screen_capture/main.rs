//! Captures the first screen from the real GPUI window for both themes.
//!
//! This is a debug-only, automation-feature command. Each surface launches a
//! real native window in its own numbered instance, seeded with synthetic
//! configuration, groups and SSH profile metadata. The child runs a local
//! script that prints fixed bytes; it never opens an SSH connection and no
//! user configuration, database, credential or desktop is read.
//!
//! ```sh
//! cargo run -p tterm-gpui --example first_screen_capture \
//!   --features tterm-gpui/automation -- \
//!   --theme dark --output target/parity/first-screen/gpui/dark
//! ```

mod automation;
mod seed;

use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use automation::Client;
use serde_json::json;

const CHILD_MARKER: &str = "TTERM_GPUI_FIRST_SCREEN_CHILD";
const INSTANCE_ENV: &str = "TTERM_GPUI_CHECK_INSTANCE";
const TOKEN_ENV: &str = "TTERM_GPUI_AUTOMATION_TOKEN_FILE";
const VIEWPORT: (u32, u32) = (800, 600);
const SCALE: u32 = 2;
const TITLEBAR_HEIGHT: u32 = 36;

struct Options {
    theme: String,
    output: PathBuf,
}

fn main() {
    if std::env::var_os(CHILD_MARKER).is_some() {
        tterm_gpui::terminal_demo::run();
        return;
    }
    if let Err(error) = run() {
        eprintln!("first_screen_capture: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut options = parse_args()?;
    options.output = if options.output.is_absolute() {
        options.output
    } else {
        std::env::current_dir()
            .map_err(|error| format!("could not read the working directory: {error}"))?
            .join(options.output)
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    fs::create_dir_all(&options.output)
        .map_err(|error| format!("could not create {}: {error}", options.output.display()))?;
    let instance_base = options.output.join("instances");
    if instance_base.exists() {
        fs::remove_dir_all(&instance_base)
            .map_err(|error| format!("could not clear {}: {error}", instance_base.display()))?;
    }
    fs::create_dir_all(&instance_base)
        .map_err(|error| format!("could not create {}: {error}", instance_base.display()))?;

    let first = next_number();
    let second = first.wrapping_add(1);
    capture_workspace_and_profiles(&options, &root, &instance_base, first)?;
    capture_settings(&options, &root, &instance_base, second)?;
    let _ = fs::remove_dir(&instance_base);

    let manifest = json!({
        "version": 1,
        "method": "real native GPUI window via the debug automation endpoint; production workspace, saved-connections and settings views",
        "isolation": "numbered instance under the capture output, synthetic config/SSH metadata, fixed local script, no SSH connection or credential read",
        "theme": options.theme,
        "language": "en",
        "uiStyle": "classic",
        "uiScalePercent": 100,
        "viewport": { "width": VIEWPORT.0, "height": VIEWPORT.1, "deviceScaleFactor": SCALE },
        "windowHeightForSettings": VIEWPORT.1 + TITLEBAR_HEIGHT,
        "crop": "workspace/profiles: full window; settings: rows 72..1272 of the client area, which is the page below the 36 logical px title bar",
        "files": [
            "workspace.first-screen.png",
            "profiles.list.first-screen.png",
            "settings.terminal.font.png"
        ]
    });
    fs::write(
        options.output.join("gpui.json"),
        serde_json::to_string_pretty(&manifest).map_err(|error| error.to_string())? + "\n",
    )
    .map_err(|error| format!("could not write gpui.json: {error}"))?;
    println!(
        "first_screen_capture: {} surfaces in {}",
        3,
        options.output.display()
    );
    Ok(())
}

fn parse_args() -> Result<Options, String> {
    let mut theme = None;
    let mut output = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--theme" => theme = arguments.next(),
            "--output" => output = arguments.next().map(PathBuf::from),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let theme = theme.ok_or("pass --theme dark or --theme light")?;
    if theme != "dark" && theme != "light" {
        return Err(format!("unknown theme {theme}; expected dark or light"));
    }
    let output = output.ok_or("pass --output DIR")?;
    Ok(Options { theme, output })
}

fn next_number() -> u32 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    (millis % 3_000_000_000) as u32 + 1
}

fn capture_workspace_and_profiles(
    options: &Options,
    root: &Path,
    base: &Path,
    number: u32,
) -> Result<(), String> {
    let instance = seed::create(base, number, &options.theme, VIEWPORT.0, VIEWPORT.1, root)?;
    let mut session = AppSession::start(options, base, &instance)?;
    session.client.wait_for("settings_open=false")?;
    std::thread::sleep(Duration::from_millis(500));
    save_png(
        &session.client.screenshot()?,
        &options.output.join("workspace.first-screen.png"),
        (VIEWPORT.0 * SCALE, VIEWPORT.1 * SCALE),
    )?;

    session.client.command("action=profiles")?;
    session.client.wait_for("profiles_count=4")?;
    session.client.wait_for("profiles_loading=false")?;
    std::thread::sleep(Duration::from_millis(150));
    save_png(
        &session.client.screenshot()?,
        &options.output.join("profiles.list.first-screen.png"),
        (VIEWPORT.0 * SCALE, VIEWPORT.1 * SCALE),
    )?;
    session.finish()
}

fn capture_settings(
    options: &Options,
    root: &Path,
    base: &Path,
    number: u32,
) -> Result<(), String> {
    let height = VIEWPORT.1 + TITLEBAR_HEIGHT;
    let instance = seed::create(base, number, &options.theme, VIEWPORT.0, height, root)?;
    let mut session = AppSession::start(options, base, &instance)?;
    session.client.wait_for("settings_open=false")?;
    session.client.command("action=settings")?;
    session.client.wait_for("settings_open=true")?;
    session.client.wait_for("settings_fonts_loaded=true")?;
    std::thread::sleep(Duration::from_millis(100));
    let image = automation::png_at_least(
        &session.client.screenshot()?,
        (VIEWPORT.0 * SCALE, height * SCALE),
    )?;
    let page = automation::crop_at(
        &image,
        TITLEBAR_HEIGHT * SCALE,
        VIEWPORT.0 * SCALE,
        VIEWPORT.1 * SCALE,
    );
    page.save(&options.output.join("settings.terminal.font.png"))
        .map_err(|error| format!("could not save the settings page: {error}"))?;
    session.finish()
}

fn save_png(bytes: &[u8], path: &Path, expected: (u32, u32)) -> Result<(), String> {
    let image = automation::png_at_least(bytes, expected)?;
    automation::crop_at(&image, 0, expected.0, expected.1)
        .save(path)
        .map_err(|error| format!("could not save {}: {error}", path.display()))
}

struct AppSession {
    child: Child,
    client: Client,
    logs: Arc<Mutex<VecDeque<String>>>,
    instance_root: PathBuf,
    log_path: PathBuf,
}

impl AppSession {
    fn start(options: &Options, base: &Path, instance: &seed::Instance) -> Result<Self, String> {
        let token_path = options
            .output
            .join(format!("automation-token-{}.txt", instance.number));
        let _ = fs::remove_file(&token_path);
        let log_path = options.output.join(format!("app-{}.log", instance.number));
        let executable = std::env::current_exe()
            .map_err(|error| format!("could not find the capture binary: {error}"))?;
        let mut child = Command::new(executable)
            .arg("--exec")
            .arg(&instance.script)
            .env(CHILD_MARKER, "1")
            .env(INSTANCE_ENV, instance.number.to_string())
            .env(TOKEN_ENV, &token_path)
            .env("TMPDIR", base)
            .env("TMP", base)
            .env("TEMP", base)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("could not start the native window: {error}"))?;
        let logs = Arc::new(Mutex::new(VecDeque::new()));
        let (lines, receiver) = std::sync::mpsc::channel();
        for pipe in [
            child
                .stdout
                .take()
                .map(|pipe| Box::new(pipe) as Box<dyn std::io::Read + Send>),
            child
                .stderr
                .take()
                .map(|pipe| Box::new(pipe) as Box<dyn std::io::Read + Send>),
        ]
        .into_iter()
        .flatten()
        {
            let lines = lines.clone();
            let logs = Arc::clone(&logs);
            std::thread::spawn(move || {
                for line in BufReader::new(pipe).lines().map_while(Result::ok) {
                    if let Ok(mut logs) = logs.lock() {
                        logs.push_back(line.clone());
                        if logs.len() > 200 {
                            logs.pop_front();
                        }
                    }
                    let _ = lines.send(line);
                }
            });
        }
        let address = wait_for_address(&mut child, &receiver, &logs)?;
        wait_for_token(&token_path)?;
        let token = fs::read_to_string(&token_path)
            .map_err(|error| format!("could not read {}: {error}", token_path.display()))?
            .trim()
            .to_string();
        let client = Client::connect(&address, token)?;
        let session = Self {
            child,
            client,
            logs,
            instance_root: instance.root.clone(),
            log_path,
        };
        session.write_log()?;
        Ok(session)
    }

    fn finish(mut self) -> Result<(), String> {
        let _ = self.client.command("action=close");
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            match self.child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(error) => return Err(format!("could not wait for the native window: {error}")),
            }
        }
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        self.write_log()?;
        if std::env::var_os("TTERM_GPUI_FIRST_SCREEN_KEEP_INSTANCE").is_none() {
            let _ = fs::remove_dir_all(&self.instance_root);
        } else {
            println!(
                "first_screen_capture: kept {}",
                self.instance_root.display()
            );
        }
        Ok(())
    }

    fn write_log(&self) -> Result<(), String> {
        let text = self
            .logs
            .lock()
            .map(|logs| logs.iter().cloned().collect::<Vec<_>>().join("\n"))
            .unwrap_or_default();
        fs::write(&self.log_path, text + "\n")
            .map_err(|error| format!("could not write {}: {error}", self.log_path.display()))
    }
}

impl Drop for AppSession {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn wait_for_address(
    child: &mut Child,
    lines: &std::sync::mpsc::Receiver<String>,
    logs: &Arc<Mutex<VecDeque<String>>>,
) -> Result<String, String> {
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if let Ok(line) = lines.recv_timeout(Duration::from_millis(200))
            && let Some(address) = line.split("automation listening on ").nth(1)
        {
            return Ok(address.trim().to_string());
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("could not poll the native window: {error}"))?
        {
            let tail = logs
                .lock()
                .map(|logs| logs.iter().cloned().collect::<Vec<_>>().join("\n"))
                .unwrap_or_default();
            return Err(format!("the native window exited with {status}:\n{tail}"));
        }
    }
    Err("the native window did not open its automation endpoint".to_string())
}

fn wait_for_token(path: &Path) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if fs::read_to_string(path).is_ok_and(|token| !token.trim().is_empty()) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    Err(format!(
        "the automation token never appeared at {}",
        path.display()
    ))
}
