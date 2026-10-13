use super::*;
#[cfg(unix)]
use tokio::io::AsyncWriteExt;

fn target<'a>(host: &'a str, username: &'a str) -> CommandTarget<'a> {
    CommandTarget {
        host,
        port: 2222,
        username,
    }
}

#[test]
fn tokens_are_replaced_as_openssh_does() {
    let expanded = expand(
        "nc -X connect -x proxy:8080 %h %p # %r %n 100%%",
        &target("db.internal", "me"),
    )
    .unwrap();
    assert_eq!(
        expanded,
        "nc -X connect -x proxy:8080 db.internal 2222 # me db.internal 100%"
    );
    assert_eq!(
        expand("ssh -W [%h]:%p jump", &target("fe80::1%en0", "")).unwrap(),
        "ssh -W [fe80::1%en0]:2222 jump"
    );
}

#[test]
fn unknown_tokens_and_a_lone_percent_are_refused() {
    let error = expand("connect %d", &target("h", "u")).unwrap_err();
    assert!(error.contains("%d"), "{error}");
    let error = expand("connect 100%", &target("h", "u")).unwrap_err();
    assert!(error.contains("%%"), "{error}");
}

#[test]
fn names_that_a_shell_would_interpret_are_refused() {
    for host in ["a;reboot", "$(id)", "a b", "`id`", "-oProxyCommand=x"] {
        assert!(expand("nc %h %p", &target(host, "u")).is_err(), "{host}");
    }
    let error = expand("ssh %r@jump", &target("h", "me|x")).unwrap_err();
    assert!(error.contains("user name"), "{error}");
    // A name the command does not use is not checked.
    assert!(expand("nc %h %p", &target("h", "me|x")).is_ok());
}

#[test]
fn the_error_keeps_the_last_lines_of_the_error_output() {
    assert_eq!(
        last_lines("one\n\ntwo\nthree\nfour\n", 3),
        "two / three / four"
    );
    assert_eq!(last_lines("  \n", 3), "");
}

#[cfg(unix)]
#[tokio::test]
async fn the_stream_carries_the_command_input_and_output() {
    let mut stream = spawn("cat").await.unwrap();
    stream.write_all(b"SSH-2.0-test\r\n").await.unwrap();
    let mut buffer = [0u8; 14];
    stream.read_exact(&mut buffer).await.unwrap();
    assert_eq!(&buffer, b"SSH-2.0-test\r\n");
    stream.shutdown().await.unwrap();
    let mut rest = Vec::new();
    stream.read_to_end(&mut rest).await.unwrap();
    assert!(rest.is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn a_failing_command_reports_its_status_and_error_output() {
    let mut stream = spawn("sh -c 'echo nc: connection refused >&2; exit 3'")
        .await
        .unwrap();
    let mut output = Vec::new();
    let error = stream.read_to_end(&mut output).await.unwrap_err();
    let message = error.to_string();
    assert!(message.contains("status 3"), "{message}");
    assert!(message.contains("nc: connection refused"), "{message}");
}

#[cfg(unix)]
#[tokio::test]
async fn writing_to_a_command_that_exited_reports_why() {
    let mut stream = spawn("tterm-no-such-program-for-tests").await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    let mut error = None;
    for _ in 0..50 {
        if let Err(failure) = stream.write_all(&[0u8; 4096]).await {
            error = Some(failure);
            break;
        }
    }
    let message = error.expect("the write fails").to_string();
    assert!(message.contains("status 127"), "{message}");
    assert!(message.contains("PATH"), "{message}");
}
