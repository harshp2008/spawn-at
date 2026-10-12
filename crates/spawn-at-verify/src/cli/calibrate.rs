//! Handler for `spawn-at-verify calibrate`: repeated measurement of one case's cloak duration.
//!
//! Besides the cloak duration it prints, for every run, the phase gaps inside the placement
//! trace and the machine state just before the run, and it saves the raw log slice of any run
//! that was not measured or took 50 ms or more, so outliers can be studied afterwards.

use super::args::{CalibrateArgs, CalibrateMode};
use super::check::{resolve_bin_path, resolve_verify_dir};
use crate::loader::{TestFilter, TestRegistry};
use crate::report::{build_session_report_with_runs, write_report_artifacts, SessionReport};
use crate::runner::{extract_claimed_window_id, RunOptions, TestResult, TestRunner};
use crate::trace::phases::{phase_gaps, PhaseGaps};
use crate::trace::TraceAnalyser;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};

/// Cloak durations at or above this many milliseconds (or missing) get their events printed and
/// their raw slice saved.
const OUTLIER_MS: f64 = 50.0;

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
    /// Gaps between placement landmarks of the scoped window.
    gaps: PhaseGaps,
    /// Machine state read just before the run started.
    system: String,
    /// Where the raw log slice was saved, for outliers.
    slice_dump: Option<PathBuf>,
}

/// Machine state: load, CPU frequency, power source and profile, and the desktop's windows.
///
/// Read-only diagnostics so batch-to-batch differences can be compared with machine state. Window
/// titles are deliberately not recorded, only the count and the focused window's class.
fn system_state(bin: &Path) -> String {
    let load1 = std::fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|s| s.split_whitespace().next().map(String::from))
        .unwrap_or_else(|| "?".into());
    let mhz: Vec<f64> = std::fs::read_to_string("/proc/cpuinfo")
        .unwrap_or_default()
        .lines()
        .filter(|l| l.starts_with("cpu MHz"))
        .filter_map(|l| l.split(':').nth(1)?.trim().parse().ok())
        .collect();
    let mhz = if mhz.is_empty() {
        "?".to_string()
    } else {
        format!("{:.0}", mhz.iter().sum::<f64>() / mhz.len() as f64)
    };
    let ac = ["ADP1", "AC", "ACAD"]
        .iter()
        .find_map(|n| std::fs::read_to_string(format!("/sys/class/power_supply/{}/online", n)).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "?".into());
    let profile = Command::new("powerprofilesctl")
        .arg("get")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "?".into());
    let (windows, focused) = Command::new(bin)
        .args(["query", "windows", "--json"])
        .output()
        .ok()
        .and_then(|o| serde_json::from_slice::<serde_json::Value>(&o.stdout).ok())
        .and_then(|v| {
            let arr = v.as_array()?;
            let focused = arr
                .iter()
                .find(|w| w.get("focused").and_then(|f| f.as_bool()) == Some(true))
                .and_then(|w| w.get("class").and_then(|c| c.as_str()).map(String::from))
                .unwrap_or_else(|| "none".into());
            Some((arr.len().to_string(), focused))
        })
        .unwrap_or_else(|| ("?".into(), "?".into()));
    format!(
        "load1={} cpu_mhz={} ac={} profile={} windows={} focused={}",
        load1, mhz, ac, profile, windows, focused
    )
}

/// Saves `slice` under `verify-results/calibrate-slices/` and returns the path.
fn dump_slice(tag: &str, slice: &str) -> Option<PathBuf> {
    let dir = Path::new("verify-results").join("calibrate-slices");
    std::fs::create_dir_all(&dir).ok()?;
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    let path = dir.join(format!("{}-{}.log", secs, tag));
    std::fs::write(&path, slice).ok()?;
    Some(path)
}

/// Builds a [`Sample`] from a finished result. `tag` names the run for saved slices.
fn sample_from(result: &TestResult, report: Option<PathBuf>, system: String, tag: &str) -> Sample {
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
    let scoped = result.trace_report();
    let gaps = scoped
        .as_ref()
        .map(|r| phase_gaps(&r.events))
        .unwrap_or_default();
    let cloak_ms = result.cloak_duration_ms();
    let flagged = cloak_ms.is_none_or(|c| c >= OUTLIER_MS);
    Sample {
        report,
        window_id: extract_claimed_window_id(&result.stdout, &result.stderr),
        slice_windows,
        cloak_ms,
        events: scoped
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
        gaps,
        system,
        slice_dump: if flagged {
            result.log_slice.as_deref().and_then(|s| dump_slice(tag, s))
        } else {
            None
        },
    }
}

/// Formats an optional millisecond value.
fn ms(v: Option<f64>) -> String {
    v.map(|v| format!("{:.1}", v)).unwrap_or_else(|| "-".into())
}

/// Prints `label` min / median / max of `vals` on one line.
fn print_stats(label: &str, mut vals: Vec<f64>) {
    vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if vals.is_empty() {
        println!("  {}: no measurements", label);
        return;
    }
    let n = vals.len();
    let median = if n % 2 == 1 {
        vals[n / 2]
    } else {
        (vals[n / 2 - 1] + vals[n / 2]) / 2.0
    };
    println!(
        "  {} over {} runs: min={:.2} median={:.2} max={:.2}",
        label,
        n,
        vals[0],
        median,
        vals[n - 1]
    );
}

/// Prints one sample per line (cloak, phase gaps, machine state), then min / median / max.
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
        println!(
            "          gaps_ms: map->cloak={} cloak->commit={} commit->position={} 4c->uncloak={}   state: {}",
            ms(s.gaps.map_to_cloak),
            ms(s.gaps.cloak_to_commit),
            ms(s.gaps.commit_to_position),
            ms(s.gaps.uncloak_4c_to_uncloak),
            s.system
        );
        if s.cloak_ms.is_none() {
            println!("          status: {}", s.status);
        }
        if s.cloak_ms.is_none_or(|c| c >= OUTLIER_MS) {
            println!("          scoped events: {}", s.events);
            if let Some(p) = &s.slice_dump {
                println!("          raw slice saved: {}", p.display());
            }
        }
    }
    print_stats(
        "cloak_duration_ms",
        samples.iter().filter_map(|s| s.cloak_ms).collect(),
    );
    print_stats(
        "commit->position gap ms",
        samples
            .iter()
            .filter_map(|s| s.gaps.commit_to_position)
            .collect(),
    );
    print_stats(
        "4c->uncloak gap ms",
        samples
            .iter()
            .filter_map(|s| s.gaps.uncloak_4c_to_uncloak)
            .collect(),
    );
    println!();
}

/// Runs `spawn-at-verify run --glob <case>` as a fresh process and reads the first result
/// back from the report directory it announces on stdout.
fn run_separate_process(
    args: &CalibrateArgs,
    bin: &Path,
    verify_dir: &Path,
    index: usize,
) -> Sample {
    let system = system_state(bin);
    let empty = |report: Option<PathBuf>, status: &str, system: String| Sample {
        report,
        window_id: None,
        slice_windows: 0,
        cloak_ms: None,
        status: status.into(),
        events: String::new(),
        gaps: PhaseGaps::default(),
        system,
        slice_dump: None,
    };
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
            return empty(None, "could not launch run subprocess", system);
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
        Some(res) => sample_from(
            &res,
            report_path,
            system,
            &format!("separate-run{:02}", index),
        ),
        None => {
            eprintln!("No report found for a run; stdout was:\n{}", stdout);
            empty(report_path, "no report.json found", system)
        }
    }
}

/// Measures the cloak duration of one case with N separate processes and/or N `--repeat` runs in
/// one process (see `--mode`), printing per-run values, phase gaps, machine state and
/// min/median/max, plus every report.json path.
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

    println!(
        "case: {}   runs per mode: {}   mode: {:?}\n",
        args.case, runs, args.mode
    );

    if matches!(args.mode, CalibrateMode::Both | CalibrateMode::Separate) {
        let separate: Vec<Sample> = (0..runs)
            .map(|i| run_separate_process(&args, &bin, &verify_dir, i + 1))
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
    }

    if matches!(args.mode, CalibrateMode::Both | CalibrateMode::Repeat) {
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
        // One machine-state snapshot for the whole in-process batch.
        let system = system_state(&bin);
        let load_start = crate::loadavg::read_loadavg();
        match runner.run_detailed() {
            Ok((preflight, env_info, results, all_runs)) => {
                let load_end = crate::loadavg::read_loadavg();
                let mut report = build_session_report_with_runs(
                    &bin, &preflight, &env_info, &results, &all_runs, false,
                );
                report.environment.set_load(load_start, load_end);
                let path = write_report_artifacts(Path::new("verify-results"), &report)
                    .ok()
                    .map(|d| d.join("report.json"));
                let samples: Vec<Sample> = all_runs
                    .first()
                    .map(|v| {
                        v.iter()
                            .enumerate()
                            .map(|(i, r)| {
                                sample_from(
                                    r,
                                    path.clone(),
                                    system.clone(),
                                    &format!("repeat-run{:02}", i + 1),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                print_samples(&format!("--repeat {} in one process:", runs), &samples);
            }
            Err(e) => {
                eprintln!("Runner execution failed: {}", e);
                exit(1);
            }
        }
    }
}
