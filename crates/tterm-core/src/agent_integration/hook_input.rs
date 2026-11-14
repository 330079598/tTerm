//! Portable stdin metadata parsing for hooks, with no additional host runtime.

/// Reads the hook JSON once. Only validated, encoded fields enter the OSC payload.
pub(super) fn script() -> String {
    let parser = include_str!("hook_input.awk");
    format!(
        "input=$(cat); fields=$(printf '%s' \"$input\" | LC_ALL=C awk '{parser}'); sid=${{fields%%;*}}; cwd=${{fields#*;}}"
    )
}

#[cfg(test)]
mod tests;
