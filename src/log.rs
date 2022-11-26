use std::fmt::Debug;
use std::time::{Duration, Instant};

use hdrhistogram::Histogram;
use serde::{Deserialize, Serialize};
use tracing::trace;

use crate::snapshots::CaptureStatus;

#[derive(Debug, Serialize, Deserialize)]
pub enum Operation {
    Snapshot {
        repo: String,
        op: Option<CaptureStatus>,
        error: Option<String>,
        latency: f32,
    },
    CollectStats {
        per_dir_stats: Histo,
        loop_stats: Histo,
    },
}

impl Operation {
    pub fn should_log(&self) -> bool {
        match self {
            Operation::Snapshot {
                repo: _,
                op,
                error,
                latency: _,
            } => op.is_some() || error.is_some(),
            Operation::CollectStats { .. } => {
                true // logic punted to StatCollector
            }
        }
    }

    pub fn log_str(&mut self) -> String {
        // This unwrap seems safe, afaict. We're not cramming any user supplied strings in here.
        serde_json::to_string(self).expect("Couldn't serialize to JSON")
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Stats {
    dir_stats: Histo,
    loop_stats: Histo,
}

/// A serializable form of a hdrhistogram, mainly just for logging out
/// in a way we want to read it
#[derive(Debug, Serialize, Deserialize)]
pub struct Histo {
    mean: f64,
    count: u64,
    min: u64,
    max: u64,
    percentiles: Vec<Percentile>,
}

/// For serializing to JSON
///
/// Choice of tiny names because this one shows up a lot, one
/// for each percentile bucket. It shows a lot more data
/// points at the upper percentiles, so we need to capture
/// both percentile and associated millisecond value.
#[derive(Debug, Serialize, Deserialize)]
pub struct Percentile {
    pct: f64,
    val: u64,
}

impl Histo {
    pub fn from_histogram(hist: &Histogram<u64>) -> Histo {
        Self {
            mean: hist.mean(),
            count: hist.len(),
            min: hist.min(),
            max: hist.max(),
            percentiles: hist
                .iter_quantiles(2)
                .map(|q| Percentile {
                    pct: q.percentile(),
                    val: q.value_iterated_to(),
                })
                .collect(),
        }
    }
}

#[derive(Debug)]
pub struct StatCollector {
    start: Instant,
    per_dir_stats: Histogram<u64>,
    loop_stats: Histogram<u64>,
}

/// 5 minutes in milliseconds
const MAX_LATENCY_IMAGINABLE: u64 = 5 * 60 * 1000;

/// How many seconds between logging stats?
const STAT_LOG_INTERVAL: f32 = 600.0;

impl StatCollector {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            per_dir_stats: Histogram::<u64>::new_with_max(MAX_LATENCY_IMAGINABLE, 3).unwrap(),
            loop_stats: Histogram::<u64>::new_with_max(MAX_LATENCY_IMAGINABLE, 3).unwrap(),
        }
    }

    pub fn to_op(&self) -> Operation {
        Operation::CollectStats {
            per_dir_stats: Histo::from_histogram(&self.per_dir_stats),
            loop_stats: Histo::from_histogram(&self.loop_stats),
        }
    }

    pub fn should_log(&self) -> bool {
        let elapsed = (Instant::now() - self.start).as_secs_f32();
        trace!(
            elapsed = elapsed,
            target = STAT_LOG_INTERVAL,
            "Should we log metrics?"
        );
        elapsed > STAT_LOG_INTERVAL
    }

    pub fn log_str(&mut self) -> String {
        let mut op = self.to_op();
        let ret = op.log_str();
        self.reset();
        ret
    }

    fn reset(&mut self) {
        self.start = Instant::now();
        self.per_dir_stats.clear();
        self.loop_stats.clear();
    }

    /// Record the time it takes to process a single directory. Mainly interested to see if
    /// there's any outliers, the histogram should be interesting.
    pub fn record_dir(&mut self, latency: Duration) {
        let value = latency.as_millis().try_into().unwrap();
        self.per_dir_stats.saturating_record(value);
    }

    /// Record the time it takes to go through all directories. I expect mean will be the
    /// most interesting datum. Mainly for projecting CPU usage.
    pub fn record_loop(&mut self, latency: Duration) {
        let value = latency.as_millis().try_into().unwrap();
        self.loop_stats.saturating_record(value);
    }
}

impl Default for StatCollector {
    fn default() -> Self {
        Self::new()
    }
}
