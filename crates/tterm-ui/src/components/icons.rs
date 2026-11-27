use gpui_kit::{Hsla, IntoElement, Styled, px, svg};

pub const EYE: &[u8] = include_bytes!("../../assets/icons/eye.svg");
pub const EYE_OFF: &[u8] = include_bytes!("../../assets/icons/eye-off.svg");
pub const ARROW_BIG_UP_DASH: &[u8] = include_bytes!("../../assets/icons/arrow-big-up-dash.svg");
pub const CHEVRON_DOWN: &[u8] = include_bytes!("../../assets/icons/chevron-down.svg");
pub const X: &[u8] = include_bytes!("../../assets/icons/x.svg");
pub const GRIP_HORIZONTAL: &[u8] = include_bytes!("../../assets/icons/grip-horizontal.svg");

pub fn icon(data: &'static [u8], color: Hsla, size: f32) -> impl IntoElement {
    svg().data(data).size(px(size)).text_color(color)
}

pub const TRIANGLE_ALERT: &[u8] = include_bytes!("../../assets/icons/triangle-alert.svg");

pub const INFO: &[u8] = include_bytes!("../../assets/icons/info.svg");
pub const PLUS: &[u8] = include_bytes!("../../assets/icons/plus.svg");
pub const ROTATE_CCW: &[u8] = include_bytes!("../../assets/icons/rotate-ccw.svg");
pub const ARROW_DOWN: &[u8] = include_bytes!("../../assets/icons/arrow-down.svg");
pub const ARROW_UP: &[u8] = include_bytes!("../../assets/icons/arrow-up.svg");
pub const CHEVRON_RIGHT: &[u8] = include_bytes!("../../assets/icons/chevron-right.svg");
pub const LAPTOP: &[u8] = include_bytes!("../../assets/icons/laptop.svg");
pub const SERVER: &[u8] = include_bytes!("../../assets/icons/server.svg");
pub const TARGET: &[u8] = include_bytes!("../../assets/icons/target.svg");
pub const WAYPOINTS: &[u8] = include_bytes!("../../assets/icons/waypoints.svg");
pub const SETTINGS: &[u8] = include_bytes!("../../assets/icons/settings.svg");
pub const BOOK_MARKED: &[u8] = include_bytes!("../../assets/icons/book-marked.svg");
pub const LAYOUT_TEMPLATE: &[u8] = include_bytes!("../../assets/icons/layout-template.svg");
pub const SQUARE_TERMINAL: &[u8] = include_bytes!("../../assets/icons/square-terminal.svg");
pub const SEARCH: &[u8] = include_bytes!("../../assets/icons/search.svg");
pub const SEARCH_X: &[u8] = include_bytes!("../../assets/icons/search-x.svg");
pub const CASE_SENSITIVE: &[u8] = include_bytes!("../../assets/icons/case-sensitive.svg");
pub const WHOLE_WORD: &[u8] = include_bytes!("../../assets/icons/whole-word.svg");
pub const REGEX: &[u8] = include_bytes!("../../assets/icons/regex.svg");
