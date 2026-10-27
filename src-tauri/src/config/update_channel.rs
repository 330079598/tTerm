//! Which update channel a first launch follows.

/// A prerelease build follows the beta channel from its first launch, as the
/// frontend's default does, so it keeps receiving prereleases.
pub(super) fn default_update_channel() -> String {
    update_channel_for_version(env!("CARGO_PKG_VERSION")).to_string()
}

/// "beta-dev" for a version with an alpha, beta, rc or dev prerelease part
/// (`0.8.0-beta.1`), otherwise "stable".
fn update_channel_for_version(version: &str) -> &'static str {
    let prerelease = version.split('-').skip(1).any(|part| {
        ["alpha", "beta", "rc", "dev"].iter().any(|tag| {
            part.strip_prefix(tag)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('.'))
        })
    });
    if prerelease {
        "beta-dev"
    } else {
        "stable"
    }
}

#[cfg(test)]
mod tests {
    use super::update_channel_for_version;

    #[test]
    fn prerelease_versions_follow_the_beta_channel() {
        assert_eq!(update_channel_for_version("0.7.1"), "stable");
        assert_eq!(update_channel_for_version("0.8.0-beta.1"), "beta-dev");
        assert_eq!(update_channel_for_version("0.8.0-rc"), "beta-dev");
        assert_eq!(update_channel_for_version("0.8.0-dev.3"), "beta-dev");
        assert_eq!(update_channel_for_version("0.8.0-betamax"), "stable");
    }
}
