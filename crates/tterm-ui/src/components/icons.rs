use gpui_kit::{Hsla, IntoElement, Styled, px, svg};

pub const EYE: &[u8] = include_bytes!("../../assets/icons/eye.svg");
pub const EYE_OFF: &[u8] = include_bytes!("../../assets/icons/eye-off.svg");
pub const ARROW_BIG_UP_DASH: &[u8] = include_bytes!("../../assets/icons/arrow-big-up-dash.svg");
pub const CHEVRON_DOWN: &[u8] = include_bytes!("../../assets/icons/chevron-down.svg");
pub const X: &[u8] = include_bytes!("../../assets/icons/x.svg");

pub fn icon(data: &'static [u8], color: Hsla, size: f32) -> impl IntoElement {
    svg().data(data).size(px(size)).text_color(color)
}

pub const TRIANGLE_ALERT: &[u8] = include_bytes!("../../assets/icons/triangle-alert.svg");

pub const INFO: &[u8] = include_bytes!("../../assets/icons/info.svg");
pub const PLUS: &[u8] = include_bytes!("../../assets/icons/plus.svg");
pub const ROTATE_CCW: &[u8] = include_bytes!("../../assets/icons/rotate-ccw.svg");
