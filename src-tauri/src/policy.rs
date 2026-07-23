use rand::Rng;
use std::time::Duration;
use tokio::time::sleep;

/// Pace launches so 8–10 instances look like sequential human starts.
pub struct LaunchPolicy {
    min_ms: u64,
    max_ms: u64,
    first: bool,
}

impl LaunchPolicy {
    pub fn new(min_ms: u64, max_ms: u64) -> Self {
        let (min_ms, max_ms) = if min_ms <= max_ms {
            (min_ms, max_ms)
        } else {
            (max_ms, min_ms)
        };
        Self {
            min_ms,
            max_ms,
            first: true,
        }
    }

    pub async fn wait_before_next(&mut self) {
        if self.first {
            self.first = false;
            return;
        }
        // Compute delay before await so the future stays Send (ThreadRng is !Send).
        let delay = {
            let mut rng = rand::thread_rng();
            rng.gen_range(self.min_ms..=self.max_ms)
        };
        sleep(Duration::from_millis(delay)).await;
    }
}

pub fn clamp_count(count: u8) -> u8 {
    count.clamp(1, crate::models::MAX_SLOTS)
}
