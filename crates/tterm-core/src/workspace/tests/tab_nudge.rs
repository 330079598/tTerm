use crate::workspace::tab_nudge::{NudgeAnimationTarget, TabNudgeState};

use super::ids;

fn bar() -> TabNudgeState {
    TabNudgeState::new(ids(&["t1", "t2"]))
}

#[test]
fn flashes_only_the_named_tab_and_scrolls_it_into_view() {
    let mut state = bar();
    state.nudge_tab("t2");
    assert!(state.is_flashing("t2"));
    assert!(!state.is_flashing("t1"));
    assert_eq!(state.scroll_requests(), 1);
}

#[test]
fn clears_the_flash_when_its_own_animation_ends_not_a_childs() {
    let mut state = bar();
    state.nudge_tab("t1");
    state.animation_end(NudgeAnimationTarget::TabChild("t1".to_owned()));
    assert!(state.is_flashing("t1"));
    state.animation_end(NudgeAnimationTarget::Tab("t1".to_owned()));
    assert!(!state.is_flashing("t1"));
}

#[test]
fn replays_a_nudge_that_is_still_running() {
    let mut state = bar();
    state.nudge_tab("t1");
    state.nudge_tab("t1");
    assert!(state.is_flashing("t1"));
    state.animation_end(NudgeAnimationTarget::Tab("t1".to_owned()));
    assert!(!state.is_flashing("t1"));
    state.nudge_tab("t1");
    assert!(state.is_flashing("t1"));
}

#[test]
fn ignores_a_tab_that_is_not_in_the_bar() {
    let mut state = bar();
    state.nudge_tab("missing");
    assert!(
        state
            .tab_ids
            .iter()
            .all(|tab_id| !state.is_flashing(tab_id))
    );
    assert_eq!(state.scroll_requests(), 0);
}

#[test]
fn shakes_the_tab_bar() {
    let mut state = bar();
    state.nudge_tab_bar();
    assert!(state.is_shaking());
    state.animation_end(NudgeAnimationTarget::Bar);
    assert!(!state.is_shaking());
}

#[test]
fn oracle_does_nothing_when_the_bar_is_absent() {
    let oracle = super::oracle_json();
    let mut state = bar();
    state.bar_present = false;
    state.nudge_tab_bar();
    assert_eq!(
        !state.is_shaking(),
        oracle["tabNudge"]["barAbsent"]
            .as_bool()
            .expect("the absent-bar result")
    );
    state.animation_end(NudgeAnimationTarget::Bar);
    assert!(!state.is_shaking());
}
