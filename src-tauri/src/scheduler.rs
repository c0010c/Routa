//! Pure scheduling helpers. The 60 s loop itself lives in the runtime.

use crate::model::{Subscription, SubscriptionId};

/// Subscriptions whose update interval has elapsed (or that were never updated).
/// A `None` interval means automatic updates are off.
pub fn due_subscriptions(subs: &[Subscription], now: i64) -> Vec<SubscriptionId> {
    subs.iter()
        .filter(|s| match (s.update_interval_mins, s.last_updated) {
            (None, _) => false,
            (Some(_), None) => true,
            (Some(mins), Some(last)) => now - last >= i64::from(mins) * 60,
        })
        .map(|s| s.id.clone())
        .collect()
}

/// Whether a periodic delay test should run now.
pub fn delay_test_due(last_run: Option<i64>, interval_mins: Option<u32>, now: i64) -> bool {
    match (interval_mins, last_run) {
        (None, _) => false,
        (Some(_), None) => true,
        (Some(mins), Some(last)) => now - last >= i64::from(mins) * 60,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sub(interval: Option<u32>, last: Option<i64>) -> Subscription {
        Subscription {
            id: SubscriptionId::new(),
            name: "s".into(),
            url: "http://x/".into(),
            update_interval_mins: interval,
            fallback_exit: None,
            last_updated: last,
            last_result: None,
            nodes: vec![],
        }
    }

    #[test]
    fn due_respects_custom_interval() {
        let now = 1_000_000;
        let ok = sub(Some(30), Some(now - 29 * 60));
        let due = sub(Some(30), Some(now - 31 * 60));
        let off = sub(None, Some(now - 10_000_000));
        let never = sub(Some(30), None);
        let off_never = sub(None, None);
        let subs = [ok, due.clone(), off, never.clone(), off_never];
        assert_eq!(due_subscriptions(&subs, now), vec![due.id, never.id]);
    }

    #[test]
    fn due_at_exact_boundary() {
        let s = sub(Some(1), Some(100));
        assert_eq!(due_subscriptions(&[s.clone()], 160), vec![s.id]);
        assert!(due_subscriptions(&[sub(Some(1), Some(100))], 159).is_empty());
    }

    #[test]
    fn delay_test_due_cases() {
        let now = 10_000;
        assert!(!delay_test_due(Some(0), None, now));
        assert!(!delay_test_due(None, None, now));
        assert!(delay_test_due(None, Some(10), now));
        assert!(!delay_test_due(Some(now - 599), Some(10), now));
        assert!(delay_test_due(Some(now - 600), Some(10), now));
    }
}
