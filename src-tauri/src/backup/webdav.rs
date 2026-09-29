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
mod tests {
    use super::*;

    fn client(base: &str) -> WebDavClient {
        WebDavClient::new(
            base,
            "user",
            Zeroizing::new("secret".to_string()),
            "tTerm-test",
        )
        .unwrap()
    }

    #[test]
    fn builds_encoded_urls_under_the_base_path() {
        let dir = vec!["tTerm".to_string(), "备份 1".to_string()];
        for base in [
            "https://dav.example.com/dav",
            "https://dav.example.com/dav/",
        ] {
            let client = client(base);
            assert_eq!(
                client.directory_url(&dir).unwrap().as_str(),
                "https://dav.example.com/dav/tTerm/%E5%A4%87%E4%BB%BD%201/"
            );
            assert_eq!(
                client.file_url(&dir, "a#b.tterm-backup").unwrap().as_str(),
                "https://dav.example.com/dav/tTerm/%E5%A4%87%E4%BB%BD%201/a%23b.tterm-backup"
            );
        }
        assert_eq!(
            client("https://nas.local:5006")
                .directory_url(&[])
                .unwrap()
                .as_str(),
            "https://nas.local:5006/"
        );
    }

    #[test]
    fn validates_server_urls() {
        assert!(parse_server_url("https://dav.jianguoyun.com/dav/").is_ok());
        assert!(parse_server_url("http://192.168.1.2:5005").is_ok());
        assert!(parse_server_url("ftp://example.com").is_err());
        assert!(parse_server_url("https://user:pw@example.com/").is_err());
        assert!(parse_server_url("https://example.com/dav?x=1").is_err());
        assert!(parse_server_url("not a url").is_err());
    }

    #[test]
    fn splits_remote_directories() {
        assert_eq!(
            split_remote_directory(" /tTerm//backups/ ").unwrap(),
            ["tTerm", "backups"]
        );
        assert!(split_remote_directory("").unwrap().is_empty());
        assert!(split_remote_directory("a/../b").is_err());
        assert!(split_remote_directory("a\\b").is_err());
    }

    #[test]
    fn parses_jianguoyun_style_listing() {
        let body = r#"<?xml version="1.0" encoding="UTF-8"?>
<d:multistatus xmlns:d="DAV:">
  <d:response>
    <d:href>/dav/tTerm/</d:href>
    <d:propstat><d:prop>
      <d:resourcetype><d:collection/></d:resourcetype>
      <d:getlastmodified>Mon, 28 Sep 2026 02:00:00 GMT</d:getlastmodified>
    </d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
  </d:response>
  <d:response>
    <d:href>/dav/tTerm/tterm-mac-20260928-020000.tterm-backup</d:href>
    <d:propstat><d:prop>
      <d:resourcetype/>
      <d:getcontentlength>2048</d:getcontentlength>
      <d:getlastmodified>Mon, 28 Sep 2026 02:00:00 GMT</d:getlastmodified>
    </d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>
  </d:response>
  <d:response>
    <d:href>/dav/tTerm/sub/</d:href>
    <d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop>
    <d:status>HTTP/1.1 200 OK</d:status></d:propstat>
  </d:response>
</d:multistatus>"#;
        let directory = Url::parse("https://dav.jianguoyun.com/dav/tTerm/").unwrap();
        let entries = parse_multistatus(body, &directory).unwrap();
        assert_eq!(
            entries,
            vec![
                DavEntry {
                    name: "tterm-mac-20260928-020000.tterm-backup".to_string(),
                    is_collection: false,
                    size: 2048,
                    modified_at: Some(1_790_560_800_000),
                },
                DavEntry {
                    name: "sub".to_string(),
                    is_collection: true,
                    size: 0,
                    modified_at: None,
                },
            ]
        );
    }

    #[test]
    fn parses_full_url_hrefs_other_prefixes_and_encoding() {
        // Nextcloud-like: upper-case prefix, full URL hrefs, a 404 propstat,
        // an entity reference and a differently encoded directory path.
        let body = r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:" xmlns:s="http://sabredav.org/ns">
 <D:response>
  <D:href>https://cloud.example.com/remote.php/dav/files/me/t%20Term/</D:href>
  <D:propstat><D:prop><D:resourcetype><D:collection/></D:resourcetype></D:prop>
  <D:status>HTTP/1.1 200 OK</D:status></D:propstat>
 </D:response>
 <D:response>
  <D:href>/remote.php/dav/files/me/t Term/a&amp;b%E5%A4%87.tterm-backup</D:href>
  <D:propstat><D:prop><D:resourcetype/><D:getcontentlength>10</D:getcontentlength></D:prop>
  <D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  <D:propstat><D:prop><D:getlastmodified/></D:prop>
  <D:status>HTTP/1.1 404 Not Found</D:status></D:propstat>
 </D:response>
</D:multistatus>"#;
        let directory =
            Url::parse("https://cloud.example.com/remote.php/dav/files/me/t%20Term/").unwrap();
        let entries = parse_multistatus(body, &directory).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "a&b备.tterm-backup");
        assert_eq!(entries[0].size, 10);
        assert!(!entries[0].is_collection);
        assert_eq!(entries[0].modified_at, None);
    }

    /// Runs against a real server:
    /// `TTERM_WEBDAV_URL=... TTERM_WEBDAV_USER=... TTERM_WEBDAV_PASSWORD=... cargo test live_server -- --ignored`
    #[tokio::test]
    #[ignore]
    async fn live_server_round_trip() {
        let env = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"));
        let client = WebDavClient::new(
            &env("TTERM_WEBDAV_URL"),
            &env("TTERM_WEBDAV_USER"),
            Zeroizing::new(env("TTERM_WEBDAV_PASSWORD")),
            "tTerm-test",
        )
        .unwrap();
        client.check().await.unwrap();
        let root = format!("tterm-live-{}", std::process::id());
        let directory = vec![root.clone(), "备份 dir".to_string()];
        assert!(client.list(&directory).await.unwrap().is_empty());
        client.ensure_directory(&directory).await.unwrap();
        // Existing directories are fine.
        client.ensure_directory(&directory).await.unwrap();

        let name = "tterm-a&b 备份-20260929-000000.tterm-backup";
        let data: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        client.put(&directory, name, data.clone()).await.unwrap();
        let entries = client.list(&directory).await.unwrap();
        assert_eq!(entries.len(), 1, "{entries:?}");
        assert_eq!(entries[0].name, name);
        assert_eq!(entries[0].size, data.len() as u64);
        assert!(!entries[0].is_collection);
        assert!(entries[0].modified_at.is_some());
        assert_eq!(client.get(&directory, name, 1 << 20).await.unwrap(), data);
        assert!(client.get(&directory, name, 1_000).await.is_err());

        client.delete(&directory, name).await.unwrap();
        client.delete(&directory, name).await.unwrap();
        assert!(client.list(&directory).await.unwrap().is_empty());
        assert!(client.get(&directory, name, 1 << 20).await.is_err());

        let wrong = WebDavClient::new(
            &env("TTERM_WEBDAV_URL"),
            &env("TTERM_WEBDAV_USER"),
            Zeroizing::new("wrong-password".to_string()),
            "tTerm-test",
        )
        .unwrap();
        let error = wrong.check().await.unwrap_err();
        assert!(
            error.contains("rejected the user name or password"),
            "{error}"
        );

        client.delete_directory(&[root]).await.unwrap();
    }

    /// Conditional requests the sync relies on:
    /// `TTERM_WEBDAV_URL=... TTERM_WEBDAV_USER=... TTERM_WEBDAV_PASSWORD=... cargo test live_conditional -- --ignored`
    #[tokio::test]
    #[ignore]
    async fn live_conditional_requests() {
        let env = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"));
        let client = WebDavClient::new(
            &env("TTERM_WEBDAV_URL"),
            &env("TTERM_WEBDAV_USER"),
            Zeroizing::new(env("TTERM_WEBDAV_PASSWORD")),
            "tTerm-test",
        )
        .unwrap();
        let directory = vec![format!("tterm-cond-{}", std::process::id())];
        client.ensure_directory(&directory).await.unwrap();
        let name = "tterm-sync.enc";
        assert!(matches!(
            client
                .get_if_changed(&directory, name, None, 1 << 20)
                .await
                .unwrap(),
            Fetched::NotFound
        ));

        let Stored::Stored { .. } = client
            .put_if_match(&directory, name, b"one".to_vec(), None)
            .await
            .unwrap()
        else {
            panic!("first upload conflicted");
        };
        let Fetched::Found { data, etag } = client
            .get_if_changed(&directory, name, None, 1 << 20)
            .await
            .unwrap()
        else {
            panic!("uploaded file not found");
        };
        assert_eq!(data, b"one");
        // Servers differ here; the sync works either way, only less
        // efficiently (no 304) or relying on revision history (no 412).
        let Some(etag) = etag else {
            eprintln!("no ETag: every sync downloads the file");
            client.delete_directory(&directory).await.unwrap();
            return;
        };
        let not_modified = matches!(
            client
                .get_if_changed(&directory, name, Some(&etag), 1 << 20)
                .await
                .unwrap(),
            Fetched::NotModified
        );
        // Another device uploads in between...
        tokio::time::sleep(Duration::from_millis(1100)).await;
        client
            .put_if_match(&directory, name, b"two".to_vec(), None)
            .await
            .unwrap();
        // ...so an upload based on the old ETag should be refused.
        let refused = matches!(
            client
                .put_if_match(&directory, name, b"three".to_vec(), Some(&etag))
                .await
                .unwrap(),
            Stored::Conflict
        );
        let Fetched::Found { data, .. } = client
            .get_if_changed(&directory, name, Some(&etag), 1 << 20)
            .await
            .unwrap()
        else {
            panic!("changed file reported unchanged");
        };
        assert_eq!(
            data,
            if refused {
                b"two".to_vec()
            } else {
                b"three".to_vec()
            }
        );
        eprintln!(
            "ETag {etag}: If-None-Match {}, If-Match {}",
            if not_modified { "honored" } else { "ignored" },
            if refused || etag.starts_with("W/") {
                "honored or skipped (weak)"
            } else {
                "ignored"
            },
        );
        client.delete_directory(&directory).await.unwrap();
    }

    #[test]
    fn rejects_malformed_listing() {
        let directory = Url::parse("https://example.com/").unwrap();
        assert!(parse_multistatus("<d:multistatus><d:response></x>", &directory).is_err());
    }
}
