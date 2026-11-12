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
