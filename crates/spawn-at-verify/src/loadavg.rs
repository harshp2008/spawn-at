//! System load average (`/proc/loadavg`), recorded with each run so a slow batch can be told
//! apart from a busy machine.

use serde::{Deserialize, Serialize};

/// The three load averages (1, 5 and 15 minutes) at one instant.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LoadAvg {
    /// Load average over the last minute.
    pub one: f64,
    /// Load average over the last five minutes.
    pub five: f64,
    /// Load average over the last fifteen minutes.
    pub fifteen: f64,
}

/// Parses the text of `/proc/loadavg` (`"0.52 0.58 0.59 1/467 12345"`): the first three fields.
/// `None` when fewer than three fields parse as numbers.
pub fn parse_loadavg(text: &str) -> Option<LoadAvg> {
    let mut it = text.split_whitespace();
    Some(LoadAvg {
        one: it.next()?.parse().ok()?,
        five: it.next()?.parse().ok()?,
        fifteen: it.next()?.parse().ok()?,
    })
}

/// Reads `/proc/loadavg` now; `None` where the file does not exist or cannot be parsed.
pub fn read_loadavg() -> Option<LoadAvg> {
    parse_loadavg(&std::fs::read_to_string("/proc/loadavg").ok()?)
}

/// Number of CPUs the process may use (at least 1).
pub fn cpu_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

/// True when a 1-minute load average is above half the CPU count.
pub fn is_busy(one: f64, cpus: usize) -> bool {
    one > cpus as f64 / 2.0
}

/// The warning shown next to a result when the machine was busy at the start or the end of the
/// run (1-minute load above half of `cpus`), else `None`. Names the load values and the limit.
pub fn busy_warning(start: Option<LoadAvg>, end: Option<LoadAvg>, cpus: usize) -> Option<String> {
    let worst = [start, end]
        .into_iter()
        .flatten()
        .map(|l| l.one)
        .fold(f64::NEG_INFINITY, f64::max);
    if !worst.is_finite() || !is_busy(worst, cpus) {
        return None;
    }
    let show = |l: Option<LoadAvg>| l.map_or("?".to_string(), |l| format!("{:.2}", l.one));
    Some(format!(
        "machine busy: 1-min load {} at start, {} at end, above half of {} CPUs ({:.1}); timings may be inflated",
        show(start),
        show(end),
        cpus,
        cpus as f64 / 2.0
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real `/proc/loadavg` line parses; short or non-numeric text does not.
    #[test]
    fn test_parse_loadavg() {
        let l = parse_loadavg("0.52 0.58 0.59 1/467 12345\n").unwrap();
        assert_eq!((l.one, l.five, l.fifteen), (0.52, 0.58, 0.59));
        assert_eq!(parse_loadavg("0.52 0.58"), None);
        assert_eq!(parse_loadavg("a b c"), None);
        assert_eq!(parse_loadavg(""), None);
    }

    /// The reading of this machine exists (Linux) and is finite and non-negative.
    #[test]
    fn test_read_loadavg_on_this_machine() {
        let l = read_loadavg().expect("/proc/loadavg");
        assert!(l.one >= 0.0 && l.five >= 0.0 && l.fifteen >= 0.0);
        assert!(cpu_count() >= 1);
    }

    /// The limit is strictly above half the CPUs: 11.0 on 22 CPUs is not busy, 11.01 is.
    #[test]
    fn test_is_busy_threshold_is_half_the_cpus() {
        assert!(!is_busy(11.0, 22));
        assert!(is_busy(11.01, 22));
        assert!(!is_busy(0.4, 1));
        assert!(is_busy(0.6, 1));
    }

    /// The warning appears when either end is busy, names both values and the limit, and is
    /// absent when both are quiet or unknown.
    #[test]
    fn test_busy_warning() {
        let q = LoadAvg {
            one: 0.5,
            five: 0.5,
            fifteen: 0.5,
        };
        let b = LoadAvg {
            one: 12.5,
            five: 6.0,
            fifteen: 3.0,
        };
        assert_eq!(busy_warning(Some(q), Some(q), 22), None);
        assert_eq!(busy_warning(None, None, 22), None);
        let w = busy_warning(Some(q), Some(b), 22).unwrap();
        assert!(
            w.contains("0.50 at start") && w.contains("12.50 at end"),
            "{}",
            w
        );
        assert!(w.contains("half of 22 CPUs (11.0)"), "{}", w);
        assert!(busy_warning(Some(b), None, 22)
            .unwrap()
            .contains("? at end"));
    }
}
