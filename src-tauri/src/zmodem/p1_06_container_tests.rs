//! Explicit opt-in tests against this card's disposable OrbStack host.
use crate::core::state::{ActiveSession, ActiveSsh};
use crate::zmodem::send::{run, ZmodemSendParams};
use crate::zmodem::session::ZmodemReceiveDriver;
use russh::{client, ChannelMsg};
use std::sync::{atomic::AtomicBool, Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tterm_core::events::zmodem::TransferEvent;

struct TestHost;
impl client::Handler for TestHost {
    type Error = russh::Error;
    async fn check_server_key(
        &mut self,
        _: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

async fn connect() -> client::Handle<TestHost> {
    assert_eq!(
        std::env::var("TTERM_P1_06_HOST").as_deref(),
        Ok("127.0.0.1:22606"),
        "explicit disposable host opt-in required"
    );
    let mut handle = client::connect(
        Arc::new(client::Config::default()),
        ("127.0.0.1", 22606),
        TestHost,
    )
    .await
    .unwrap();
    assert!(handle
        .authenticate_password("alice", "alice-pass")
        .await
        .unwrap()
        .success());
    handle
}

async fn exec(handle: &client::Handle<TestHost>, command: &str) -> Vec<u8> {
    let mut channel = handle.channel_open_session().await.unwrap();
    channel.exec(true, command).await.unwrap();
    let mut output = Vec::new();
    while let Some(message) = channel.wait().await {
        if let ChannelMsg::Data { data } = message {
            output.extend_from_slice(&data);
        }
    }
    output
}

#[tokio::test]
#[ignore = "requires the disposable P1-06 Docker host and explicit environment opt-in"]
async fn container_ssh_auth_exec_and_forwarding() {
    let handle = connect().await;
    assert_eq!(exec(&handle, "printf P1-06-SSH").await, b"P1-06-SSH");
    let channel = handle
        .channel_open_direct_tcpip("127.0.0.1", 22, "127.0.0.1", 0)
        .await
        .unwrap();
    let mut stream = channel.into_stream();
    let mut greeting = [0; 7];
    stream.read_exact(&mut greeting).await.unwrap();
    assert_eq!(&greeting, b"SSH-2.0");
    stream.shutdown().await.unwrap();
    handle
        .disconnect(russh::Disconnect::ByApplication, "done", "en")
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires lrzsz in the disposable P1-06 Docker host"]
async fn container_zmodem_download() {
    let handle = connect().await;
    exec(&handle,"mkdir -p /home/alice/p1-06; python3 -c 'open(\"/home/alice/p1-06/payload.bin\",\"wb\").write(bytes(range(256))*256)'").await;
    let dir = crate::ssh::test_sshd::TempDir::new();
    let events = Arc::new(Mutex::new(Vec::new()));
    let record = events.clone();
    let mut driver = ZmodemReceiveDriver::with_test_emit(
        move |kind, payload| record.lock().unwrap().push((kind.to_string(), payload)),
        "p1-06-download".into(),
        1,
        Default::default(),
        true,
        dir.path.clone(),
    );
    let mut channel = handle.channel_open_session().await.unwrap();
    channel
        .exec(true, "cd /home/alice/p1-06 && sz --binary payload.bin")
        .await
        .unwrap();
    let mut writer = channel.make_writer();
    tokio::time::timeout(Duration::from_secs(30), async {
        while let Some(message) = channel.wait().await {
            match message {
                ChannelMsg::Data { data } => {
                    let result = driver.process(&data);
                    writer.write_all(&result.outgoing).await.unwrap();
                    writer.flush().await.unwrap();
                }
                ChannelMsg::Eof | ChannelMsg::ExitStatus { .. } => break,
                _ => {}
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(
        std::fs::read(dir.path.join("payload.bin")).unwrap(),
        (0..256).flat_map(|_| 0u8..=255).collect::<Vec<_>>()
    );
    let events = events.lock().unwrap();
    assert_eq!(events.first().unwrap().0, "transfer-start");
    assert_eq!(events.last().unwrap().0, "transfer-complete");
    assert_eq!(events.last().unwrap().1["success"], true);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires lrzsz in the disposable P1-06 Docker host"]
async fn container_zmodem_upload() {
    let handle = connect().await;
    exec(
        &handle,
        "mkdir -p /home/alice/p1-06/upload; rm -f /home/alice/p1-06/upload/upload.bin",
    )
    .await;
    let dir = crate::ssh::test_sshd::TempDir::new();
    let payload = (0..256).flat_map(|_| 0u8..=255).collect::<Vec<_>>();
    let source = dir.path.join("upload.bin");
    std::fs::write(&source, &payload).unwrap();
    let mut channel = handle.channel_open_session().await.unwrap();
    channel
        .exec(true, "cd /home/alice/p1-06/upload && rz --binary")
        .await
        .unwrap();
    let mut writer = channel.make_writer();
    let (input_tx, mut input_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    let (resize_tx, _) = tokio::sync::mpsc::unbounded_channel();
    let writing = tokio::spawn(async move {
        while let Some(bytes) = input_rx.recv().await {
            if writer.write_all(&bytes).await.is_err() {
                break;
            }
            let _ = writer.flush().await;
        }
    });
    let active = Arc::new(tokio::sync::Mutex::new(Some(ActiveSession::Ssh(
        ActiveSsh {
            input: crate::terminal::types::PtyInput::over_channel(input_tx.clone()),
            input_tx,
            resize_tx,
            task: tokio::spawn(async {}),
            output_tail: Default::default(),
            encoding: Default::default(),
        },
    ))));
    let (tx, rx) = std::sync::mpsc::channel();
    let events = Arc::new(Mutex::new(Vec::new()));
    let record = events.clone();
    let sending = std::thread::spawn(move || {
        run(ZmodemSendParams {
            emit: Box::new(move |event| record.lock().unwrap().push(event)),
            tab_id: "p1-06-upload".into(),
            zmodem_map: Default::default(),
            incoming: rx,
            active,
            cancel_requested: Arc::new(AtomicBool::new(false)),
            paths: vec![source],
        })
    });
    tokio::time::timeout(Duration::from_secs(30), async {
        while let Some(message) = channel.wait().await {
            match message {
                ChannelMsg::Data { data } => {
                    if tx.send(data.to_vec()).is_err() {
                        break;
                    }
                }
                ChannelMsg::Eof | ChannelMsg::ExitStatus { .. } => break,
                _ => {}
            }
        }
    })
    .await
    .unwrap();
    drop(tx);
    sending.join().unwrap();
    writing.abort();
    let _ = writing.await;
    assert_eq!(
        exec(&handle, "cat /home/alice/p1-06/upload/upload.bin").await,
        payload
    );
    let events = events.lock().unwrap();
    assert!(matches!(events.first(), Some(TransferEvent::Start(_))));
    assert!(matches!(events.last(),Some(TransferEvent::Complete(p)) if p.success));
}
