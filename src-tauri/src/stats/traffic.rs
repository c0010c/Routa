//! Turns periodic `/connections` snapshots into rates and cumulative totals.

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::core::api::ConnectionsSnapshot;
use crate::model::ListenerId;
use crate::store::{Bytes, TrafficTotals};

/// Inbound name prefix used by the tool's own subscription fetches.
const FETCH_PREFIX: &str = "__fetch__";

/// Bytes per second.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, specta::Type)]
pub struct Rate {
    pub up: u64,
    pub down: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, specta::Type)]
pub struct TrafficTick {
    pub total_rate: Rate,
    /// Every listener that had a delta in this tick.
    pub per_listener_rate: HashMap<ListenerId, Rate>,
    pub totals: TrafficTotals,
}

pub struct TrafficAggregator {
    last_conn: HashMap<String, (u64, u64)>,
    last_global: Option<(u64, u64)>,
    pub totals: TrafficTotals,
}

/// Delta between two cumulative readings; a decrease means the counter
/// restarted, so the whole new reading is the delta.
fn delta(prev: u64, now: u64) -> u64 {
    if now >= prev {
        now - prev
    } else {
        now
    }
}

fn rate(bytes: u64, secs: f64) -> u64 {
    if secs > 0.0 {
        (bytes as f64 / secs) as u64
    } else {
        0
    }
}

impl TrafficAggregator {
    pub fn new(totals: TrafficTotals) -> Self {
        Self {
            last_conn: HashMap::new(),
            last_global: None,
            totals,
        }
    }

    /// Folds one snapshot in. Per-connection deltas feed per-listener totals
    /// (a new connection counts its full bytes). Global totals use the
    /// `upload_total`/`download_total` delta; the very first snapshot after
    /// `new` only records the baseline and counts no global delta.
    pub fn ingest(
        &mut self,
        snap: &ConnectionsSnapshot,
        name_to_id: &HashMap<String, ListenerId>,
        elapsed: Duration,
    ) -> TrafficTick {
        let secs = elapsed.as_secs_f64();
        let mut per_listener: HashMap<ListenerId, (u64, u64)> = HashMap::new();
        let mut next_conn = HashMap::with_capacity(snap.connections.len());

        for c in &snap.connections {
            let (pu, pd) = self.last_conn.get(&c.id).copied().unwrap_or((0, 0));
            let (du, dd) = (delta(pu, c.upload), delta(pd, c.download));
            next_conn.insert(c.id.clone(), (c.upload, c.download));
            let name = &c.metadata.inbound_name;
            if name.starts_with(FETCH_PREFIX) {
                continue;
            }
            if let Some(id) = name_to_id.get(name) {
                if du > 0 || dd > 0 {
                    let e = per_listener.entry(id.clone()).or_default();
                    e.0 += du;
                    e.1 += dd;
                }
            }
        }
        self.last_conn = next_conn;

        let (gu, gd) = match self.last_global {
            Some((pu, pd)) => (delta(pu, snap.upload_total), delta(pd, snap.download_total)),
            None => (0, 0),
        };
        self.last_global = Some((snap.upload_total, snap.download_total));
        self.totals.total.up += gu;
        self.totals.total.down += gd;

        let mut per_listener_rate = HashMap::new();
        for (id, (u, d)) in per_listener {
            let t: &mut Bytes = self.totals.per_listener.entry(id.clone()).or_default();
            t.up += u;
            t.down += d;
            per_listener_rate.insert(
                id,
                Rate {
                    up: rate(u, secs),
                    down: rate(d, secs),
                },
            );
        }

        TrafficTick {
            total_rate: Rate {
                up: rate(gu, secs),
                down: rate(gd, secs),
            },
            per_listener_rate,
            totals: self.totals.clone(),
        }
    }

    /// Zeroes the cumulative totals but keeps the baselines, so the next
    /// tick does not re-count bytes already seen.
    pub fn reset(&mut self) {
        self.totals = TrafficTotals::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::api::{Conn, ConnMeta};

    fn lid(s: &str) -> ListenerId {
        ListenerId(s.into())
    }

    fn conn(id: &str, inbound: &str, up: u64, down: u64) -> Conn {
        Conn {
            id: id.into(),
            chains: vec![],
            upload: up,
            download: down,
            metadata: ConnMeta {
                inbound_name: inbound.into(),
                ..Default::default()
            },
        }
    }

    fn snap(up: u64, down: u64, conns: Vec<Conn>) -> ConnectionsSnapshot {
        ConnectionsSnapshot {
            upload_total: up,
            download_total: down,
            connections: conns,
        }
    }

    fn map() -> HashMap<String, ListenerId> {
        HashMap::from([("in-a".to_string(), lid("a"))])
    }

    const SEC: Duration = Duration::from_secs(1);

    #[test]
    fn per_listener_deltas_and_rates() {
        let mut agg = TrafficAggregator::new(TrafficTotals::default());
        agg.ingest(&snap(100, 200, vec![conn("1", "in-a", 100, 200)]), &map(), SEC);
        let tick = agg.ingest(
            &snap(2100, 8200, vec![conn("1", "in-a", 2100, 8200)]),
            &map(),
            Duration::from_secs(2),
        );
        assert_eq!(tick.per_listener_rate[&lid("a")], Rate { up: 1000, down: 4000 });
        assert_eq!(tick.total_rate, Rate { up: 1000, down: 4000 });
        assert_eq!(agg.totals.per_listener[&lid("a")], Bytes { up: 2100, down: 8200 });
        assert_eq!(agg.totals.total, Bytes { up: 2000, down: 8000 });
        assert_eq!(tick.totals, agg.totals);
    }

    #[test]
    fn new_connection_counts_full_bytes() {
        let mut agg = TrafficAggregator::new(TrafficTotals::default());
        agg.ingest(&snap(0, 0, vec![]), &map(), SEC);
        let tick = agg.ingest(&snap(50, 70, vec![conn("n", "in-a", 50, 70)]), &map(), SEC);
        assert_eq!(tick.per_listener_rate[&lid("a")], Rate { up: 50, down: 70 });
        assert_eq!(agg.totals.per_listener[&lid("a")], Bytes { up: 50, down: 70 });
    }

    #[test]
    fn closed_connection_dropped_and_reappearing_id_counts_full() {
        let mut agg = TrafficAggregator::new(TrafficTotals::default());
        agg.ingest(&snap(0, 0, vec![conn("1", "in-a", 10, 10)]), &map(), SEC);
        agg.ingest(&snap(0, 0, vec![]), &map(), SEC);
        assert!(agg.last_conn.is_empty());
        agg.ingest(&snap(0, 0, vec![conn("1", "in-a", 10, 10)]), &map(), SEC);
        assert_eq!(agg.totals.per_listener[&lid("a")], Bytes { up: 20, down: 20 });
    }

    #[test]
    fn decreasing_connection_counter_uses_new_value() {
        let mut agg = TrafficAggregator::new(TrafficTotals::default());
        agg.ingest(&snap(0, 0, vec![conn("1", "in-a", 1000, 1000)]), &map(), SEC);
        let tick = agg.ingest(&snap(0, 0, vec![conn("1", "in-a", 30, 40)]), &map(), SEC);
        assert_eq!(tick.per_listener_rate[&lid("a")], Rate { up: 30, down: 40 });
    }

    #[test]
    fn global_total_handles_core_restart() {
        let mut agg = TrafficAggregator::new(TrafficTotals::default());
        agg.ingest(&snap(10_000, 10_000, vec![]), &map(), SEC);
        let tick = agg.ingest(&snap(300, 400, vec![]), &map(), SEC);
        assert_eq!(tick.total_rate, Rate { up: 300, down: 400 });
        assert_eq!(agg.totals.total, Bytes { up: 300, down: 400 });
    }

    #[test]
    fn first_snapshot_sets_baseline() {
        let mut agg = TrafficAggregator::new(TrafficTotals {
            total: Bytes { up: 5, down: 6 },
            per_listener: HashMap::new(),
        });
        let tick = agg.ingest(&snap(9999, 8888, vec![]), &map(), SEC);
        assert_eq!(tick.total_rate, Rate::default());
        assert_eq!(agg.totals.total, Bytes { up: 5, down: 6 });
        let tick = agg.ingest(&snap(10_099, 8988, vec![]), &map(), SEC);
        assert_eq!(tick.total_rate, Rate { up: 100, down: 100 });
        assert_eq!(agg.totals.total, Bytes { up: 105, down: 106 });
    }

    #[test]
    fn fetch_inbound_ignored() {
        let mut agg = TrafficAggregator::new(TrafficTotals::default());
        let tick = agg.ingest(
            &snap(
                0,
                0,
                vec![
                    conn("1", "__fetch__sub1", 500, 500),
                    conn("2", "stranger", 7, 7),
                ],
            ),
            &HashMap::from([
                ("__fetch__sub1".to_string(), lid("x")),
                ("in-a".to_string(), lid("a")),
            ]),
            SEC,
        );
        assert!(tick.per_listener_rate.is_empty());
        assert!(agg.totals.per_listener.is_empty());
    }

    #[test]
    fn zero_elapsed_gives_zero_rate_but_counts_bytes() {
        let mut agg = TrafficAggregator::new(TrafficTotals::default());
        let tick = agg.ingest(
            &snap(0, 0, vec![conn("1", "in-a", 100, 100)]),
            &map(),
            Duration::ZERO,
        );
        assert_eq!(tick.per_listener_rate[&lid("a")], Rate::default());
        assert_eq!(agg.totals.per_listener[&lid("a")], Bytes { up: 100, down: 100 });
    }

    #[test]
    fn reset_zeroes_totals() {
        let mut agg = TrafficAggregator::new(TrafficTotals::default());
        agg.ingest(&snap(0, 0, vec![conn("1", "in-a", 100, 100)]), &map(), SEC);
        agg.reset();
        assert_eq!(agg.totals, TrafficTotals::default());
    }

    #[test]
    fn reset_keeps_baseline() {
        let mut agg = TrafficAggregator::new(TrafficTotals::default());
        agg.ingest(&snap(1000, 1000, vec![conn("1", "in-a", 100, 100)]), &map(), SEC);
        agg.reset();
        let tick = agg.ingest(
            &snap(1050, 1060, vec![conn("1", "in-a", 130, 140)]),
            &map(),
            SEC,
        );
        assert_eq!(agg.totals.per_listener[&lid("a")], Bytes { up: 30, down: 40 });
        assert_eq!(agg.totals.total, Bytes { up: 50, down: 60 });
        assert_eq!(tick.totals, agg.totals);
    }
}
