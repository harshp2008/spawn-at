//! Handler for `spawn-at-verify calibrate`: repeated measurement of one case's cloak duration.

use super::args::CalibrateArgs;
use super::check::{resolve_bin_path, resolve_verify_dir};
use crate::loader::{TestFilter, TestRegistry};
use crate::report::{build_session_report, write_report_artifacts, SessionReport};
use crate::runner::{extract_claimed_window_id, RunOptions, TestResult, TestRunner};
use crate::trace::TraceAnalyser;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};

/// One measured run: where its report lives (if any), the window the fixture named in stdout,
/// how many distinct windows the log slice mentions, and the measured cloak duration.
struct Sample {
    report: Option<PathBuf>,
    window_id: Option<u64>,
    slice_windows: usize,
    cloak_ms: Option<f64>,
    /// Final status and message of the run, shown when no cloak duration was measured.
    status: String,
    /// Scoped events as `TAG@+ms` relative to the first scoped event, for diagnosing outliers.
    events: String,
}

/// Builds a [`Sample`] from a finished result.
fn sample_from(result: &TestResult, report: Option<PathBuf>) -> Sample {
    let slice_windows = result
        .log_slice
        .as_deref()
        .map(|s| {
            TraceAnalyser::new()
                .parse_events(s)
                .iter()
                .filter_map(|e| e.window_id)
                .collect::<BTreeSet<u64>>()
                .len()
        })
        .unwrap_or(0);
    Sample {
        report,
        window_id: extract_claimed_window_id(&result.stdout, &result.stderr),
        slice_windows,
        cloak_ms: result.cloak_duration_ms(),
        events: result
            .trace_report()
            .map(|r| {
                let t0 = r.events.first().map(|e| e.elapsed_ms).unwrap_or(0.0);
                r.events
                    .iter()
                    .map(|e| format!("{}@+{:.1}", e.tag, e.elapsed_ms - t0))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default(),
        status: format!(
            "{:?}: {}",
            result.final_status,
            result.message.as_deref().unwrap_or("")
        ),
    }
}

/// Prints one sample per line, then min / median / max of the cloak durations.
fn print_samples(title: &str, samples: &[Sample]) {
    println!("{}", title);
    for (i, s) in samples.iter().enumerate() {
        let cloak = s
            .cloak_ms
            .map(|c| format!("{:7.2} ms", c))
            .unwrap_or_else(|| "    none   ".into());
        let win = s
            .window_id
            .map(|w| w.to_string())
            .unwrap_or_else(|| "none".into());
        let rep = s
            .report
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(first run of the repeat session report)".into());
        println!(
            "  run {:>2}: cloak_duration_ms={}  window_id={}  windows_in_slice={}  report={}",
            i + 1,
            cloak,
            win,
            s.slice_windows,
            rep
        );
        if s.cloak_ms.is_none() {
            println!("          status: {}", s.status);
        }
        if s.cloak_ms.is_none_or(|c| c >= 50.0) {
            println!("          scoped events: {}", s.events);
        }
    }
    let mut vals: Vec<f64> = samples.iter().filter_map(|s| s.cloak_ms).collect();
    vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if vals.is_empty() {
        println!("  cloak_duration_ms: no measurements");
    } else {
        let n = vals.len();
        let median = if n % 2 == 1 {
            vals[n / 2]
        } else {
            (vals[n / 2 - 1] + vals[n / 2]) / 2.0
        };
        println!(
            "  cloak_duration_ms over {} runs: min={:.2} median={:.2} max={:.2}",
            n,
            vals[0],
            median,
            vals[n - 1]
        );
    }
    println!();
}

/// Runs `spawn-at-verify run --glob <case>` as a fresh process and reads the first result
/// back from the report directory it announces on stdout.
fn run_separate_process(args: &CalibrateArgs, bin: &Path, verify_dir: &Path) -> Sample {
    let exe = std::env::current_exe().expect("current_exe");
    let mut cmd = Command::new(exe);
    cmd.arg("run")
        .args(["--glob", &args.case])
        .arg("--bin")
        .arg(bin)
        .arg("--dir")
        .arg(verify_dir);
    if args.allow_dirty {
        cmd.arg("--allow-dirty");
    }
    let out = match cmd.output() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("Failed to launch run subprocess: {}", e);
            return Sample {
                report: None,
                window_id: None,
                slice_windows: 0,
                cloak_ms: None,
                status: "could not launch run subprocess".into(),
                events: String::new(),
            };
        }
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    let dir = stdout
        .lines()
        .find_map(|l| l.strip_prefix("Session artifacts written to: "))
        .map(PathBuf::from);
    let report_path = dir.map(|d| d.join("report.json"));
    let parsed = report_path
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str::<SessionReport>(&t).ok())
        .and_then(|r| r.results.into_iter().next());
    match parsed {
        Some(res) => sample_from(&res, report_path),
        None => {
            eprintln!("No report found for a run; stdout was:\n{}", stdout);
            Sample {
                report: report_path,
                window_id: None,
                slice_windows: 0,
                cloak_ms: None,
                status: "no report.json found".into(),
                events: String::new(),
            }
        }
    }
}

/// Measures the cloak duration of one case with N separate processes and with N `--repeat`
/// runs in one process, printing min/median/max and every report.json path.
pub fn handle_calibrate(args: CalibrateArgs) {
    let verify_dir = resolve_verify_dir(args.dir.clone()).unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        exit(1)
    });
    let bin = resolve_bin_path(args.bin.clone()).unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        exit(1)
    });
    let runs = args.runs.clamp(1, 100);

    println!("case: {}   runs per mode: {}\n", args.case, runs);

    let separate: Vec<Sample> = (0..runs)
        .map(|_| run_separate_process(&args, &bin, &verify_dir))
        .collect();
    print_samples(&format!("N separate processes (N={}):", runs), &separate);
    let mut paths: Vec<_> = separate.iter().filter_map(|s| s.report.clone()).collect();
    paths.sort();
    paths.dedup();
    if paths.len() < separate.len() {
        println!(
            "  note: {} runs shared a report directory (second-granularity names)\n",
            separate.len() - paths.len()
        );
    }

    let registry = TestRegistry::load_from_dir(&verify_dir).unwrap_or_else(|e| {
        eprintln!("Error loading test registry: {}", e);
        exit(1)
    });
    let runner = TestRunner::new(
        registry,
        RunOptions {
            bin_path: bin.clone(),
            allow_dirty: args.allow_dirty,
            repeat: runs,
            filter: TestFilter {
                id_glob: Some(args.case.clone()),
                ..Default::default()
            },
            ..Default::default()
        },
    );
    match runner.run_detailed() {
        Ok((preflight, env_info, results, all_runs)) => {
            let report = build_session_report(&bin, &preflight, &env_info, &results, false);
            let path = write_report_artifacts(Path::new("verify-results"), &report)
                .ok()
                .map(|d| d.join("report.json"));
            let samples: Vec<Sample> = all_runs
                .first()
                .map(|v| v.iter().map(|r| sample_from(r, path.clone())).collect())
                .unwrap_or_default();
            print_samples(&format!("--repeat {} in one process:", runs), &samples);
        }
        Err(e) => {
            eprintln!("Runner execution failed: {}", e);
            exit(1);
        }
    }
}
