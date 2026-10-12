//! Sizing of the Tokio runtime the app's async work runs on.

fn tokio_worker_threads_from(
    override_value: Option<&str>,
    available_parallelism: Option<usize>,
) -> usize {
    const MIN_WORKERS: usize = 2;
    const DEFAULT_MAX_WORKERS: usize = 8;
    const ENV_MAX_WORKERS: usize = 16;

    if let Some(raw) = override_value {
        if let Ok(parsed) = raw.trim().parse::<usize>() {
            return parsed.clamp(MIN_WORKERS, ENV_MAX_WORKERS);
        }
    }

    available_parallelism
        .map(|n| n.clamp(MIN_WORKERS, DEFAULT_MAX_WORKERS))
        .unwrap_or(MIN_WORKERS)
}

/// Adaptive Tokio worker count for desktop workloads (SSH/SFTP concurrency).
/// Override with `TTERM_TOKIO_WORKERS` (clamped to 2-16).
pub fn tokio_worker_threads() -> usize {
    let available_parallelism = std::thread::available_parallelism().ok().map(|n| n.get());
    tokio_worker_threads_from(
        std::env::var("TTERM_TOKIO_WORKERS").ok().as_deref(),
        available_parallelism,
    )
}

#[cfg(test)]
mod tests {
    use super::tokio_worker_threads_from;

    #[test]
    fn worker_count_uses_bounded_available_parallelism() {
        assert_eq!(tokio_worker_threads_from(None, None), 2);
        assert_eq!(tokio_worker_threads_from(None, Some(1)), 2);
        assert_eq!(tokio_worker_threads_from(None, Some(6)), 6);
        assert_eq!(tokio_worker_threads_from(None, Some(64)), 8);
    }

    #[test]
    fn worker_count_honors_a_valid_bounded_override() {
        assert_eq!(tokio_worker_threads_from(Some("6"), Some(2)), 6);
        assert_eq!(tokio_worker_threads_from(Some("1"), Some(8)), 2);
        assert_eq!(tokio_worker_threads_from(Some("99"), Some(2)), 16);
    }

    #[test]
    fn invalid_override_falls_back_to_available_parallelism() {
        assert_eq!(tokio_worker_threads_from(Some("invalid"), Some(4)), 4);
    }
}
