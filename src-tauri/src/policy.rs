use crate::models::{AccountTier, TierPreset};
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
        self.wait_range(self.min_ms, self.max_ms).await;
    }

    pub async fn wait_for_tier(&mut self, tier: AccountTier) {
        let (min_ms, max_ms) = tier.delay_range();
        self.wait_range(min_ms, max_ms).await;
    }

    async fn wait_range(&mut self, min_ms: u64, max_ms: u64) {
        if self.first {
            self.first = false;
            return;
        }
        let (lo, hi) = if min_ms <= max_ms {
            (min_ms, max_ms)
        } else {
            (max_ms, min_ms)
        };
        let delay = {
            let mut rng = rand::thread_rng();
            rng.gen_range(lo..=hi)
        };
        sleep(Duration::from_millis(delay)).await;
    }
}

pub fn clamp_count(count: u8) -> u8 {
    count.clamp(1, crate::models::MAX_SLOTS)
}

pub fn tier_presets() -> Vec<TierPreset> {
    vec![
        TierPreset {
            id: "primary".into(),
            label: "主号模板".into(),
            description: "1–2 个主号：更长间隔，适合重要账号".into(),
            count: 2,
            min_delay_ms: 5000,
            max_delay_ms: 10000,
            use_tier_delays: true,
            slot_tiers: vec!["primary".into(), "primary".into()],
            aliases: vec!["主号 1".into(), "主号 2".into()],
        },
        TierPreset {
            id: "secondary".into(),
            label: "辅号模板".into(),
            description: "4–6 个辅号：中等间隔，日常运营".into(),
            count: 6,
            min_delay_ms: 2500,
            max_delay_ms: 6000,
            use_tier_delays: true,
            slot_tiers: vec![
                "secondary".into(),
                "secondary".into(),
                "secondary".into(),
                "secondary".into(),
                "secondary".into(),
                "secondary".into(),
            ],
            aliases: (1..=6).map(|i| format!("辅号 {i}")).collect(),
        },
        TierPreset {
            id: "mixed".into(),
            label: "主+辅混合".into(),
            description: "2 主号 + 6 辅号：主号更慢，辅号常规".into(),
            count: 8,
            min_delay_ms: 2500,
            max_delay_ms: 10000,
            use_tier_delays: true,
            slot_tiers: {
                let mut v = vec!["primary".into(), "primary".into()];
                v.extend(std::iter::repeat("secondary".into()).take(6));
                v
            },
            aliases: {
                let mut v = vec!["主号 1".into(), "主号 2".into()];
                v.extend((1..=6).map(|i| format!("辅号 {i}")));
                v
            },
        },
        TierPreset {
            id: "test".into(),
            label: "测试模板".into(),
            description: "验证多开是否可用，间隔较短".into(),
            count: 3,
            min_delay_ms: 1500,
            max_delay_ms: 3500,
            use_tier_delays: true,
            slot_tiers: vec!["test".into(), "test".into(), "test".into()],
            aliases: vec!["测试 1".into(), "测试 2".into(), "测试 3".into()],
        },
    ]
}
