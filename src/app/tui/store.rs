//! Metric history stores used by the terminal UI.

use macmon::{CpuTierInfo, FanMetric, MemMetrics, Metrics, SocInfo};

use crate::config::RatioMode;

/// Samples kept for the history graphs, newest first: one per column.
const HISTORY_LEN: usize = 1024;

#[derive(Debug, Default, Clone)]
pub(super) struct RatioSeries {
  pub(super) items: Vec<u64>, // Recent percentages (0..=100), newest first.
  pub(super) ratio: f64,      // Latest ratio (0.0..=1.0).
}

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct FreqSample {
  freq_mhz: u64,
  scaled_ratio: f64,
  active_ratio: f64,
}

impl FreqSample {
  pub(super) fn new(freq_mhz: u32, scaled_ratio: f32, active_ratio: f32) -> Self {
    Self {
      freq_mhz: freq_mhz as u64,
      scaled_ratio: scaled_ratio as f64,
      active_ratio: active_ratio as f64,
    }
  }
}

impl RatioSeries {
  fn push(&mut self, ratio: f64) {
    // rounded like the percent in the box title, so the graph agrees with it
    self.items.insert(0, (ratio * 100.0).round() as u64);
    self.items.truncate(HISTORY_LEN);
    self.ratio = ratio;
  }
}

/// One frequency with parallel scaled and active ratio histories.
#[derive(Debug, Default, Clone)]
pub(super) struct FreqStore {
  pub(super) freq_mhz: u64,
  scaled: RatioSeries,
  active: RatioSeries,
}

impl FreqStore {
  pub(super) fn push(&mut self, sample: FreqSample) {
    self.freq_mhz = sample.freq_mhz;
    self.scaled.push(sample.scaled_ratio);
    self.active.push(sample.active_ratio);
  }

  pub(super) fn ratio(&self, mode: RatioMode) -> &RatioSeries {
    match mode {
      RatioMode::Scaled => &self.scaled,
      RatioMode::Active => &self.active,
    }
  }
}

/// History of one CPU cluster.
#[derive(Debug, Default)]
pub(super) struct ClusterStore {
  /// Tier label: `E` / `P` on M1–M4, `E` / `S` on M5, `P` / `S` on M5 Pro/Max, `E` / `P` / `S` on
  /// M6.
  pub(super) label: String,
  /// Core count, for the chip summary.
  pub(super) count: usize,
  pub(super) freq: FreqStore,
}

/// Histories of the CPU clusters, one per CPU tier of the library, lowest tier first.
#[derive(Debug, Default)]
pub(super) struct CpuClusters {
  pub(super) items: Vec<ClusterStore>,
}

impl CpuClusters {
  /// The CPU tiers of `soc` with their labels and core counts, without samples yet, so their boxes
  /// and the chip summary are complete before the first metrics sample.
  pub(super) fn from_soc(soc: &SocInfo) -> Self {
    let cluster = |tier: &CpuTierInfo| ClusterStore {
      label: tier.label.clone(),
      count: usize::from(tier.cores),
      freq: FreqStore::default(),
    };
    Self { items: soc.cpu_tiers.iter().map(cluster).collect() }
  }

  /// Adds the samples of the CPU tiers of `data`, which come in the same order as in `SocInfo`.
  pub(super) fn push(&mut self, data: &Metrics) {
    for (cluster, tier) in self.items.iter_mut().zip(&data.cpu_tiers) {
      cluster.freq.push(FreqSample::new(tier.freq_mhz, tier.scaled_ratio, tier.active_ratio));
    }
  }
}

/// Latest RAM / SWAP usage with the RAM usage history (bytes, newest first).
#[derive(Debug, Default)]
pub(super) struct MemoryStore {
  pub(super) items: Vec<u64>,
  pub(super) ram_usage: u64,
  pub(super) ram_total: u64,
  pub(super) swap_usage: u64,
  pub(super) swap_total: u64,
}

impl MemoryStore {
  pub(super) fn push(&mut self, value: MemMetrics) {
    self.items.insert(0, value.ram_usage);
    self.items.truncate(HISTORY_LEN);

    self.ram_usage = value.ram_usage;
    self.ram_total = value.ram_total;
    self.swap_usage = value.swap_usage;
    self.swap_total = value.swap_total;
  }
}

/// Measured CPU temperatures: milli-Celsius for graph bars, zero for a missing sample.
#[derive(Debug, Default)]
pub(super) struct TempStore {
  pub(super) items: Vec<u64>,
  current: Option<f32>,
}

impl TempStore {
  pub(super) fn last(&self) -> Option<f32> {
    self.current
  }

  pub(super) fn push(&mut self, value: Option<f32>) {
    self.current = value.filter(|v| *v > 0.0 && *v <= 150.0);
    self.items.insert(0, self.current.map_or(0, |v| ((v * 1000.0).round() as u64).max(1)));
    self.items.truncate(HISTORY_LEN);
  }
}

#[derive(Debug, Default)]
pub(super) struct FanStore {
  items: Vec<FanMetric>,
}

impl FanStore {
  pub(super) fn push(&mut self, value: Vec<FanMetric>) {
    self.items = value;
  }

  /// `1200 rpm`, `1200/1300 rpm` with several fans; empty without fans.
  pub(super) fn label(&self) -> String {
    if self.items.is_empty() {
      return String::new();
    }
    let rpms: Vec<String> = self.items.iter().map(|fan| fan.rpm.to_string()).collect();
    format!("{} rpm", rpms.join("/"))
  }
}

/// Share of `value` in `total`; 0 for a zero total (no sample yet).
pub(super) fn ratio(value: f64, total: f64) -> f64 {
  if total == 0.0 { 0.0 } else { value / total }
}

#[cfg(test)]
mod tests {
  use macmon::MemMetrics;

  use super::{FreqSample, FreqStore, HISTORY_LEN, MemoryStore, TempStore};
  use crate::config::RatioMode;

  #[test]
  fn temperature_preserves_fractional_readings_without_smoothing() {
    let mut store = TempStore::default();
    store.push(Some(40.125));
    store.push(Some(60.375));
    assert_eq!(store.last(), Some(60.375));
    assert_eq!(store.items, [60_375, 40_125]);
  }

  #[test]
  fn histories_are_capped() {
    let mut freq = FreqStore::default();
    let (mut mem, mut temp) = (MemoryStore::default(), TempStore::default());
    for i in 0..HISTORY_LEN + 10 {
      freq.push(FreqSample::new(1000, 0.25, 0.5));
      mem.push(MemMetrics { ram_total: 100, ram_usage: i as u64, swap_total: 0, swap_usage: 0 });
      temp.push(Some((i % 150 + 1) as f32));
    }

    // newest first
    let last = HISTORY_LEN as u64 + 9;
    assert_eq!((mem.items.len(), mem.items[0]), (HISTORY_LEN, last));
    for mode in [RatioMode::Scaled, RatioMode::Active] {
      assert_eq!(freq.ratio(mode).items.len(), HISTORY_LEN, "{mode:?}");
    }
    assert_eq!((temp.items.len(), temp.last()), (HISTORY_LEN, Some((last % 150 + 1) as f32)));
  }

  #[test]
  fn missing_and_invalid_temperatures_never_become_measurements() {
    let mut store = TempStore::default();
    for value in [None, Some(0.0), Some(-1.0), Some(151.0), Some(f32::NAN), Some(f32::INFINITY)] {
      store.push(value);
      assert_eq!(store.last(), None);
    }
    assert_eq!(store.items, [0; 6]);
    for _ in 0..HISTORY_LEN {
      store.push(None);
    }
    assert_eq!(store.items.len(), HISTORY_LEN);
    assert!(store.items.iter().all(|&v| v == 0));
  }

  #[test]
  fn missing_temperature_preserves_a_gap_and_clears_the_current_value() {
    let mut store = TempStore::default();
    store.push(Some(40.25));
    store.push(Some(60.5));
    store.push(None);
    assert_eq!(store.last(), None, "a missing reading must not become an estimate");
    assert_eq!(store.items.len(), 3, "missing readings still occupy a sample slot");
    store.push(Some(50.75));
    assert_eq!(store.last(), Some(50.75));
    assert_eq!(store.items, [50_750, 0, 60_500, 40_250]);
  }
}
