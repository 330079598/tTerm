//! Copying the selected text out of the grid.
//!
//! The selection may cover hundreds of thousands of scrollback rows, so the
//! text is built only when the user copies it (design note 6.3).

use super::{State, Terminal};
use crate::selection::{SelectionRange, word};

impl Terminal {
    /// The selected text, with trailing blanks trimmed and wrapped lines
    /// joined.
    ///
    /// The caller should only ask when the user copies: a selection over the
    /// whole scrollback would otherwise build a huge string every frame
    /// (design note 6.3).
    pub fn selection_text(&self) -> Option<String> {
        let mut state = self.state.lock();
        let combined = active_selection_range(&state)?;
        if state.word_selection && state.word_drag.is_some() {
            let anchor = state.term.selection.clone();
            word::set_range(&mut state.term, combined);
            let text = state.term.selection_to_string();
            state.term.selection = anchor;
            return text;
        }
        state.term.selection_to_string()
    }
}

/// The selection the view and copy operation should use.
///
/// A word drag keeps its original word in Alacritty's selection so that the
/// anchor follows the grid's lifecycle, while `word_drag` holds the word under
/// the pointer. The displayed range is always rebuilt from those two.
pub(super) fn active_selection_range(state: &State) -> Option<SelectionRange> {
    let anchor = state
        .term
        .selection
        .as_ref()?
        .to_range(&state.term)
        .map(SelectionRange::from)?;
    match state.word_drag {
        Some(drag) if state.word_selection => Some(SelectionRange {
            start: anchor.start.min(drag.start),
            end: anchor.end.max(drag.end),
            block: false,
        }),
        _ => Some(anchor),
    }
}
