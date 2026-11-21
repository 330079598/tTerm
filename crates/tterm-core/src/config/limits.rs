//! Settings bounds and choices shared with `src/lib/appConfig.ts`.

use std::ops::RangeInclusive;

/// Built-in notification sounds and the custom-file option.
pub const NOTIFICATION_SOUND_NAMES: [&str; 6] =
    ["system", "chime", "ding", "pop", "rise", "custom"];
/// Renderer id retained in the shared configuration format.
pub const DEFAULT_TERMINAL_RENDERER: &str = "webgl";
/// Supported idle delays for hidden renderers, in seconds.
pub const HIDDEN_RENDERER_RELEASE_SECS_OPTIONS: [u32; 5] = [0, 10, 30, 60, 300];
/// Default idle delay for hidden renderers.
pub const DEFAULT_HIDDEN_RENDERER_RELEASE_SECS: u32 = 10;
/// Font size used on first launch.
pub const DEFAULT_TERMINAL_FONT_SIZE: u16 = 14;
/// Default delay before enabling predictive echo.
pub const DEFAULT_PREDICTIVE_ECHO_THRESHOLD_MS: u16 = 30;
/// Shortest predictive echo delay in milliseconds.
pub const PREDICTIVE_ECHO_THRESHOLD_MIN_MS: u16 = 10;
/// Longest predictive echo delay in milliseconds.
pub const PREDICTIVE_ECHO_THRESHOLD_MAX_MS: u16 = 2_000;
/// Longest log retention in days.
pub const TERMINAL_LOG_MAX_RETENTION_DAYS: u32 = 3_650;
/// Maximum size of retained logs in MiB.
pub const TERMINAL_LOG_MAX_TOTAL_MB: u32 = 1024 * 1024;
/// Maximum SFTP rate in KiB per second.
pub const MAX_BANDWIDTH_LIMIT_KIB: u32 = 10 * 1024 * 1024;
/// Minimum configured reconnect attempts.
pub const RECONNECT_MAX_ATTEMPTS_MIN: u32 = 1;
/// Maximum configured reconnect attempts.
pub const RECONNECT_MAX_ATTEMPTS_MAX: u32 = 99;
/// Accepted font sizes.
pub const TERMINAL_FONT_SIZE_RANGE: RangeInclusive<u16> = 6..=72;
/// Accepted line-height multipliers.
pub const TERMINAL_LINE_HEIGHT_RANGE: RangeInclusive<f64> = 1.0..=2.0;
/// Accepted letter spacing in pixels.
pub const TERMINAL_LETTER_SPACING_RANGE: RangeInclusive<i16> = -5..=10;
/// Accepted translucent window opacity percentages.
pub const WINDOW_OPACITY_PERCENT_RANGE: RangeInclusive<u8> = 30..=95;
/// Accepted window blur radii.
pub const WINDOW_BLUR_RADIUS_RANGE: RangeInclusive<u8> = 1..=60;
/// Accepted minimum command duration for notifications, in seconds.
pub const NOTIFY_COMMAND_MIN_SECS_RANGE: RangeInclusive<u16> = 1..=3600;
/// Accepted number of simultaneous visible toasts.
pub const TOAST_MAX_VISIBLE_RANGE: RangeInclusive<u8> = 1..=5;
