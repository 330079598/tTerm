//! The scenes P0-25 paints, one per capability the card asks about.
//!
//! Each scene is a small tree of `div`s, plus a `canvas` where GPUI has no
//! style for the effect and the probe has to draw it itself. The notes in the
//! scene bodies say what the scene is meant to show and, where the answer is
//! "not supported", which fallback it paints instead; the API and source
//! evidence behind each answer is in `reports/P0-25.md`.

use gpui_kit::prelude::*;
use gpui_kit::{
    App, Bounds, BoxShadow, Div, Fill, Font, Hsla, PathBuilder, Pixels, Point, Rgba, SharedString,
    Size, TextAlign, TextRun, Window, canvas, div, linear_color_stop, linear_gradient, point, px,
    rgb, rgba, size, svg,
};

/// The page color every scene paints on; also the color of the "background" a
/// radial gradient fades into when it is faked with concentric circles.
pub(crate) const PAGE: u32 = 0x0d1117;
/// The panel color, one step lighter than the page.
const PANEL: u32 = 0x161b22;
/// The border color of the panels.
const BORDER: u32 = 0x30363d;
/// The body text color.
pub(crate) const TEXT: u32 = 0xe6edf3;
/// The caption color.
const MUTED: u32 = 0x8b949e;
/// The accent color, GitHub blue.
const ACCENT: u32 = 0x58a6ff;

/// The scene painted by one run of the probe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scene {
    /// Item 1: the window background appearance.
    BlurWindow,
    /// Item 2: an element that wants `backdrop-filter: blur()`.
    BlurElement,
    /// Item 3: a rounded container that clips its child.
    RoundedMask,
    /// Item 4: how many stops a linear gradient takes.
    LinearGradient,
    /// Item 5: a radial gradient.
    RadialGradient,
    /// Item 6: letter spacing.
    LetterSpacing,
    /// Item 7: several box shadows on one element.
    Shadows,
    /// Item 8: tinting an SVG.
    SvgColor,
}

impl Scene {
    /// Every scene, in the card's item order.
    pub(crate) const ALL: [Scene; 8] = [
        Scene::BlurWindow,
        Scene::BlurElement,
        Scene::RoundedMask,
        Scene::LinearGradient,
        Scene::RadialGradient,
        Scene::LetterSpacing,
        Scene::Shadows,
        Scene::SvgColor,
    ];

    /// The name `--scene` takes.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Scene::BlurWindow => "blur-window",
            Scene::BlurElement => "blur-element",
            Scene::RoundedMask => "rounded-mask",
            Scene::LinearGradient => "linear-gradient",
            Scene::RadialGradient => "radial-gradient",
            Scene::LetterSpacing => "letter-spacing",
            Scene::Shadows => "shadows",
            Scene::SvgColor => "svg-color",
        }
    }

    /// Resolves a `--scene` argument.
    pub(crate) fn from_name(name: &str) -> Option<Scene> {
        Scene::ALL.into_iter().find(|scene| scene.name() == name)
    }

    /// The headless window size, in logical pixels, the scene was laid out for.
    pub(crate) fn window_size(self) -> Size<Pixels> {
        match self {
            Scene::BlurWindow => size(px(760.), px(460.)),
            Scene::BlurElement => size(px(760.), px(560.)),
            Scene::RoundedMask => size(px(700.), px(420.)),
            Scene::LinearGradient => size(px(740.), px(480.)),
            Scene::RadialGradient => size(px(760.), px(540.)),
            Scene::LetterSpacing => size(px(720.), px(340.)),
            Scene::Shadows => size(px(760.), px(420.)),
            Scene::SvgColor => size(px(680.), px(300.)),
        }
    }

    /// The one-line note painted above the scene.
    pub(crate) fn caption(self) -> &'static str {
        match self {
            Scene::BlurWindow => {
                "1. Window-level blur: WindowBackgroundAppearance::Blurred. macOS puts an \
                 NSVisualEffectView behind the content; the content itself stays transparent."
            }
            Scene::BlurElement => {
                "2. Element-level blur: no API. The overlay below is an 80% opaque fill, which \
                 dims the terminal mock without blurring it."
            }
            Scene::RoundedMask => {
                "3. Content mask is a rectangle: overflow_hidden does not clip to the radius. \
                 Left is the failure, right is the inset-child workaround."
            }
            Scene::LinearGradient => {
                "4. Linear gradients take exactly two stops (Background.colors is \
                 [LinearColorStop; 2]). Below: two stops, three colors by two quads, and a hard \
                 stop from equal percentages."
            }
            Scene::RadialGradient => {
                "5. No radial gradient, no conic gradient and no mask. Below: 24 and 96 \
                 concentric circles, then a ring and a comet stroked on a linear gradient so the \
                 middle shows the background through."
            }
            Scene::LetterSpacing => {
                "6. No letter-spacing option in TextStyle, TextRun or shape_line. Below: one shaped \
                 line, one shaped line with each glyph moved 3 px, and the same glyphs at 0 px as \
                 the control."
            }
            Scene::Shadows => {
                "7. Box shadows are a Vec: one, two and three layers, plus an inset layer."
            }
            Scene::SvgColor => {
                "8. SVG is rasterized to an alpha mask and tinted with one color. Below: the same \
                 three-color file under three text_color values."
            }
        }
    }

    /// The scene's content.
    fn body(self) -> Div {
        match self {
            Scene::BlurWindow => blur_window(),
            Scene::BlurElement => blur_element(),
            Scene::RoundedMask => rounded_mask(),
            Scene::LinearGradient => linear_gradient_bars(),
            Scene::RadialGradient => radial_gradient_fallbacks(),
            Scene::LetterSpacing => letter_spacing(),
            Scene::Shadows => shadows(),
            Scene::SvgColor => svg_tints(),
        }
    }
}

/// One scene with its caption above it.
pub(crate) fn section(scene: Scene) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .text_xs()
                .text_color(rgb(MUTED))
                .child(scene.caption()),
        )
        .child(scene.body())
}

/// A caption above one demo element.
fn labelled<E: IntoElement>(caption: &'static str, child: E) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_xs().text_color(rgb(MUTED)).child(caption))
        .child(child)
}

/// Item 1: a translucent card on a window whose background appearance is
/// [`WindowBackgroundAppearance::Blurred`]. Nothing in this view paints the
/// page color, so the desktop behind the window is what gets blurred.
fn blur_window() -> Div {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .w(px(440.))
                .p_6()
                .rounded(px(16.))
                .bg(rgba(0x161b22cc))
                .border_1()
                .border_color(rgba(0x8b949e40))
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_color(rgb(TEXT))
                        .child("Blurred window background"),
                )
                .child(div().text_color(rgb(MUTED)).child(
                    "The card is 80% opaque. The blur comes from the compositor, so it only shows \
                     in a real window, not in a headless render.",
                )),
        )
}

/// Item 2: the mock terminal the overlay covers. Colored bars stand in for
/// text so the screenshot shows what the blur would have smoothed.
fn terminal_mock() -> Div {
    let lines = [
        (0x7ee787_u32, 0.72_f32),
        (0x8b949e, 0.42),
        (0x58a6ff, 0.58),
        (0x8b949e, 0.30),
        (0xf778ba, 0.64),
        (0x8b949e, 0.22),
        (0xd2a8ff, 0.50),
        (0x8b949e, 0.36),
    ];
    let mut terminal = div()
        .size_full()
        .flex()
        .flex_col()
        .gap_2()
        .p_4()
        .bg(rgb(0x010409));
    for (color, width) in lines {
        terminal = terminal.child(
            div()
                .h(px(12.))
                .w(px(420. * width))
                .rounded(px(3.))
                .bg(rgb(color)),
        );
    }
    terminal
}

/// Item 2: a dialog over a terminal mock, with the fill that replaces the
/// blur GPUI cannot draw.
fn blur_element() -> Div {
    div()
        .relative()
        .h(px(340.))
        .w_full()
        .rounded(px(12.))
        .overflow_hidden()
        .child(terminal_mock())
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .bg(rgba(0x0d1117cc)),
        )
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .w(px(380.))
                        .p_5()
                        .rounded(px(12.))
                        .bg(rgb(PANEL))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(div().text_color(rgb(TEXT)).child("Host key changed"))
                        .child(div().text_color(rgb(MUTED)).child(
                            "No BackdropFilter in GPUI: this panel is an opaque fill, and the \
                             terminal behind it is dimmed rather than blurred.",
                        )),
                ),
        )
}

/// One rounded panel of item 3. Without `inset` the child reaches the corners;
/// with it the child is held inside by the parent's padding.
fn rounded_panel(inset: bool) -> Div {
    let content = div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgb(ACCENT))
        .child(div().text_color(rgb(0x0b1f33)).text_xs().child(if inset {
            "inset 24 px: rounded"
        } else {
            "overflow_hidden: square"
        }));
    let panel = div()
        .w(px(300.))
        .h(px(200.))
        .rounded(px(24.))
        .bg(rgb(PANEL))
        .border_1()
        .border_color(rgb(BORDER))
        .overflow_hidden();
    if inset {
        panel.p(px(24.)).child(content.rounded(px(16.)))
    } else {
        panel.child(content)
    }
}

/// Item 3, side by side.
fn rounded_mask() -> Div {
    div()
        .flex()
        .flex_row()
        .gap_5()
        .child(rounded_panel(false))
        .child(rounded_panel(true))
}

/// A full-width rounded swatch of one background.
fn swatch(fill: impl Into<Fill>) -> Div {
    div().h(px(52.)).w_full().rounded(px(6.)).bg(fill)
}

/// Item 4: the two stops the API takes, the two-quad workaround for a third
/// color, and the hard stop that equal percentages produce.
fn linear_gradient_bars() -> Div {
    let two_stops = linear_gradient(
        90.,
        linear_color_stop(rgb(ACCENT), 0.),
        linear_color_stop(rgb(0xf778ba), 1.),
    );
    let left = div().h_full().w(px(340.)).bg(linear_gradient(
        90.,
        linear_color_stop(rgb(0x7ee787), 0.),
        linear_color_stop(rgb(0x58a6ff), 1.),
    ));
    let right = div().h_full().flex_grow(1.0).bg(linear_gradient(
        90.,
        linear_color_stop(rgb(0x58a6ff), 0.),
        linear_color_stop(rgb(0xd2a8ff), 1.),
    ));
    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(labelled(
            "two stops: linear_gradient(90, stop(blue, 0), stop(pink, 1))",
            swatch(two_stops),
        ))
        .child(labelled(
            "three colors: two quads, each one gradient; the seam is invisible because both end on blue",
            div()
                .flex()
                .flex_row()
                .h(px(52.))
                .w_full()
                .rounded(px(6.))
                .overflow_hidden()
                .child(left)
                .child(right),
        ))
        .child(labelled(
            "hard stop: both stops at 50%, which is the only extra control the percentages give",
            swatch(linear_gradient(
                90.,
                linear_color_stop(rgb(0x7ee787), 0.5),
                linear_color_stop(rgb(0xff7b72), 0.5),
            )),
        ))
}

/// Mixes two colors in sRGB; the probe uses it to step the faked radial
/// gradient.
fn mix(from: Rgba, to: Rgba, t: f32) -> Rgba {
    Rgba {
        r: from.r + (to.r - from.r) * t,
        g: from.g + (to.g - from.g) * t,
        b: from.b + (to.b - from.b) * t,
        a: from.a + (to.a - from.a) * t,
    }
}

/// Fills a 48-sided circle, the probe's stand-in for the missing radial
/// gradient path.
fn paint_circle(window: &mut Window, center: Point<Pixels>, radius: Pixels, color: Rgba) {
    const SEGMENTS: usize = 48;
    let mut builder = PathBuilder::fill();
    for index in 0..SEGMENTS {
        let angle = std::f32::consts::TAU * index as f32 / SEGMENTS as f32;
        let vertex = point(
            center.x + px(f32::from(radius) * angle.cos()),
            center.y + px(f32::from(radius) * angle.sin()),
        );
        if index == 0 {
            builder.move_to(vertex);
        } else {
            builder.line_to(vertex);
        }
    }
    builder.close();
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

/// The point at `angle` on a circle, so the fill and stroke paths share one
/// vertex calculation.
fn circle_vertex(center: Point<Pixels>, radius: f32, angle: f32) -> Point<Pixels> {
    point(
        center.x + px(radius * angle.cos()),
        center.y + px(radius * angle.sin()),
    )
}

/// Item 5: a radial gradient faked by painting `steps` circles from the page
/// color at the rim to the accent color in the middle. More steps make the
/// color bands narrower, but they never disappear: the band between two rings
/// still has one flat color.
fn paint_radial_fallback(bounds: Bounds<Pixels>, window: &mut Window, steps: usize) {
    let center = bounds.center();
    let diameter = f32::from(bounds.size.width).min(f32::from(bounds.size.height));
    for step in (0..steps).rev() {
        let t = step as f32 / (steps - 1) as f32;
        let radius = diameter / 2.0 * (step + 1) as f32 / steps as f32;
        paint_circle(window, center, px(radius), mix(rgb(ACCENT), rgb(PAGE), t));
    }
}

/// Item 5: the spinner's ring, stroked as a path instead of filling a disc and
/// covering the middle with the background color. Nothing is painted inside,
/// so the canvas' own background shows through the hole.
fn paint_ring(bounds: Bounds<Pixels>, window: &mut Window) {
    const SEGMENTS: usize = 64;
    let center = bounds.center();
    let stroke = 16.0;
    let radius = (f32::from(bounds.size.width).min(f32::from(bounds.size.height)) - stroke) / 2.0;
    let mut builder = PathBuilder::stroke(px(stroke));
    for index in 0..SEGMENTS {
        let angle = std::f32::consts::TAU * index as f32 / SEGMENTS as f32;
        let vertex = circle_vertex(center, radius, angle);
        if index == 0 {
            builder.move_to(vertex);
        } else {
            builder.line_to(vertex);
        }
    }
    builder.close();
    if let Ok(path) = builder.build() {
        window.paint_path(path, rgb(ACCENT));
    }
}

/// Item 5: the comet spinner's conic-gradient fallback. GPUI has no conic
/// gradient, so the circle is stroked in 48 arcs and each arc gets its own
/// alpha, from transparent at the tail to opaque at the head.
fn paint_comet(bounds: Bounds<Pixels>, window: &mut Window) {
    const SEGMENTS: usize = 48;
    let center = bounds.center();
    let stroke = 16.0;
    let radius = (f32::from(bounds.size.width).min(f32::from(bounds.size.height)) - stroke) / 2.0;
    let color = Hsla::from(rgb(ACCENT));
    for index in 0..SEGMENTS {
        let from = std::f32::consts::TAU * index as f32 / SEGMENTS as f32;
        let to = std::f32::consts::TAU * (index + 1) as f32 / SEGMENTS as f32;
        let mut builder = PathBuilder::stroke(px(stroke));
        builder.move_to(circle_vertex(center, radius, from));
        builder.line_to(circle_vertex(center, radius, to));
        let t = (index + 1) as f32 / SEGMENTS as f32;
        if let Ok(path) = builder.build() {
            window.paint_path(path, color.opacity(t));
        }
    }
}

/// The two faked radial gradients. A radial gradient fades into the page
/// color, so this stage is the page color.
fn radial_discs() -> Div {
    div()
        .flex()
        .flex_row()
        .gap_6()
        .p_4()
        .rounded(px(12.))
        .bg(rgb(PAGE))
        .child(labelled(
            "24 concentric circles: visible color bands",
            canvas(
                |_, _, _| (),
                |bounds, _, window, _| paint_radial_fallback(bounds, window, 24),
            )
            .w(px(160.))
            .h(px(160.)),
        ))
        .child(labelled(
            "96 concentric circles: narrower color bands",
            canvas(
                |_, _, _| (),
                |bounds, _, window, _| paint_radial_fallback(bounds, window, 96),
            )
            .w(px(160.))
            .h(px(160.)),
        ))
}

/// The rings sit on a linear gradient: both are stroked, so the gradient has
/// to show through the hole in the ring and between the comet's arcs.
fn radial_rings() -> Div {
    div()
        .flex()
        .flex_row()
        .gap_6()
        .p_4()
        .rounded(px(12.))
        .bg(linear_gradient(
            90.,
            linear_color_stop(rgb(0x7ee787), 0.),
            linear_color_stop(rgb(0xf778ba), 1.),
        ))
        .child(labelled(
            "ring: PathBuilder::stroke(16), the middle is untouched",
            canvas(
                |_, _, _| (),
                |bounds, _, window, _| paint_ring(bounds, window),
            )
            .w(px(140.))
            .h(px(140.)),
        ))
        .child(labelled(
            "comet: 48 stroked arcs, alpha 0 to 1, the conic-gradient fallback",
            canvas(
                |_, _, _| (),
                |bounds, _, window, _| paint_comet(bounds, window),
            )
            .w(px(140.))
            .h(px(140.)),
        ))
}

/// Item 5: both fallbacks, one stage per background.
fn radial_gradient_fallbacks() -> Div {
    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(radial_discs())
        .child(radial_rings())
}

/// Paints one line through `ShapedLine::paint`. This is the reference the
/// other rows are compared against.
fn paint_whole_line(bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App, text: &'static str) {
    let font = Font::default();
    let font_size = px(20.);
    let line_height = px(30.);
    let color = Hsla::from(rgb(TEXT));
    let run = text_run(&font, color, text.len());
    let line = window
        .text_system()
        .shape_line(SharedString::from(text), font_size, &[run], None);
    let _ = line.paint(
        bounds.origin,
        line_height,
        TextAlign::Left,
        None,
        window,
        cx,
    );
}

/// Item 6's fallback: shape the line once, then paint each decoded glyph with
/// `window.paint_glyph` at its shaped position plus `spacing * glyph_index`.
/// Shaping the whole line keeps the font's kerning; shaping one character at a
/// time loses it, which is why the old per-character control line ended short.
fn paint_spaced_line(
    bounds: Bounds<Pixels>,
    window: &mut Window,
    text: &'static str,
    spacing: Pixels,
) {
    let font = Font::default();
    let font_size = px(20.);
    let line_height = px(30.);
    let color = Hsla::from(rgb(TEXT));
    let run = text_run(&font, color, text.len());
    let line = window
        .text_system()
        .shape_line(SharedString::from(text), font_size, &[run], None);
    // The baseline `ShapedLine::paint` uses: the line box centers the font's
    // ascent and descent around the glyphs.
    let padding_top = (line_height - line.ascent - line.descent) / 2.;
    let baseline = bounds.origin.y + padding_top + line.ascent;
    let mut glyph_index = 0;
    for run in &line.runs {
        for glyph in &run.glyphs {
            let origin = point(
                bounds.origin.x + glyph.position.x + spacing * glyph_index,
                baseline + glyph.position.y,
            );
            let _ = if glyph.is_emoji {
                window.paint_emoji(origin, run.font_id, glyph.id, line.font_size)
            } else {
                window.paint_glyph(origin, run.font_id, glyph.id, line.font_size, color)
            };
            glyph_index += 1;
        }
    }
}

/// One text run covering `len` bytes.
fn text_run(font: &Font, color: Hsla, len: usize) -> TextRun {
    TextRun {
        len,
        font: font.clone(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    }
}

/// Item 6: the whole line, the same line shaped once and moved 3 px per glyph,
/// and the same glyph loop with 0 px as the control.
fn letter_spacing() -> Div {
    const TEXT: &str = "tTerm GPUI spacing";
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(labelled(
            "one shape_line painted by ShapedLine::paint: the reference",
            canvas(
                |_, _, _| (),
                |bounds, _, window, cx| paint_whole_line(bounds, window, cx, TEXT),
            )
            .h(px(30.))
            .w_full(),
        ))
        .child(labelled(
            "one shape_line, each glyph after the first moved 3 px: kerning is kept",
            canvas(
                |_, _, _| (),
                |bounds, _, window, _| paint_spaced_line(bounds, window, TEXT, px(3.)),
            )
            .h(px(30.))
            .w_full(),
        ))
        .child(labelled(
            "control: the same glyph loop with 0 px, which must match the first line pixel for pixel",
            canvas(
                |_, _, _| (),
                |bounds, _, window, _| paint_spaced_line(bounds, window, TEXT, px(0.)),
            )
            .h(px(30.))
            .w_full(),
        ))
}

/// One card of item 7 carrying `shadows`.
fn shadow_card(label: &'static str, shadows: Vec<BoxShadow>) -> Div {
    div()
        .w(px(150.))
        .h(px(96.))
        .rounded(px(12.))
        .bg(rgb(PANEL))
        .border_1()
        .border_color(rgb(BORDER))
        .shadow(shadows)
        .flex()
        .items_center()
        .justify_center()
        .child(div().text_xs().text_color(rgb(MUTED)).child(label))
}

/// Item 7: one, two and three drop shadows plus one inset shadow.
fn shadows() -> Div {
    let one = vec![BoxShadow::new(px(0.), px(1.), rgba(0x00000099).into()).blur_radius(px(4.))];
    let two = vec![
        BoxShadow::new(px(0.), px(1.), rgba(0x00000099).into()).blur_radius(px(2.)),
        BoxShadow::new(px(0.), px(4.), rgba(0x00000080).into()).blur_radius(px(12.)),
    ];
    let three = vec![
        BoxShadow::new(px(0.), px(1.), rgba(0x00000099).into()).blur_radius(px(2.)),
        BoxShadow::new(px(0.), px(8.), rgba(0x00000080).into()).blur_radius(px(16.)),
        BoxShadow::new(px(0.), px(24.), rgba(0x00000059).into()).blur_radius(px(32.)),
    ];
    let inset = vec![
        BoxShadow::new(px(0.), px(1.), rgba(0x00000099).into())
            .blur_radius(px(4.))
            .inset(),
    ];
    // A mid-gray band: a black shadow on the near-black page is invisible in
    // a screenshot, so the layering could not be compared by eye.
    div()
        .flex()
        .flex_row()
        .gap_6()
        .p_8()
        .items_center()
        .rounded(px(10.))
        .bg(rgb(0x2d333b))
        .child(shadow_card("1 layer", one))
        .child(shadow_card("2 layers", two))
        .child(shadow_card("3 layers", three))
        .child(shadow_card("inset", inset))
}

/// The three-color SVG the tinting probe uses.
const TWO_COLOR_SVG: &[u8] = include_bytes!("assets/p0_25_two_color.svg");

/// One tinted copy of [`TWO_COLOR_SVG`].
fn svg_tint(label: &'static str, color: Rgba) -> Div {
    labelled(
        label,
        svg()
            .data(TWO_COLOR_SVG)
            .w(px(96.))
            .h(px(96.))
            .text_color(Hsla::from(color)),
    )
}

/// Item 8: the same file under three colors. The file itself has three fill
/// colors; every copy comes out flat because only the alpha mask survives.
fn svg_tints() -> Div {
    div()
        .flex()
        .flex_row()
        .gap_8()
        .items_center()
        .child(svg_tint("text_color = blue", rgb(ACCENT)))
        .child(svg_tint("text_color = red", rgb(0xff7b72)))
        .child(svg_tint("text_color = green", rgb(0x7ee787)))
}
