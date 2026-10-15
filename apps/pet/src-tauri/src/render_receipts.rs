use code_musume_contracts::RenderReceipt;
use std::{collections::VecDeque, time::Instant};

#[derive(Default)]
pub struct RenderReceipts {
    targets: VecDeque<(u64, Instant)>,
    pub latest: Option<RenderReceipt>,
}
impl RenderReceipts {
    pub fn published(&mut self, revision: u64, received: Instant) {
        if self.targets.len() == 256 {
            self.targets.pop_front();
        }
        self.targets.push_back((revision, received));
    }
    pub fn acknowledge(
        &mut self,
        instance: &str,
        supplied: &str,
        revision: u64,
        now: Instant,
    ) -> bool {
        if instance != supplied || self.latest.as_ref().is_some_and(|r| revision <= r.revision) {
            return false;
        }
        let Some((_, received)) = self.targets.iter().find(|(r, _)| *r == revision) else {
            return false;
        };
        self.latest = Some(RenderReceipt {
            instance_id: instance.into(),
            revision,
            latency_ms: now.saturating_duration_since(*received).as_secs_f64() * 1000.0,
        });
        self.targets.retain(|(r, _)| *r > revision);
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn matches_instance_and_revision_and_rejects_late_receipts() {
        let start = Instant::now();
        let mut r = RenderReceipts::default();
        r.published(1, start);
        r.published(2, start);
        assert!(!r.acknowledge("app", "old", 2, start));
        assert!(!r.acknowledge("app", "app", 99, start));
        assert!(r.acknowledge("app", "app", 2, start + Duration::from_millis(40)));
        assert_eq!(r.latest.as_ref().unwrap().latency_ms, 40.0);
        assert!(!r.acknowledge("app", "app", 1, start));
        assert!(!r.acknowledge("app", "app", 2, start));
    }
}
