use std::{collections::VecDeque, time::Duration};

use serde::Serialize;

use crate::model::Snapshot;

/// How far back the history reaches, at the interval it was created with.
pub const HISTORY_SPAN: Duration = Duration::from_secs(600);

/// One sample of the values shown as graphs.
#[derive(Debug, Clone, Serialize)]
pub struct HistoryPoint {
    pub timestamp: u64,
    pub cpu: f32,
    pub mem: f32,
    /// Bytes per second, summed over all interfaces except loopback.
    pub rx_rate: u64,
    pub tx_rate: u64,
}

impl HistoryPoint {
    pub fn from_snapshot(s: &Snapshot) -> Self {
        let nets = s.nets.iter().filter(|n| !is_loopback(&n.name));
        let (rx_rate, tx_rate) = nets.fold((0u64, 0u64), |(rx, tx), n| {
            (rx.saturating_add(n.rx_rate), tx.saturating_add(n.tx_rate))
        });
        Self { timestamp: s.timestamp, cpu: s.cpu.total, mem: s.mem.percent, rx_rate, tx_rate }
    }
}

/// Ring buffer of the most recent samples, oldest first.
pub struct History {
    points: VecDeque<HistoryPoint>,
    cap: usize,
}

impl History {
    /// Sized to cover [`HISTORY_SPAN`] at the given refresh interval.
    pub fn new(interval: Duration) -> Self {
        let cap = (HISTORY_SPAN.as_secs_f64() / interval.as_secs_f64().max(0.001)).ceil() as usize;
        Self::with_capacity(cap)
    }

    pub fn with_capacity(cap: usize) -> Self {
        let cap = cap.max(1);
        Self { points: VecDeque::with_capacity(cap), cap }
    }

    pub fn push(&mut self, snapshot: &Snapshot) {
        if self.points.len() == self.cap {
            self.points.pop_front();
        }
        self.points.push_back(HistoryPoint::from_snapshot(snapshot));
    }

    pub fn points(&self) -> impl ExactSizeIterator<Item = &HistoryPoint> + DoubleEndedIterator {
        self.points.iter()
    }

    pub fn capacity(&self) -> usize {
        self.cap
    }

    pub fn to_vec(&self) -> Vec<HistoryPoint> {
        self.points.iter().cloned().collect()
    }
}

/// `lo` on Linux, `Loopback Pseudo-Interface 1` on Windows.
fn is_loopback(name: &str) -> bool {
    name == "lo" || name.to_ascii_lowercase().contains("loopback")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::NetInfo;

    fn snapshot(timestamp: u64) -> Snapshot {
        Snapshot { timestamp, ..Default::default() }
    }

    fn net(name: &str, rx_rate: u64, tx_rate: u64) -> NetInfo {
        NetInfo { name: name.into(), rx_rate, tx_rate, rx_total: 0, tx_total: 0 }
    }

    #[test]
    fn drops_oldest_when_full() {
        let mut h = History::with_capacity(3);
        for t in 0..5 {
            h.push(&snapshot(t));
        }
        let ts: Vec<u64> = h.points().map(|p| p.timestamp).collect();
        assert_eq!(ts, [2, 3, 4]);
    }

    #[test]
    fn capacity_covers_span() {
        assert_eq!(History::new(Duration::from_secs(2)).cap, 300);
        assert_eq!(History::new(Duration::from_millis(500)).cap, 1200);
    }

    #[test]
    fn net_rate_skips_loopback() {
        let mut s = snapshot(0);
        s.nets = vec![
            net("eth0", 100, 10),
            net("lo", 1000, 1000),
            net("Loopback Pseudo-Interface 1", 1000, 1000),
            net("wlan0", 50, 5),
        ];
        let p = HistoryPoint::from_snapshot(&s);
        assert_eq!((p.rx_rate, p.tx_rate), (150, 15));
    }
}
