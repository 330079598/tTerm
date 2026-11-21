//! Applies committed scrollback changes without invalidating unrelated window views.

use std::rc::Rc;

use gpui_kit::{App, Entity, Subscription};
use tterm_core::config::scrollback::resolve_scrollback_lines;

use crate::config_state::{ConfigState, ScrollbackChanged};
use crate::terminal_demo::binding::SessionManager;

pub(super) fn history_capacity(lines: u32) -> usize {
    resolve_scrollback_lines(Some(f64::from(lines))) as usize
}

pub(super) fn subscribe_history(
    cx: &mut App,
    config: &Entity<ConfigState>,
    sessions: &Rc<SessionManager>,
) -> Subscription {
    let sessions = Rc::downgrade(sessions);
    cx.subscribe(config, move |_, event: &ScrollbackChanged, _| {
        if let Some(sessions) = sessions.upgrade() {
            sessions.set_scrollback(history_capacity(event.lines));
        }
    })
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, sync::Arc};

    use gpui_kit::{AppContext, TestAppContext};
    use tterm_core::config::AppConfig;
    use tterm_core::workspace::tab_attention::TabAttentionStore;

    use super::super::{INITIAL_CELL, INITIAL_GRID};
    use super::*;
    use crate::terminal_demo::{command::CommandSpec, runtime::test_runtime};

    #[gpui_kit::test]
    async fn saving_against_an_open_tab_updates_the_attached_model(cx: &mut TestAppContext) {
        let runtime = test_runtime();
        let sessions = Rc::new(SessionManager::new(
            Arc::clone(&runtime.services),
            Rc::new(RefCell::new(TabAttentionStore::new())),
        ));
        let config = cx.update(|cx| {
            cx.new(|_| {
                ConfigState::with_writer(
                    AppConfig::default(),
                    runtime._runtime.handle(),
                    Arc::new(|_| Ok(())),
                )
            })
        });
        let _subscription = cx.update(|cx| subscribe_history(cx, &config, &sessions));
        let command = if cfg!(windows) { "cmd.exe" } else { "/bin/sh" };
        let connection = CommandSpec::Program {
            program: command.into(),
            args: Vec::new(),
        }
        .connection_options()
        .unwrap();
        let tab = sessions.open("live-history", connection, INITIAL_GRID, INITIAL_CELL);
        let attachment = tab.attach().unwrap();
        let model = attachment.model();
        for _ in 0..64 {
            model.write(b"history\r\n");
        }
        assert!(model.snapshot().history_size() > 3);
        cx.executor().allow_parking();
        for lines in [3, 0] {
            let saved = cx
                .update(|cx| config.update(cx, |state, cx| state.save_scrollback_lines(lines, cx)));
            assert_eq!(saved.await, Ok(lines));
            assert_eq!(model.scrollback(), history_capacity(lines));
            if lines == 3 {
                assert_eq!(model.snapshot().history_size(), 3);
            }
            assert_eq!(
                tab.attach().unwrap().session_nonce(),
                attachment.session_nonce()
            );
        }
    }
}
