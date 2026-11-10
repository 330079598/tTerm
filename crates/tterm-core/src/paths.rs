//! Application directories resolved by the client at startup.

use std::path::PathBuf;

/// Paths shared by domain services, without a dependency on a UI runtime.
/// Resolving paths does not create directories or open persisted data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    /// Local data differs from roaming data on Windows.
    pub local_data_dir: PathBuf,
    pub cache_dir: PathBuf,
    /// Application diagnostics; terminal recordings still live in config/logs
    /// unless the user chooses another directory.
    pub log_dir: PathBuf,
    /// A user directory that may be unavailable, particularly on Linux.
    pub download_dir: Option<PathBuf>,
    pub temp_dir: PathBuf,
}

impl AppPaths {
    /// Resolves native desktop paths using the same directory library and
    /// identifier suffixes as Tauri's default desktop path resolver.
    pub fn native(identifier: &str) -> Result<Self, String> {
        let resolve = |directory: Option<PathBuf>, name: &str| {
            directory
                .map(|base| base.join(identifier))
                .ok_or_else(|| format!("Failed to resolve app {name} directory"))
        };
        let local_data_dir = resolve(dirs::data_local_dir(), "local data")?;
        #[cfg(target_os = "macos")]
        let log_dir = dirs::home_dir()
            .map(|home| home.join("Library/Logs").join(identifier))
            .ok_or("Failed to resolve app log directory")?;
        #[cfg(not(target_os = "macos"))]
        let log_dir = local_data_dir.join("logs");

        Ok(Self {
            config_dir: resolve(dirs::config_dir(), "config")?,
            data_dir: resolve(dirs::data_dir(), "data")?,
            local_data_dir,
            cache_dir: resolve(dirs::cache_dir(), "cache")?,
            log_dir,
            download_dir: dirs::download_dir(),
            temp_dir: std::env::temp_dir(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_preserve_case_and_platform_layout() {
        for identifier in [
            "com.stone.tTerm",
            "com.stone.tTerm.dev",
            "com.stone.tTerm.gpui-dev",
        ] {
            let paths = AppPaths::native(identifier).expect("resolve paths");
            #[cfg(target_os = "macos")]
            {
                let library = dirs::home_dir().unwrap().join("Library");
                assert_eq!(
                    paths.config_dir,
                    library.join("Application Support").join(identifier)
                );
                assert_eq!(paths.data_dir, paths.config_dir);
                assert_eq!(paths.local_data_dir, paths.config_dir);
                assert_eq!(paths.cache_dir, library.join("Caches").join(identifier));
                assert_eq!(paths.log_dir, library.join("Logs").join(identifier));
            }
            #[cfg(target_os = "windows")]
            {
                assert_eq!(
                    paths.config_dir,
                    dirs::config_dir().unwrap().join(identifier)
                );
                assert_eq!(paths.data_dir, dirs::data_dir().unwrap().join(identifier));
                assert_eq!(
                    paths.local_data_dir,
                    dirs::data_local_dir().unwrap().join(identifier)
                );
                assert_eq!(paths.cache_dir, paths.local_data_dir);
                assert_eq!(paths.log_dir, paths.local_data_dir.join("logs"));
            }
            assert_eq!(paths.temp_dir, std::env::temp_dir());
        }
    }

    // Each environment is given its own process so parallel tests never race
    // over HOME or XDG variables in the test runner.
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_xdg_absolute_relative_and_unset() {
        for mode in ["absolute", "relative", "unset"] {
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command
                .args(["--exact", "paths::tests::linux_xdg_child"])
                .env("TTERM_PATH_TEST_MODE", mode)
                .env("HOME", "/tmp/tterm-path-test-home");
            for (key, suffix) in [
                ("XDG_CONFIG_HOME", "config"),
                ("XDG_DATA_HOME", "data"),
                ("XDG_CACHE_HOME", "cache"),
            ] {
                match mode {
                    "absolute" => {
                        command.env(key, format!("/tmp/tterm-xdg/{suffix}"));
                    }
                    "relative" => {
                        command.env(key, suffix);
                    }
                    _ => {
                        command.env_remove(key);
                    }
                }
            }
            assert!(command.status().unwrap().success(), "mode: {mode}");
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_xdg_child() {
        let Ok(mode) = std::env::var("TTERM_PATH_TEST_MODE") else {
            return;
        };
        let paths = AppPaths::native("com.stone.tTerm.gpui-dev").unwrap();
        let base = PathBuf::from("/tmp/tterm-path-test-home");
        let (config, data, cache) = if mode == "absolute" {
            (
                PathBuf::from("/tmp/tterm-xdg/config"),
                PathBuf::from("/tmp/tterm-xdg/data"),
                PathBuf::from("/tmp/tterm-xdg/cache"),
            )
        } else {
            (
                base.join(".config"),
                base.join(".local/share"),
                base.join(".cache"),
            )
        };
        assert_eq!(paths.config_dir, config.join("com.stone.tTerm.gpui-dev"));
        assert_eq!(paths.data_dir, data.join("com.stone.tTerm.gpui-dev"));
        assert_eq!(paths.local_data_dir, paths.data_dir);
        assert_eq!(paths.cache_dir, cache.join("com.stone.tTerm.gpui-dev"));
        assert_eq!(paths.log_dir, paths.local_data_dir.join("logs"));
    }
}
