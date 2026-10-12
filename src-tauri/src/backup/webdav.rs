//! A minimal WebDAV client: just enough to keep backup archives in one remote
//! directory. Every request uses Basic auth, which Jianguoyun, Nextcloud,
//! Synology and Alist all accept.

use percent_encoding::percent_decode_str;
use quick_xml::escape::resolve_predefined_entity;
use quick_xml::events::Event;
use quick_xml::Reader;
use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE, ETAG, IF_MATCH, IF_NONE_MATCH};
use reqwest::{Client, Method, RequestBuilder, Response, StatusCode, Url};
use std::error::Error as _;
use std::time::Duration;
use zeroize::Zeroizing;

const PROPFIND_BODY: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:"><d:prop><d:resourcetype/><d:getcontentlength/><d:getlastmodified/></d:prop></d:propfind>"#;
/// A directory listing is a few hundred bytes per file.
const MAX_LISTING_SIZE: u64 = 8 * 1024 * 1024;
const MAX_DIRECTORY_DEPTH: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DavEntry {
    pub name: String,
    pub is_collection: bool,
    pub size: u64,
    /// Unix milliseconds, when the server reports it.
    pub modified_at: Option<i64>,
}

pub(crate) struct WebDavClient {
    http: Client,
    base: Url,
    username: String,
    password: Zeroizing<String>,
}

impl WebDavClient {
    pub fn new(
        server_url: &str,
        username: &str,
        password: Zeroizing<String>,
        user_agent: &str,
    ) -> Result<Self, String> {
        // reqwest is built with `rustls-no-provider` (matching
        // tauri-plugin-updater) and panics without a process default, so
        // install ring unless another caller already did.
        if rustls::crypto::CryptoProvider::get_default().is_none() {
            let _ = rustls::crypto::ring::default_provider().install_default();
        }
        let http = Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(300))
            .user_agent(user_agent)
            .build()
            .map_err(|error| format!("Failed to create the WebDAV client: {error}"))?;
        Ok(Self {
            http,
            base: parse_server_url(server_url)?,
            username: username.to_string(),
            password,
        })
    }

    /// Checks that the server URL answers and accepts the credentials.
    pub async fn check(&self) -> Result<(), String> {
        match self.propfind(self.base.clone(), "0").await? {
            Some(_) => Ok(()),
            None => Err("The WebDAV server URL was not found (HTTP 404).".to_string()),
        }
    }

    /// Creates the directory and any missing parents.
    pub async fn ensure_directory(&self, directory: &[String]) -> Result<(), String> {
        if directory.is_empty()
            || self
                .propfind(self.directory_url(directory)?, "0")
                .await?
                .is_some()
        {
            return Ok(());
        }
        for depth in 1..=directory.len() {
            let url = self.directory_url(&directory[..depth])?;
            if depth < directory.len() && self.propfind(url.clone(), "0").await?.is_some() {
                continue;
            }
            let response = self.send(self.request(method(b"MKCOL"), url)).await?;
            let status = response.status();
            // 405: the collection already exists.
            if !status.is_success() && status != StatusCode::METHOD_NOT_ALLOWED {
                return Err(status_error("create the remote folder", status));
            }
        }
        Ok(())
    }

    /// Lists the entries directly inside a directory; empty if it does not exist.
    pub async fn list(&self, directory: &[String]) -> Result<Vec<DavEntry>, String> {
        let url = self.directory_url(directory)?;
        match self.propfind(url.clone(), "1").await? {
            Some(body) => parse_multistatus(&body, &url),
            None => Ok(Vec::new()),
        }
    }

    pub async fn put(&self, directory: &[String], name: &str, data: Vec<u8>) -> Result<(), String> {
        let request = self
            .request(Method::PUT, self.file_url(directory, name)?)
            .header(CONTENT_TYPE, "application/octet-stream")
            .header(CONTENT_LENGTH, data.len())
            .body(data);
        let status = self.send(request).await?.status();
        if status.is_success() {
            Ok(())
        } else {
            Err(status_error("upload the backup", status))
        }
    }

    pub async fn get(
        &self,
        directory: &[String],
        name: &str,
        limit: u64,
    ) -> Result<Vec<u8>, String> {
        let response = self
            .send(self.request(Method::GET, self.file_url(directory, name)?))
            .await?;
        let status = response.status();
        if !status.is_success() {
            return Err(status_error("download the backup", status));
        }
        read_limited(
            response,
            limit,
            "The remote backup exceeds the 128 MiB limit.",
        )
        .await
    }

    /// Downloads a file unless its ETag still matches `etag`.
    pub async fn get_if_changed(
        &self,
        directory: &[String],
        name: &str,
        etag: Option<&str>,
        limit: u64,
    ) -> Result<Fetched, String> {
        let mut request = self.request(Method::GET, self.file_url(directory, name)?);
        if let Some(etag) = etag {
            request = request.header(IF_NONE_MATCH, etag);
        }
        let response = self.send(request).await?;
        match response.status() {
            StatusCode::NOT_MODIFIED => Ok(Fetched::NotModified),
            StatusCode::NOT_FOUND => Ok(Fetched::NotFound),
            status if status.is_success() => {
                let etag = etag_of(&response);
                let data =
                    read_limited(response, limit, "The remote sync file is too large.").await?;
                Ok(Fetched::Found { data, etag })
            }
            status => Err(status_error("download the sync file", status)),
        }
    }

    /// Uploads a file only if it still has the ETag it was read with, so
    /// another device's upload in between is not overwritten.
    pub async fn put_if_match(
        &self,
        directory: &[String],
        name: &str,
        data: Vec<u8>,
        etag: Option<&str>,
    ) -> Result<Stored, String> {
        let mut request = self
            .request(Method::PUT, self.file_url(directory, name)?)
            .header(CONTENT_TYPE, "application/octet-stream")
            .header(CONTENT_LENGTH, data.len())
            .body(data);
        // A weak ETag (Apache sends one for a file changed within the last
        // second) never matches `If-Match`, which compares strongly.
        if let Some(etag) = etag.filter(|etag| !etag.starts_with("W/")) {
            request = request.header(IF_MATCH, etag);
        }
        let response = self.send(request).await?;
        match response.status() {
            // 423: another upload of the same file is in progress (rclone).
            StatusCode::PRECONDITION_FAILED | StatusCode::LOCKED => Ok(Stored::Conflict),
            status if status.is_success() => Ok(Stored::Stored {
                etag: etag_of(&response),
            }),
            status => Err(status_error("upload the sync file", status)),
        }
    }

    /// Deletes a file; one that is already gone counts as deleted.
    pub async fn delete(&self, directory: &[String], name: &str) -> Result<(), String> {
        let status = self
            .send(self.request(Method::DELETE, self.file_url(directory, name)?))
            .await?
            .status();
        if status.is_success() || status == StatusCode::NOT_FOUND {
            Ok(())
        } else {
            Err(status_error("delete the backup", status))
        }
    }

    /// Deletes a directory and everything in it; used to clean up live tests.
    #[cfg(test)]
    pub async fn delete_directory(&self, directory: &[String]) -> Result<(), String> {
        let status = self
            .send(self.request(Method::DELETE, self.directory_url(directory)?))
            .await?
            .status();
        if status.is_success() || status == StatusCode::NOT_FOUND {
            Ok(())
        } else {
            Err(status_error("delete the remote folder", status))
        }
    }

    /// Returns the multistatus body, or `None` when the resource does not exist.
    async fn propfind(&self, url: Url, depth: &str) -> Result<Option<String>, String> {
        let request = self
            .request(method(b"PROPFIND"), url)
            .header("Depth", depth)
            .header(CONTENT_TYPE, "application/xml; charset=utf-8")
            .body(PROPFIND_BODY);
        let response = self.send(request).await?;
        match response.status() {
            StatusCode::NOT_FOUND => Ok(None),
            StatusCode::MULTI_STATUS | StatusCode::OK => {
                let body = read_limited(
                    response,
                    MAX_LISTING_SIZE,
                    "The WebDAV directory listing is too large.",
                )
                .await?;
                String::from_utf8(body)
                    .map(Some)
                    .map_err(|_| "The WebDAV server sent an invalid directory listing.".to_string())
            }
            status => Err(status_error("read the remote folder", status)),
        }
    }

    fn request(&self, method: Method, url: Url) -> RequestBuilder {
        self.http
            .request(method, url)
            .basic_auth(&self.username, Some(self.password.as_str()))
    }

    async fn send(&self, request: RequestBuilder) -> Result<Response, String> {
        request.send().await.map_err(request_error)
    }

    fn directory_url(&self, directory: &[String]) -> Result<Url, String> {
        self.url_with(directory, "")
    }

    fn file_url(&self, directory: &[String], name: &str) -> Result<Url, String> {
        self.url_with(directory, name)
    }

    /// The base URL with the directory segments and a last segment appended;
    /// an empty last segment leaves the trailing slash a collection needs.
    fn url_with(&self, directory: &[String], last: &str) -> Result<Url, String> {
        let mut url = self.base.clone();
        url.path_segments_mut()
            .map_err(|_| "WebDAV server URL is invalid.".to_string())?
            .pop_if_empty()
            .extend(directory)
            .push(last);
        Ok(url)
    }
}

pub(crate) enum Fetched {
    NotModified,
    NotFound,
    Found { data: Vec<u8>, etag: Option<String> },
}

pub(crate) enum Stored {
    Stored {
        etag: Option<String>,
    },
    /// The file changed since it was read, or is being written right now.
    Conflict,
}

fn etag_of(response: &Response) -> Option<String> {
    response
        .headers()
        .get(ETAG)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

fn method(name: &'static [u8]) -> Method {
    Method::from_bytes(name).unwrap_or(Method::GET)
}

pub(crate) fn parse_server_url(value: &str) -> Result<Url, String> {
    let url = Url::parse(value.trim()).map_err(|_| "WebDAV server URL is invalid.".to_string())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("WebDAV server URL must start with https:// or http://.".to_string());
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err("WebDAV server URL has no host.".to_string());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(
            "Enter the user name and password in their own fields, not in the URL.".to_string(),
        );
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err("WebDAV server URL cannot contain a query or fragment.".to_string());
    }
    Ok(url)
}

/// Splits a remote directory such as `tTerm/backups` into path segments.
pub(crate) fn split_remote_directory(value: &str) -> Result<Vec<String>, String> {
    let mut segments = Vec::new();
    for segment in value.split('/').map(str::trim).filter(|s| !s.is_empty()) {
        if segment == "."
            || segment == ".."
            || segment.contains('\\')
            || segment.chars().any(char::is_control)
        {
            return Err(format!("Remote folder segment '{segment}' is not allowed."));
        }
        segments.push(segment.to_string());
    }
    if segments.len() > MAX_DIRECTORY_DEPTH {
        return Err("Remote folder is nested too deeply.".to_string());
    }
    Ok(segments)
}

/// Parses a PROPFIND multistatus body into the entries inside `directory`,
/// leaving out the directory itself. Element prefixes vary by server
/// (`d:`, `D:`, `ns0:` or none), so elements are matched by local name.
pub(crate) fn parse_multistatus(body: &str, directory: &Url) -> Result<Vec<DavEntry>, String> {
    #[derive(Default)]
    struct Raw {
        href: String,
        is_collection: bool,
        size: u64,
        modified_at: Option<i64>,
    }

    let invalid = |error: &dyn std::fmt::Display| format!("Invalid WebDAV response: {error}");
    let directory_path = decoded_path(directory.path());
    let mut reader = Reader::from_str(body);
    let mut current: Option<Raw> = None;
    let mut text = String::new();
    let mut entries = Vec::new();
    loop {
        match reader.read_event().map_err(|error| invalid(&error))? {
            Event::Start(element) => {
                if element
                    .local_name()
                    .as_ref()
                    .eq_ignore_ascii_case("response")
                {
                    current = Some(Raw::default());
                }
                text.clear();
            }
            Event::Empty(element) => {
                if element
                    .local_name()
                    .as_ref()
                    .eq_ignore_ascii_case("collection")
                {
                    if let Some(raw) = current.as_mut() {
                        raw.is_collection = true;
                    }
                }
            }
            Event::Text(value) => text.push_str(&value.xml10_content()),
            Event::CData(value) => text.push_str(&value.xml10_content()),
            Event::GeneralRef(reference) => {
                if let Some(ch) = reference.resolve_char_ref().map_err(|e| invalid(&e))? {
                    text.push(ch);
                } else {
                    text.push_str(resolve_predefined_entity(&reference).unwrap_or_default());
                }
            }
            Event::End(element) => {
                let name = element.local_name();
                let name = name.as_ref().to_ascii_lowercase();
                if let Some(raw) = current.as_mut() {
                    match name.as_str() {
                        "href" => raw.href = text.trim().to_string(),
                        "collection" => raw.is_collection = true,
                        "getcontentlength" => raw.size = text.trim().parse().unwrap_or(0),
                        "getlastmodified" => raw.modified_at = parse_http_date(text.trim()),
                        "response" => {
                            if let Some(raw) = current.take() {
                                if let Some(name) =
                                    entry_name(&raw.href, directory, &directory_path)
                                {
                                    entries.push(DavEntry {
                                        name,
                                        is_collection: raw.is_collection,
                                        size: raw.size,
                                        modified_at: raw.modified_at,
                                    });
                                }
                            }
                        }
                        _ => {}
                    }
                }
                text.clear();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(entries)
}

/// The decoded last path segment of `href`, or `None` for the listed
/// directory itself. Hrefs can be absolute paths or full URLs, and servers
/// percent-encode differently, so paths are compared decoded.
fn entry_name(href: &str, directory: &Url, directory_path: &str) -> Option<String> {
    let url = directory.join(href).ok()?;
    let path = decoded_path(url.path());
    if path == directory_path {
        return None;
    }
    let name = path.rsplit('/').next()?.to_string();
    (!name.is_empty()).then_some(name)
}

fn decoded_path(path: &str) -> String {
    percent_decode_str(path)
        .decode_utf8_lossy()
        .trim_end_matches('/')
        .to_string()
}

fn parse_http_date(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc2822(value)
        .ok()
        .map(|date| date.timestamp_millis())
}

async fn read_limited(
    mut response: Response,
    limit: u64,
    too_large: &str,
) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|length| length > limit)
    {
        return Err(too_large.to_string());
    }
    let mut data = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(request_error)? {
        if data.len() as u64 + chunk.len() as u64 > limit {
            return Err(too_large.to_string());
        }
        data.extend_from_slice(&chunk);
    }
    Ok(data)
}

fn request_error(error: reqwest::Error) -> String {
    if error.is_timeout() {
        return "The WebDAV server did not respond in time.".to_string();
    }
    // reqwest's own message rarely names the cause (DNS, TLS, refused).
    let mut message = if error.is_connect() {
        "Could not connect to the WebDAV server".to_string()
    } else {
        "WebDAV request failed".to_string()
    };
    let mut source = error.source();
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    message
}

fn status_error(action: &str, status: StatusCode) -> String {
    match status.as_u16() {
        401 => "The WebDAV server rejected the user name or password. Some services, such as Jianguoyun, require an app password.".to_string(),
        403 => format!("The WebDAV server denied permission to {action}."),
        404 => format!("Could not {action}: the remote path was not found."),
        409 => format!("Could not {action}: the remote folder does not exist."),
        423 => format!("Could not {action}: the remote file is locked."),
        429 | 503 => "The WebDAV server is limiting requests. Try again later.".to_string(),
        507 => "The WebDAV server is out of storage space.".to_string(),
        code => format!("The WebDAV server could not {action} (HTTP {code})."),
    }
}

#[cfg(test)]
mod tests;
