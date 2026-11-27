//! Measures the six full-screen images from `scripts/parity/first-screen.mjs`.
//! Passing this necessary check does not certify layout, colors or masked SSIM.

use std::process::ExitCode;
use tterm_parity::compare::FULL_SSIM_MIN;
use tterm_parity::compare::image_diff::{heatmap, ssim};

fn main() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let root = tterm_parity::repo_root().join("target/parity/first-screen");
    let mut passed = true;
    for theme in ["dark", "light"] {
        for screen in [
            "workspace.first-screen",
            "profiles.list.first-screen",
            "settings.terminal.font",
        ] {
            let file = format!("{screen}.png");
            let reference =
                image::open(root.join("reference").join(theme).join(&file))?.into_rgba8();
            let actual = image::open(root.join("gpui").join(theme).join(&file))?.into_rgba8();
            if reference.dimensions() != (1600, 1200) || actual.dimensions() != (1600, 1200) {
                return Err(format!("{theme}/{file}: expected two 1600×1200 images").into());
            }
            let score = ssim(&reference, &actual, None);
            println!("{theme} {screen} {score:.6}");
            if score < FULL_SSIM_MIN {
                let output = root.join("heatmaps").join(theme);
                std::fs::create_dir_all(&output)?;
                heatmap(&reference, &actual).save(output.join(&file))?;
                passed = false;
            }
        }
    }
    Ok(if passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
