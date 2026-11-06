//! The HTML summary. It is deliberately self-contained and timestamp-free so
//! two runs over the same input produce the same file.

use super::{Report, ScreenReport};

/// Renders the report to a single HTML document.
pub fn render(report: &Report) -> String {
    let mut html = String::from(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\
         <title>tTerm parity report</title><style>\
         body{font:14px/1.5 -apple-system,BlinkMacSystemFont,\"Segoe UI\",sans-serif;\
         margin:32px;color:#1f2328;background:#fff}\
         h1{font-size:22px;margin:0 0 8px}h2{font-size:17px;margin:24px 0 8px}\
         .pass{color:#1a7f37}.fail{color:#cf222e}\
         .card{border:1px solid #d0d7de;border-radius:8px;padding:16px;margin:16px 0}\
         .images{display:flex;gap:16px;flex-wrap:wrap}\
         figure{margin:0}img{max-width:520px;border:1px solid #d0d7de;border-radius:4px}\
         figcaption{color:#57606a;font-size:12px;margin-top:4px}\
         table{border-collapse:collapse;width:100%;margin-top:12px}\
         th,td{border:1px solid #d0d7de;padding:4px 8px;text-align:left;vertical-align:top}\
         th{background:#f6f8fa}code{font-family:ui-monospace,SFMono-Regular,monospace}\
         .failure{color:#cf222e;font-family:ui-monospace,SFMono-Regular,monospace;font-size:12px}\
         </style></head><body>\n",
    );
    html.push_str("<h1>tTerm UI parity report</h1>\n");
    html.push_str(&format!(
        "<p>Reference frontend commit: <code>{}</code>. Thresholds: fixed ≤ {} px, \
         text ≤ max({} px, {}%), masked SSIM ≥ {}, full SSIM ≥ {}.</p>\n",
        escape(&report.reference.frontend_commit),
        report.thresholds.fixed_px,
        report.thresholds.text_px,
        report.thresholds.text_fraction * 100.0,
        report.thresholds.masked_ssim,
        report.thresholds.full_ssim
    ));
    html.push_str(&format!(
        "<p class=\"{}\"><strong>Overall: {}</strong></p>\n",
        if report.passed { "pass" } else { "fail" },
        if report.passed { "PASS" } else { "FAIL" }
    ));
    for screen in &report.screens {
        render_screen(&mut html, screen);
    }
    html.push_str("</body></html>\n");
    html
}

fn render_screen(html: &mut String, screen: &ScreenReport) {
    html.push_str(&format!(
        "<section class=\"card\"><h2>{} <span class=\"{}\">{}</span></h2>\n",
        escape(&screen.id),
        if screen.passed { "pass" } else { "fail" },
        if screen.passed { "PASS" } else { "FAIL" }
    ));
    html.push_str(&format!(
        "<p>{}<br>SSIM full <code>{:.6}</code>, masked <code>{:.6}</code>. \
         Colour ΔE<sub>00</sub> mean <code>{:.4}</code>, max <code>{:.4}</code>, \
         pixels above 1: <code>{}</code>.</p>\n",
        escape(&screen.title),
        screen.ssim.full,
        screen.ssim.masked,
        screen.colors.mean_delta_e,
        screen.colors.max_delta_e,
        screen.colors.pixels_above_one
    ));
    html.push_str("<div class=\"images\">\n");
    html.push_str(&figure(&screen.images.reference, "reference (WebKit)"));
    html.push_str(&figure(&screen.images.actual, "actual (GPUI)"));
    if !screen.heatmap.is_empty() {
        html.push_str(&figure(&screen.heatmap, "difference heatmap"));
    }
    html.push_str("</div>\n");

    if screen.layout.elements.is_empty() {
        html.push_str("<p class=\"failure\">No elements were recorded.</p>\n");
    } else {
        html.push_str(
            "<table><thead><tr><th>element</th><th>max bounds Δ</th><th>result</th>\
             <th>failures</th></tr></thead><tbody>\n",
        );
        for element in &screen.layout.elements {
            let failures = if element.failures.is_empty() {
                String::new()
            } else {
                element
                    .failures
                    .iter()
                    .map(|failure| escape(failure))
                    .collect::<Vec<_>>()
                    .join("<br>")
            };
            html.push_str(&format!(
                "<tr><td><code>{}</code></td><td>{:.2} px</td><td class=\"{}\">{}</td>\
                 <td class=\"failure\">{}</td></tr>\n",
                escape(&element.id),
                element.max_bounds_delta,
                if element.passed { "pass" } else { "fail" },
                if element.passed { "pass" } else { "fail" },
                failures
            ));
        }
        html.push_str("</tbody></table>\n");
    }
    if !screen.failures.is_empty() {
        html.push_str("<p class=\"failure\">");
        html.push_str(
            &screen
                .failures
                .iter()
                .map(|failure| escape(failure))
                .collect::<Vec<_>>()
                .join("<br>"),
        );
        html.push_str("</p>\n");
    }
    html.push_str("</section>\n");
}

fn figure(source: &str, caption: &str) -> String {
    format!(
        "<figure><img src=\"{}\" alt=\"{}\"><figcaption>{}</figcaption></figure>\n",
        escape(source),
        escape(caption),
        escape(caption)
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
