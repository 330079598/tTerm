//! Token-bucket bandwidth limiter.
//!
//! One limiter is shared by every transfer of a direction, so the configured
//! rate caps the sum of all lanes of all concurrent transfers — the point of
//! a limit is to leave the rest of the link to interactive sessions and to
//! whatever else the server is serving, which a per-transfer cap would not do
//! as soon as two transfers run side by side.
//!
//! Tokens are taken *before* a request is sent, so a throttled request waits
//! locally instead of sitting on the wire where it would run into the SFTP
//! request timeout. Waits are sliced (see [`MAX_WAIT_SLICE`]) so a changed
//! rate and a cancel both take effect promptly on transfers already running.
//!
//! Callers split their I/O into requests of at most [`RateLimiter::max_request`]
//! bytes. A request is then only sent once the bucket holds all of its bytes,
//! so the bytes moved never run ahead of `bucket + rate × elapsed`. Letting a
//! larger request through on credit and paying afterwards keeps the long-run
//! average right, but every transfer starts with one request sent for free —
//! at 30 KiB/s with 255 KiB writes that showed as ~40 KiB/s for minutes.

use std::sync::Mutex;
use std::time::Duration;

use tokio::sync::watch;
use tokio::time::Instant;

/// Tokens a full bucket holds, as time at the configured rate. Small enough
/// that an idle limiter cannot release a burst that noticeably exceeds the
/// rate, large enough that small requests are not throttled one by one.
const BURST: Duration = Duration::from_millis(250);

/// Longest single sleep before the bucket is re-checked.
const MAX_WAIT_SLICE: Duration = Duration::from_millis(100);

/// Smallest bucket, and so the smallest request size at very low rates:
/// below this the per-request SFTP framing would dominate the payload.
const MIN_BUCKET_BYTES: f64 = 1024.0;

#[derive(Debug)]
struct Bucket {
    /// 0 means unlimited.
    bytes_per_sec: u64,
    /// May go negative, but only for a caller that ignores
    /// [`RateLimiter::max_request`]: an oversized request is let through once
    /// the bucket is full and the overdraft is paid back by later requests.
    available: f64,
    last_refill: Instant,
}

impl Bucket {
    fn capacity(&self) -> f64 {
        (self.bytes_per_sec as f64 * BURST.as_secs_f64()).max(MIN_BUCKET_BYTES)
    }

    fn refill(&mut self, now: Instant) {
        let elapsed = now
            .saturating_duration_since(self.last_refill)
            .as_secs_f64();
        self.last_refill = now;
        self.available =
            (self.available + elapsed * self.bytes_per_sec as f64).min(self.capacity());
    }
}

#[derive(Debug)]
pub struct RateLimiter {
    bucket: Mutex<Bucket>,
}

impl RateLimiter {
    /// A limiter at `bytes_per_sec`; 0 means unlimited.
    pub fn new(bytes_per_sec: u64) -> Self {
        let mut bucket = Bucket {
            bytes_per_sec,
            available: 0.0,
            last_refill: Instant::now(),
        };
        bucket.available = bucket.capacity();
        Self {
            bucket: Mutex::new(bucket),
        }
    }

    #[cfg(test)]
    pub fn rate(&self) -> u64 {
        self.bucket.lock().expect("rate limiter lock").bytes_per_sec
    }

    /// Largest request that can be sent without running ahead of the rate
    /// (the bucket size), or `None` when unlimited. Read per request, so a
    /// changed rate also changes the request size of running transfers.
    pub fn max_request(&self) -> Option<u64> {
        let bucket = self.bucket.lock().expect("rate limiter lock");
        (bucket.bytes_per_sec != 0).then(|| bucket.capacity() as u64)
    }

    /// Changes the rate for every transfer, including those in flight.
    pub fn set_rate(&self, bytes_per_sec: u64) {
        let mut bucket = self.bucket.lock().expect("rate limiter lock");
        if bucket.bytes_per_sec == bytes_per_sec {
            return;
        }
        let now = Instant::now();
        if bucket.bytes_per_sec != 0 {
            // Settle what accrued at the old rate before switching.
            bucket.refill(now);
        }
        let was_unlimited = bucket.bytes_per_sec == 0;
        bucket.bytes_per_sec = bytes_per_sec;
        bucket.last_refill = now;
        bucket.available = if was_unlimited {
            bucket.capacity()
        } else {
            // A lower rate must not inherit a bucket sized for the higher one;
            // an overdraft is carried over and paid back at the new rate.
            bucket.available.min(bucket.capacity())
        };
    }

    /// Waits until `bytes` may be sent. Returns `false` when `cancel` fires
    /// first.
    pub async fn acquire(&self, bytes: u64, cancel: &watch::Receiver<bool>) -> bool {
        let mut cancel = cancel.clone();
        loop {
            let wait = {
                let mut bucket = self.bucket.lock().expect("rate limiter lock");
                if bucket.bytes_per_sec == 0 {
                    return true;
                }
                bucket.refill(Instant::now());
                // A request within `max_request` waits for all of its bytes. A
                // larger one could never fit: it waits for a full bucket and
                // the rest becomes an overdraft (see `available`).
                let needed = (bytes as f64).min(bucket.capacity());
                if bucket.available >= needed {
                    bucket.available -= bytes as f64;
                    return true;
                }
                Duration::from_secs_f64((needed - bucket.available) / bucket.bytes_per_sec as f64)
                    .min(MAX_WAIT_SLICE)
            };

            if *cancel.borrow() {
                return false;
            }
            tokio::select! {
                _ = tokio::time::sleep(wait) => {}
                changed = cancel.changed() => {
                    // A dropped sender cannot cancel any more: keep waiting on
                    // the clock alone.
                    if changed.is_err() {
                        tokio::time::sleep(wait).await;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn never_cancelled() -> (watch::Sender<bool>, watch::Receiver<bool>) {
        watch::channel(false)
    }

    /// Sends `total` bytes in requests of at most `step` bytes, split further
    /// to [`RateLimiter::max_request`] the way the transfer engine does, and
    /// returns the elapsed (virtual) time.
    async fn send(limiter: &RateLimiter, total: u64, step: u64) -> Duration {
        let (_tx, rx) = never_cancelled();
        let start = Instant::now();
        let mut sent = 0;
        while sent < total {
            let want = step
                .min(limiter.max_request().unwrap_or(u64::MAX))
                .min(total - sent);
            assert!(limiter.acquire(want, &rx).await);
            sent += want;
        }
        start.elapsed()
    }

    #[tokio::test(start_paused = true)]
    async fn unlimited_never_waits() {
        let limiter = RateLimiter::new(0);
        assert_eq!(limiter.max_request(), None);
        assert_eq!(
            send(&limiter, 100 * 1024 * 1024, 255 * 1024).await,
            Duration::ZERO
        );
    }

    #[tokio::test(start_paused = true)]
    async fn throughput_matches_the_rate() {
        // 100 KiB/s, 1 MiB in wire-sized requests larger than the bucket.
        let limiter = RateLimiter::new(100 * 1024);
        assert_eq!(limiter.max_request(), Some(25 * 1024));
        let elapsed = send(&limiter, 1024 * 1024, 255 * 1024).await;
        // 1024 KiB at 100 KiB/s is 10.24 s; only the first full bucket
        // (0.25 s worth) is free.
        let secs = elapsed.as_secs_f64();
        assert!((9.95..=10.05).contains(&secs), "took {secs} s");

        // Sustained, with the bucket drained: exactly the rate.
        let elapsed = send(&limiter, 1024 * 1024, 255 * 1024).await;
        let secs = elapsed.as_secs_f64();
        assert!((10.2..=10.3).contains(&secs), "took {secs} s");
    }

    /// The reported average — bytes so far ÷ time so far — must never exceed
    /// the rate by more than one bucket, from the very first request.
    #[tokio::test(start_paused = true)]
    async fn bytes_never_run_ahead_of_the_budget() {
        let rate = 30 * 1024u64;
        let limiter = RateLimiter::new(rate);
        let bucket = limiter.max_request().unwrap();
        let (_tx, rx) = never_cancelled();
        let start = Instant::now();
        let mut sent = 0u64;
        while sent < 1020 * 1024 {
            let want = (255 * 1024u64).min(bucket);
            assert!(limiter.acquire(want, &rx).await);
            sent += want;
            let budget = bucket as f64 + rate as f64 * start.elapsed().as_secs_f64();
            assert!(sent as f64 <= budget + 1.0, "{sent} B > {budget:.0} B");
        }
        // 1020 KiB at 30 KiB/s: ~34 s, not the ~25.5 s that showed 40 KiB/s.
        let secs = start.elapsed().as_secs_f64();
        assert!((33.5..=34.1).contains(&secs), "took {secs} s");
    }

    #[tokio::test(start_paused = true)]
    async fn concurrent_requests_share_one_budget() {
        let limiter = std::sync::Arc::new(RateLimiter::new(1024 * 1024));
        let start = Instant::now();
        let mut tasks = tokio::task::JoinSet::new();
        for _ in 0..8 {
            let limiter = limiter.clone();
            tasks.spawn(async move { send(&limiter, 1024 * 1024, 64 * 1024).await });
        }
        while tasks.join_next().await.is_some() {}
        // 8 MiB at 1 MiB/s, whatever the number of lanes.
        let secs = start.elapsed().as_secs_f64();
        assert!((7.5..=8.5).contains(&secs), "took {secs} s");
    }

    #[tokio::test(start_paused = true)]
    async fn a_rate_change_applies_to_a_waiting_request() {
        let limiter = std::sync::Arc::new(RateLimiter::new(1024));
        let (_tx, rx) = never_cancelled();
        // Drain the bucket; the next request then has to wait for a full
        // bucket at 1 KiB/s, and its 1 MiB overdraft would stall whoever
        // comes after it for ~17 minutes.
        assert!(limiter.acquire(256, &rx).await);
        let waiter = {
            let limiter = limiter.clone();
            let rx = rx.clone();
            tokio::spawn(async move {
                let start = Instant::now();
                assert!(limiter.acquire(1024 * 1024, &rx).await);
                start.elapsed()
            })
        };
        tokio::time::sleep(Duration::from_millis(50)).await;
        limiter.set_rate(0);
        let waited = waiter.await.unwrap();
        assert!(waited <= Duration::from_millis(200), "waited {waited:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn cancel_interrupts_a_wait() {
        let limiter = RateLimiter::new(1);
        let (tx, rx) = watch::channel(false);
        assert!(limiter.acquire(1, &rx).await);
        let cancel = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(30)).await;
            tx.send(true).unwrap();
            tx
        });
        let start = Instant::now();
        assert!(!limiter.acquire(1024, &rx).await);
        assert!(start.elapsed() <= Duration::from_millis(50));
        drop(cancel.await.unwrap());
    }

    #[tokio::test(start_paused = true)]
    async fn lowering_the_rate_shrinks_a_full_bucket() {
        // A full 1 MiB/s bucket holds 256 KiB; at 1 KiB/s it may only hold
        // the 1 KiB minimum.
        let limiter = RateLimiter::new(1024 * 1024);
        limiter.set_rate(1024);
        assert_eq!(limiter.rate(), 1024);
        assert_eq!(limiter.max_request(), Some(1024));
        let elapsed = send(&limiter, 2048, 256).await;
        let secs = elapsed.as_secs_f64();
        assert!((0.9..=1.1).contains(&secs), "took {secs} s");
    }
}
