//! `burn-cpu`: keeps N threads of this process spinning for S seconds, to put the machine under
//! a known CPU load while a live measurement runs.
//!
//! The load is threads inside one process, not child processes: when the process exits (the
//! deadline, or a kill) the load ends with it and nothing is left behind.

use std::hint::black_box;
use std::thread;
use std::time::{Duration, Instant};

/// Largest `--threads` accepted; a typo such as 2200 is an error, not a fork bomb.
pub const MAX_THREADS: usize = 1024;
/// Largest `--seconds` accepted (one day).
pub const MAX_SECONDS: f64 = 86_400.0;

/// Usage text printed with every argument error.
pub const USAGE: &str = "Usage: cargo xtask burn-cpu --threads N --seconds S\n  \
     N: 1..=1024 spinning threads\n  S: seconds to run, greater than 0 and at most 86400 (fractions allowed)";

/// Parsed `burn-cpu` arguments.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BurnArgs {
    /// Number of spinning threads.
    pub threads: usize,
    /// How long they spin.
    pub seconds: f64,
}

/// Parses the arguments after `burn-cpu`: `--threads N` and `--seconds S`, in either order, each
/// exactly once. Rejects unknown flags, a missing or repeated flag, a missing value, a
/// non-number, `N` outside 1..=1024 and `S` that is not finite, not above 0, or above 86400.
pub fn parse_args(args: &[String]) -> Result<BurnArgs, String> {
    let mut threads: Option<usize> = None;
    let mut seconds: Option<f64> = None;
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--threads" => {
                let value = it.next().ok_or("--threads needs a value")?;
                if threads.is_some() {
                    return Err("--threads given twice".to_string());
                }
                let n: usize = value
                    .parse()
                    .map_err(|_| format!("--threads: '{}' is not a whole number", value))?;
                if n == 0 || n > MAX_THREADS {
                    return Err(format!("--threads: {} is outside 1..={}", n, MAX_THREADS));
                }
                threads = Some(n);
            }
            "--seconds" => {
                let value = it.next().ok_or("--seconds needs a value")?;
                if seconds.is_some() {
                    return Err("--seconds given twice".to_string());
                }
                let s: f64 = value
                    .parse()
                    .map_err(|_| format!("--seconds: '{}' is not a number", value))?;
                if !s.is_finite() || s <= 0.0 || s > MAX_SECONDS {
                    return Err(format!(
                        "--seconds: {} must be above 0 and at most {}",
                        value, MAX_SECONDS
                    ));
                }
                seconds = Some(s);
            }
            other => return Err(format!("unknown argument '{}'", other)),
        }
    }
    Ok(BurnArgs {
        threads: threads.ok_or("--threads is required")?,
        seconds: seconds.ok_or("--seconds is required")?,
    })
}

/// Spins `threads` threads until `duration` has elapsed and joins them all. Returns each thread's
/// loop-iteration count (all above zero for any duration long enough to start a thread), so the
/// caller can see that every thread really ran.
pub fn burn(threads: usize, duration: Duration) -> Vec<u64> {
    let deadline = Instant::now() + duration;
    let handles: Vec<_> = (0..threads)
        .map(|_| {
            thread::spawn(move || {
                let mut iterations = 0u64;
                let mut acc = 1u64;
                // The clock is read once per 4096 multiplications so the thread spends its time
                // on arithmetic, not on the clock; `black_box` stops the loop being optimised out.
                while Instant::now() < deadline {
                    for _ in 0..4096 {
                        acc = black_box(acc.wrapping_mul(6364136223846793005).wrapping_add(1));
                    }
                    iterations += 1;
                }
                black_box(acc);
                iterations
            })
        })
        .collect();
    handles
        .into_iter()
        .map(|h| h.join().expect("burn thread panicked"))
        .collect()
}

/// Entry point: parses `args`, burns, prints one line when done. Exits 1 with the usage text on
/// an argument error.
pub fn run_burn_cpu(args: &[String]) {
    let parsed = match parse_args(args) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("burn-cpu: {}\n{}", e, USAGE);
            std::process::exit(1);
        }
    };
    eprintln!(
        "burn-cpu: {} threads for {} s",
        parsed.threads, parsed.seconds
    );
    let started = Instant::now();
    let counts = burn(parsed.threads, Duration::from_secs_f64(parsed.seconds));
    eprintln!(
        "burn-cpu: done after {:.2} s, {} threads joined",
        started.elapsed().as_secs_f64(),
        counts.len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    /// Both flags in either order parse, and a fractional second count is kept exactly.
    #[test]
    fn test_parse_args_accepts_both_orders_and_fractions() {
        let want = BurnArgs {
            threads: 22,
            seconds: 90.0,
        };
        assert_eq!(
            parse_args(&a(&["--threads", "22", "--seconds", "90"])),
            Ok(want)
        );
        assert_eq!(
            parse_args(&a(&["--seconds", "90", "--threads", "22"])),
            Ok(want)
        );
        assert_eq!(
            parse_args(&a(&["--threads", "1", "--seconds", "0.25"])),
            Ok(BurnArgs {
                threads: 1,
                seconds: 0.25
            })
        );
    }

    /// A missing flag is named in the error, so `--threads 4` alone says `--seconds` is missing.
    #[test]
    fn test_parse_args_requires_both_flags() {
        assert_eq!(
            parse_args(&a(&["--threads", "4"])),
            Err("--seconds is required".to_string())
        );
        assert_eq!(
            parse_args(&a(&["--seconds", "4"])),
            Err("--threads is required".to_string())
        );
        assert_eq!(
            parse_args(&a(&[])),
            Err("--threads is required".to_string())
        );
    }

    /// A flag with nothing after it, a repeated flag and an unknown flag are all errors.
    #[test]
    fn test_parse_args_rejects_missing_values_repeats_and_unknown_flags() {
        assert_eq!(
            parse_args(&a(&["--threads"])),
            Err("--threads needs a value".to_string())
        );
        assert_eq!(
            parse_args(&a(&["--seconds"])),
            Err("--seconds needs a value".to_string())
        );
        assert_eq!(
            parse_args(&a(&["--threads", "2", "--threads", "3", "--seconds", "1"])),
            Err("--threads given twice".to_string())
        );
        assert_eq!(
            parse_args(&a(&["--threads", "2", "--seconds", "1", "--seconds", "2"])),
            Err("--seconds given twice".to_string())
        );
        assert_eq!(
            parse_args(&a(&["--threads", "2", "--seconds", "1", "--nice"])),
            Err("unknown argument '--nice'".to_string())
        );
    }

    /// Thread counts of 0, 1025, a negative number, a fraction and text are rejected; 1 and 1024
    /// (the limits) are accepted.
    #[test]
    fn test_parse_args_thread_bounds() {
        for bad in ["0", "1025", "-3", "2.5", "many", ""] {
            assert!(
                parse_args(&a(&["--threads", bad, "--seconds", "1"])).is_err(),
                "--threads {:?} should be rejected",
                bad
            );
        }
        for good in ["1", "1024"] {
            assert!(parse_args(&a(&["--threads", good, "--seconds", "1"])).is_ok());
        }
    }

    /// Seconds of 0, negative, NaN, infinity, above one day and text are rejected; 86400 is the
    /// largest accepted value.
    #[test]
    fn test_parse_args_second_bounds() {
        for bad in ["0", "-1", "NaN", "inf", "86400.5", "soon", ""] {
            assert!(
                parse_args(&a(&["--threads", "1", "--seconds", bad])).is_err(),
                "--seconds {:?} should be rejected",
                bad
            );
        }
        assert!(parse_args(&a(&["--threads", "1", "--seconds", "86400"])).is_ok());
    }

    /// 3 threads for 0.2 s: all 3 are joined, each ran its loop at least once, and the call did
    /// not return before the deadline.
    #[test]
    fn test_burn_joins_every_thread_and_waits_for_the_deadline() {
        let started = Instant::now();
        let counts = burn(3, Duration::from_millis(200));
        assert!(started.elapsed() >= Duration::from_millis(200));
        assert_eq!(counts.len(), 3);
        assert!(counts.iter().all(|&c| c > 0), "{:?}", counts);
    }
}
