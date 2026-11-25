use super::*;

fn assert_header(visual: &mut VisualTestContext) {
    assert_page_box(
        visual,
        super::settings_page::SETTINGS_HEADER_ID,
        0.0,
        36.0,
        800.0,
        73.0,
    );
    assert_page_box(
        visual,
        super::settings_page::SETTINGS_SEARCH_FIELD_ID,
        496.0,
        60.0,
        256.0,
        32.0,
    );
}

#[gpui_kit::test]
fn page_layout_matches_the_reference_metrics(cx: &mut TestAppContext) {
    let mut harness = harness(cx, AppConfig::default());
    harness
        .visual
        .simulate_resize(gpui_kit::size(gpui_kit::px(800.0), gpui_kit::px(636.0)));
    click(harness.visual, SETTINGS_OPEN_BUTTON_ID);

    // The 36px window title bar sits above the 800x600 settings content area.
    assert_page_box(harness.visual, SETTINGS_PAGE_ID, 0.0, 36.0, 800.0, 600.0);
    assert_header(harness.visual);
    assert_page_box(
        harness.visual,
        super::settings_page::SETTINGS_SIDEBAR_ID,
        0.0,
        109.0,
        192.0,
        527.0,
    );
    assert_page_box(
        harness.visual,
        super::settings_page::SETTINGS_TAB_TERMINAL_ID,
        12.0,
        139.0,
        159.0,
        32.0,
    );
    assert_page_box(
        harness.visual,
        super::settings_page::SETTINGS_PANEL_ID,
        192.0,
        109.0,
        608.0,
        527.0,
    );
    // The page shows text rendering first, then the cursor samples, the
    // clipboard switches, scrollback, padding, the preview and one Save last.
    // The font section is taller than the viewport, so the later sections are
    // laid out below it rather than hidden by a second save action.
    assert_page_box(
        harness.visual,
        crate::font_setting::SECTION_ID,
        216.0,
        133.0,
        536.0,
        626.0,
    );
    assert_page_box(
        harness.visual,
        crate::font_setting::LINE_HEIGHT_INPUT_ID,
        216.0,
        289.0,
        262.0,
        36.0,
    );
    assert_page_box(
        harness.visual,
        crate::font_setting::LETTER_SPACING_INPUT_ID,
        490.0,
        289.0,
        262.0,
        36.0,
    );
    let cursor = page_bounds(harness.visual, crate::appearance_setting::CURSOR_SECTION_ID);
    let clipboard = page_bounds(harness.visual, crate::clipboard_setting::SECTION_ID);
    assert_eq!(clipboard.origin.x.as_f32(), 216.0);
    assert_eq!(clipboard.size.width.as_f32(), 536.0);
    assert!(
        clipboard.origin.y > cursor.origin.y + cursor.size.height,
        "the clipboard switches follow the cursor choices"
    );
    let scrollback = page_bounds(
        harness.visual,
        super::settings_page::SETTINGS_SCROLLBACK_SECTION_ID,
    );
    assert_eq!(scrollback.origin.x.as_f32(), 216.0);
    assert_eq!(scrollback.size.width.as_f32(), 536.0);
    assert_eq!(scrollback.size.height.as_f32(), 111.0);
    assert_eq!(
        scrollback.origin.y,
        clipboard.origin.y + clipboard.size.height + gpui_kit::px(24.0),
        "the scrollback section follows the clipboard section with one section gap"
    );
    let scrollback_input = page_bounds(
        harness.visual,
        super::settings_page::SETTINGS_SCROLLBACK_INPUT_ID,
    );
    assert_eq!(scrollback_input.origin.x.as_f32(), 216.0);
    assert_eq!(scrollback_input.size.width.as_f32(), 536.0);
    assert_eq!(scrollback_input.size.height.as_f32(), 36.0);
    assert_eq!(
        scrollback_input.origin.y,
        scrollback.origin.y + gpui_kit::px(22.0)
    );
    let padding = page_bounds(
        harness.visual,
        crate::appearance_setting::PADDING_SECTION_ID,
    );
    assert!(
        padding.origin.y > scrollback.origin.y + scrollback.size.height,
        "the padding section follows the scrollback section"
    );
    let preview = page_bounds(harness.visual, crate::font_setting::PREVIEW_ID);
    assert_eq!(preview.origin.x.as_f32(), 216.0);
    assert_eq!(preview.size.width.as_f32(), 536.0);
    assert_eq!(preview.size.height.as_f32(), 40.0);
    assert!(
        preview.origin.y > padding.origin.y,
        "the preview follows the padding section"
    );
    let save = page_bounds(harness.visual, super::settings_page::SETTINGS_SAVE_ID);
    assert_eq!(save.origin.x.as_f32(), 216.0);
    assert_eq!(save.size.width.as_f32(), 536.0);
    assert_eq!(save.size.height.as_f32(), 36.0);
    assert!(
        save.origin.y > preview.origin.y + preview.size.height,
        "the one Save stays below the last section"
    );
    assert!(
        harness
            .visual
            .debug_bounds("settings-scrollback-save")
            .is_none(),
        "the scrollback field must not keep an orphan save button"
    );

    if std::env::var_os("TTERM_GPUI_WRITE_SETTINGS_LAYOUT").is_some() {
        write_layout_json(harness.visual, "layout.json", &LAYOUT_IDS);
    }

    type_search(&mut harness, "scrollback");
    assert_page_box(
        harness.visual,
        super::settings_page::SETTINGS_SEARCH_RESULT_0_ID,
        216.0,
        154.0,
        560.0,
        40.0,
    );
    if std::env::var_os("TTERM_GPUI_WRITE_SETTINGS_LAYOUT").is_some() {
        write_layout_json(harness.visual, "layout-search.json", &SEARCH_LAYOUT_IDS);
    }

    press(harness.visual, "escape");
    type_search(&mut harness, "zzz");
    if std::env::var_os("TTERM_GPUI_WRITE_SETTINGS_LAYOUT").is_some() {
        write_layout_json(harness.visual, "layout-empty.json", &EMPTY_LAYOUT_IDS);
    }

    harness.visual.update(|window, _| window.remove_window());
}

const LAYOUT_IDS: [&str; 10] = [
    SETTINGS_PAGE_ID,
    super::settings_page::SETTINGS_HEADER_ID,
    super::settings_page::SETTINGS_TITLE_ID,
    super::settings_page::SETTINGS_SEARCH_FIELD_ID,
    super::settings_page::SETTINGS_SIDEBAR_ID,
    super::settings_page::SETTINGS_TAB_TERMINAL_ID,
    super::settings_page::SETTINGS_PANEL_ID,
    super::settings_page::SETTINGS_SCROLLBACK_SECTION_ID,
    super::settings_page::SETTINGS_SCROLLBACK_INPUT_ID,
    super::settings_page::SETTINGS_SAVE_ID,
];

const SEARCH_LAYOUT_IDS: [&str; 6] = [
    SETTINGS_PAGE_ID,
    super::settings_page::SETTINGS_SEARCH_FIELD_ID,
    super::settings_page::SETTINGS_SEARCH_RESULTS_ID,
    super::settings_page::SETTINGS_SEARCH_COUNT_ID,
    super::settings_page::SETTINGS_SEARCH_RESULT_0_ID,
    super::settings_page::SETTINGS_SIDEBAR_ID,
];

const EMPTY_LAYOUT_IDS: [&str; 4] = [
    SETTINGS_PAGE_ID,
    super::settings_page::SETTINGS_SEARCH_FIELD_ID,
    super::settings_page::SETTINGS_SEARCH_EMPTY_ID,
    super::settings_page::SETTINGS_SIDEBAR_ID,
];
