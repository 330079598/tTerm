use super::output::deliver_output;
use super::{close_session, finish_output, SshChannelIo, SshExitSignal};
use crate::core::session::types::SessionPlan;
use crate::core::state::ChannelContext;
use crate::ssh::jump::pool::{PooledSession, RouteKey};
use crate::ssh::types::SshClientHandler;
use crate::zmodem::session::{ZmodemReceiveDriver, ZmodemSettings};
use russh::ChannelMsg;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

struct IncomingOutput<'a> {
    tab: &'a ChannelContext,
    sender: &'a Option<crate::terminal::io_batcher::TerminalOutputSender>,
    output_decoder: &'a mut Option<crate::terminal::encoding::OutputDecoder>,
    zmodem_driver: &'a mut ZmodemReceiveDriver,
    zmodem_send_pipe: &'a mut crate::zmodem::ZmodemSendPipe,
    ssh_query_pending: &'a mut Vec<u8>,
    record_raw_output: bool,
}

impl IncomingOutput<'_> {
    async fn data(
        &mut self,
        data: &[u8],
        writer_stream: &mut (impl tokio::io::AsyncWrite + Unpin),
    ) -> Result<(), String> {
        if self.record_raw_output {
            crate::session_log::record_output(&self.tab.app, &self.tab.tab_id, data);
        }

        if self.zmodem_send_pipe.try_forward(data) {
            return Ok(());
        }

        // Process terminal queries (vim t_u7, t_RV, etc.) before forwarding to UI
        // This is critical for SSH connections where vim waits for responses
        let processed = crate::terminal::ssh_query_handler::process_ssh_output_for_ui(
            data,
            self.ssh_query_pending,
            writer_stream,
        )
        .await;

        let bytes = match processed {
            Ok(bytes) => bytes,
            Err(e) => {
                // Fallback: use raw data if processing fails
                eprintln!("SSH output processing failed: {}", e);
                data.to_vec()
            }
        };
        if !bytes.is_empty() {
            let outcome = self.zmodem_driver.process(&bytes);
            if !outcome.outgoing.is_empty() {
                if let Err(err) = writer_stream.write_all(&outcome.outgoing).await {
                    return Err(format!("SSH write failed: {err}"));
                }
            }
            if let Some(remainder) = outcome.send_requested {
                let tx = crate::zmodem::arm_send(self.tab, remainder);
                self.zmodem_send_pipe.arm(tx);
                return Ok(());
            }
            if !outcome.passthrough.is_empty() {
                deliver_output(
                    &self.tab.app,
                    &self.tab.tab_id,
                    self.sender.as_ref(),
                    self.output_decoder.as_mut(),
                    outcome.passthrough,
                );
            }
        }
        Ok(())
    }
}

pub(super) struct ConnectedSession {
    pub(super) tab: ChannelContext,
    pub(super) settings: ZmodemSettings,
    pub(super) plan: SessionPlan,
    pub(super) route: Option<RouteKey>,
    pub(super) session: Arc<PooledSession<SshClientHandler>>,
    pub(super) channel: russh::Channel<russh::client::Msg>,
    pub(super) early_output: Option<Vec<u8>>,
    pub(super) io: SshChannelIo,
}

pub(super) async fn read_connection(connection: ConnectedSession) -> SshExitSignal {
    let ConnectedSession {
        tab,
        settings,
        plan,
        route,
        session,
        channel,
        early_output,
        io,
    } = connection;
    let SshChannelIo {
        mut stop_rx,
        mut input_rx,
        mut resize_rx,
        mut sender,
    } = io;
    let app = tab.app.clone();
    let tab_id = tab.tab_id.clone();
    let (mut reader, writer) = channel.split();
    let mut writer_stream = writer.make_writer();
    let mut ssh_query_pending = Vec::new();
    let connected_at = std::time::Instant::now();
    let mut zmodem_driver = ZmodemReceiveDriver::new(&tab, settings);
    // Once a `rz`-style trigger arms this tab for the send direction, every
    // subsequent byte is piped here instead of through `zmodem_driver` —
    // see `ZmodemSendPipe`'s doc comment.
    let mut zmodem_send_pipe = crate::zmodem::ZmodemSendPipe::default();
    let mut output_decoder = plan.encoding.output_decoder();
    // A UTF-8 session logs output as received. A transcoded one logs it in
    // `deliver_output` instead, since bytes in a foreign charset would be
    // unreadable in a plain-text log.
    let record_raw_output = output_decoder.is_none();

    if let Some(output) = early_output.filter(|output| !output.is_empty()) {
        if record_raw_output {
            crate::session_log::record_output(&app, &tab_id, &output);
        }
        deliver_output(
            &app,
            &tab_id,
            sender.as_ref(),
            output_decoder.as_mut(),
            output,
        );
    }

    loop {
        tokio::select! {
            _ = stop_rx.changed() => {
                if *stop_rx.borrow() {
                    let _ = writer.close().await;
                    close_session(route.as_ref(), &session, "Session closed").await;
                    finish_output(&mut sender);
                    return SshExitSignal {
                        terminated: true,
                        recoverable: false,
                        reason: None,
                        connected_duration: Some(connected_at.elapsed()),
                    };
                }
            }
            incoming = input_rx.recv() => {
                if let Some(data) = incoming {
                    if let Err(err) = writer_stream.write_all(&data).await {
                        finish_output(&mut sender);
                        return SshExitSignal {
                            terminated: false,
                            recoverable: true,
                            reason: Some(format!("SSH write failed: {err}")),
                            connected_duration: Some(connected_at.elapsed()),
                        };
                    }
                }
            }
            resize = resize_rx.recv() => {
                if let Some((next_rows, next_cols)) = resize {
                    if let Err(err) = writer.window_change(next_cols as u32, next_rows as u32, 0, 0).await {
                        finish_output(&mut sender);
                        return SshExitSignal {
                            terminated: false,
                            recoverable: true,
                            reason: Some(format!("SSH resize failed: {err}")),
                            connected_duration: Some(connected_at.elapsed()),
                        };
                    }
                }
            }
            event = reader.wait() => {
                match event {
                    Some(ChannelMsg::Data { data }) => {
                        let result = IncomingOutput {
                            tab: &tab, sender: &sender, output_decoder: &mut output_decoder,
                            zmodem_driver: &mut zmodem_driver, zmodem_send_pipe: &mut zmodem_send_pipe,
                            ssh_query_pending: &mut ssh_query_pending, record_raw_output,
                        }.data(data.as_ref(), &mut writer_stream).await;
                        if let Err(reason) = result {
                            finish_output(&mut sender);
                            return SshExitSignal {
                                terminated: false,
                                recoverable: true,
                                reason: Some(reason),
                                connected_duration: Some(connected_at.elapsed()),
                            };
                        }
                    }
                    Some(ChannelMsg::ExtendedData { data, .. }) => {
                        if record_raw_output {
                            crate::session_log::record_output(&app, &tab_id, data.as_ref());
                        }
                        deliver_output(
                            &app,
                            &tab_id,
                            sender.as_ref(),
                            output_decoder.as_mut(),
                            data.as_ref().to_vec(),
                        );
                    }
                    Some(ChannelMsg::ExitStatus { .. }) => {
                        close_session(route.as_ref(), &session, "Shell exited").await;
                        finish_output(&mut sender);
                        return SshExitSignal {
                            terminated: true,
                            recoverable: false,
                            reason: None,
                            connected_duration: Some(connected_at.elapsed()),
                        };
                    }
                    Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => {
                        finish_output(&mut sender);
                        return SshExitSignal {
                            terminated: false,
                            recoverable: true,
                            reason: Some("SSH channel closed".to_string()),
                            connected_duration: Some(connected_at.elapsed()),
                        };
                    }
                    _ => {}
                }
            }
        }
    }
}
