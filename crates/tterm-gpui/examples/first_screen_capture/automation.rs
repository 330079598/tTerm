//! The small loopback client the capture uses to drive the debug app.
//!
//! The production listener serves one command per connection, so every
//! request opens its own stream and reads its own framed reply.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

pub struct Client {
    address: String,
    token: String,
}

impl Client {
    pub fn connect(address: &str, token: String) -> Result<Self, String> {
        Ok(Self {
            address: address.to_string(),
            token,
        })
    }

    pub fn command(&mut self, command: &str) -> Result<Vec<u8>, String> {
        let stream = TcpStream::connect(&self.address)
            .map_err(|error| format!("could not connect to {}: {error}", self.address))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .and_then(|()| stream.set_write_timeout(Some(Duration::from_secs(15))))
            .map_err(|error| format!("could not configure the automation socket: {error}"))?;
        let request = format!("token={}\ncommand={command}\n", self.token);
        let mut reader = BufReader::new(stream);
        reader
            .get_mut()
            .write_all(request.as_bytes())
            .map_err(|error| format!("could not send {command}: {error}"))?;
        reader
            .get_mut()
            .shutdown(std::net::Shutdown::Write)
            .map_err(|error| format!("could not finish {command}: {error}"))?;
        let mut header = String::new();
        reader
            .read_line(&mut header)
            .map_err(|error| format!("could not read the {command} header: {error}"))?;
        let mut parts = header.split_whitespace();
        let status = parts.next().ok_or("the automation reply has no status")?;
        let _content_type = parts
            .next()
            .ok_or("the automation reply has no content type")?;
        let length: usize = parts
            .next()
            .ok_or("the automation reply has no length")?
            .parse()
            .map_err(|error| format!("the automation reply length is invalid: {error}"))?;
        let mut body = vec![0; length];
        reader
            .read_exact(&mut body)
            .map_err(|error| format!("could not read the {command} body: {error}"))?;
        if status == "OK" {
            Ok(body)
        } else {
            Err(String::from_utf8_lossy(&body).into_owned())
        }
    }

    pub fn text(&mut self, command: &str) -> Result<String, String> {
        let body = self.command(command)?;
        String::from_utf8(body).map_err(|error| format!("{command} returned non-UTF-8: {error}"))
    }

    pub fn screenshot(&mut self) -> Result<Vec<u8>, String> {
        self.command("screenshot")
    }

    pub fn wait_for(&mut self, needle: &str) -> Result<String, String> {
        for _ in 0..80 {
            match self.text("capture-state") {
                Ok(state) if state.contains(needle) => return Ok(state),
                Ok(_) => {}
                Err(error) => return Err(error),
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Err(format!("the capture never reported {needle}"))
    }
}

/// Reads a PNG body into RGBA and checks it covers the requested crop.
///
/// macOS can report one extra physical row for a window created at an exact
/// logical height. The crop removes it, so the visible 800x600 area still
/// matches the WebKit viewport.
pub fn png_at_least(bytes: &[u8], expected: (u32, u32)) -> Result<image::RgbaImage, String> {
    let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map_err(|error| format!("the screenshot is not a PNG: {error}"))?
        .to_rgba8();
    if image.width() != expected.0 || image.height() < expected.1 {
        return Err(format!(
            "the screenshot is {}x{}, expected at least {}x{}; the seeded window geometry was not applied",
            image.width(),
            image.height(),
            expected.0,
            expected.1
        ));
    }
    Ok(image)
}

/// Keeps `height` rows starting at `y`, for the settings content crop.
pub fn crop_at(image: &image::RgbaImage, y: u32, width: u32, height: u32) -> image::RgbaImage {
    image::imageops::crop_imm(image, 0, y, width, height).to_image()
}
