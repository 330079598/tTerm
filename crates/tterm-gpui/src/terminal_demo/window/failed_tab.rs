//! The placeholder tab a workspace opens when its session cannot start.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use gpui_kit::prelude::*;
use tterm_core::agent_status::AgentResumeSession;
use tterm_core::i18n::translate::TranslationKey;
use tterm_core::workspace::tab::Tab;
use tterm_terminal::palette::TerminalPalette;
use tterm_terminal::terminal::{Config, Terminal};
use tterm_terminal_view::view::font::TerminalFont;
use tterm_terminal_view::view::{TerminalView, TerminalViewConfig};

use super::super::binding::TabBinding;
use super::picker;
use super::{INITIAL_CELL, INITIAL_GRID, Prototype, WindowTab};

/// The parts of a failed launch needed to build its placeholder tab.
pub(super) struct FailedTab {
    pub(super) saved_id: String,
    pub(super) saved: Tab,
    pub(super) restored: bool,
    pub(super) default_shell: bool,
    pub(super) may_resume_agent: bool,
    pub(super) agent_resume: Option<AgentResumeSession>,
    pub(super) binding: Option<Rc<TabBinding>>,
    pub(super) fallback_key: TranslationKey,
    pub(super) search: picker::ConnectionLabel,
}

/// Builds a local view that shows the launch error instead of a session.
///
/// The placeholder draws with the shared config font, so a failed launch does
/// not fall back to the platform size while every other pane uses the user's
/// choice.
pub(super) fn failed(
    cx: &mut Context<Prototype>,
    tab: FailedTab,
    error: String,
    font: TerminalFont,
    palette: TerminalPalette,
) -> WindowTab {
    eprintln!("tterm-gpui: {error}");
    let terminal = Arc::new(Terminal::new(Config::default(), INITIAL_GRID, INITIAL_CELL));
    terminal.write(format!("\x1b[31m[{error}]\x1b[0m\r\n").as_bytes());
    let view = cx.new(|cx| {
        TerminalView::new(
            cx,
            terminal,
            TerminalViewConfig {
                font,
                palette,
                ..TerminalViewConfig::default()
            },
        )
    });
    WindowTab {
        saved_id: tab.saved_id,
        saved: tab.saved,
        restored: tab.restored,
        default_shell: tab.default_shell,
        may_resume_agent: tab.may_resume_agent,
        agent_resume: tab.agent_resume,
        agent_resume_controller: None,
        search: tab.search,
        fallback_key: tab.fallback_key,
        view,
        search_bar: None,
        binding: tab.binding,
        title: Arc::new(Mutex::new(None)),
        program_title: RefCell::default(),
        paste_controller: None,
        context_menu: None,
    }
}

/// Reads the agent-resume record a saved tab carries, if it has one.
pub(super) fn saved_agent_resume(tab: &Tab) -> Option<AgentResumeSession> {
    tab.connection
        .as_ref()
        .and_then(|connection| connection.saved_field("agentResume"))
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
}
