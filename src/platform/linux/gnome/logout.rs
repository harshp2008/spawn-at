//! Decides whether a GNOME Wayland install still needs a log-out.
//!
//! A changed extension only loads at the next log-in on Wayland. When the extension about to be
//! deployed is byte-identical to the deployed one, the extension is ACTIVE, and the deployed
//! files are older than the running gnome-shell process, the running Shell has already loaded
//! exactly this code and the log-out advice can be dropped. Every other case, and every fact
//! that cannot be read, keeps the advice. The facts sit behind [`ExtensionFacts`] so tests can
//! inject them; [`RealFacts`] reads the real extension directory, `gnome-extensions info` and /proc.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A file found in the deployed extension directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeployedFile {
    pub name: String,
    pub bytes: Vec<u8>,
    pub modified: SystemTime,
}

/// The facts the decision needs. `Err` means "could not be read".
pub trait ExtensionFacts {
    /// The regular files of the deployed extension directory; `None` when it does not exist.
    /// A sub-directory or other non-regular entry is reported as an error-free extra file by
    /// the real implementation (name ending in `/`).
    fn deployed_files(&self) -> Result<Option<Vec<DeployedFile>>, String>;
    /// True when `gnome-extensions info` reports the state ACTIVE.
    fn is_active(&self) -> Result<bool, String>;
    /// When the running gnome-shell process of this user started.
    fn shell_started(&self) -> Result<SystemTime, String>;
}

/// What the install should tell the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogoutNeed {
    /// Files identical, extension active, files older than the running Shell: no log-out.
    NotNeeded,
    /// Keep today's behaviour (ask and warn); the text says which fact decided.
    Needed(String),
}

/// Safety margin: file times must be at least this much before the Shell's start time, because
/// the start time is derived from the boot time and clock ticks and is not exact.
const START_MARGIN: Duration = Duration::from_secs(2);

/// Applies the rule. `expected` is the variant the install is about to deploy.
pub fn decide(facts: &dyn ExtensionFacts, expected: &[(&str, &str)]) -> LogoutNeed {
    let deployed = match facts.deployed_files() {
        Ok(Some(d)) => d,
        Ok(None) => return LogoutNeed::Needed("first install: no deployed extension".into()),
        Err(e) => return LogoutNeed::Needed(format!("cannot read the deployed files: {e}")),
    };
    if deployed.len() != expected.len() {
        return LogoutNeed::Needed("the deployed file set differs (extra or missing file)".into());
    }
    for (name, content) in expected {
        match deployed.iter().find(|d| d.name == *name) {
            None => return LogoutNeed::Needed(format!("{name} is missing from the deployment")),
            Some(d) if d.bytes != content.as_bytes() => {
                return LogoutNeed::Needed(format!("{name} changed"))
            }
            Some(_) => {}
        }
    }
    match facts.is_active() {
        Ok(true) => {}
        Ok(false) => return LogoutNeed::Needed("the extension is not active".into()),
        Err(e) => return LogoutNeed::Needed(format!("cannot read the extension state: {e}")),
    }
    let started = match facts.shell_started() {
        Ok(t) => t,
        Err(e) => return LogoutNeed::Needed(format!("cannot read the Shell start time: {e}")),
    };
    let newest = deployed.iter().map(|d| d.modified).max();
    match newest {
        Some(t) if t + START_MARGIN <= started => LogoutNeed::NotNeeded,
        Some(_) => LogoutNeed::Needed(
            "the extension files are newer than the running Shell (not loaded yet)".into(),
        ),
        None => LogoutNeed::Needed("no deployed files".into()),
    }
}

/// Reads the State line of `gnome-extensions info <uuid>`: `Some(true)` for ACTIVE.
pub fn parse_info_active(info: &str) -> Option<bool> {
    info.lines().find_map(|l| {
        let (key, value) = l.trim().split_once(':')?;
        (key.trim() == "State").then(|| value.trim().eq_ignore_ascii_case("ACTIVE"))
    })
}

/// Start time of a process from the text of `/proc/<pid>/stat`, the boot time (`btime` of
/// /proc/stat, seconds since the epoch) and the clock ticks per second.
pub fn parse_proc_start(stat: &str, btime: u64, ticks_per_sec: u64) -> Option<SystemTime> {
    // The command name is in parentheses and may contain spaces; the fields after the last ')'
    // start at field 3, so starttime (field 22) is the 20th one after it.
    let after = &stat[stat.rfind(')')? + 1..];
    let ticks: u64 = after.split_whitespace().nth(19)?.parse().ok()?;
    if ticks_per_sec == 0 {
        return None;
    }
    let ms = ticks * 1000 / ticks_per_sec;
    Some(UNIX_EPOCH + Duration::from_secs(btime) + Duration::from_millis(ms))
}

/// Reads the facts from this machine.
pub struct RealFacts {
    pub ext_dir: PathBuf,
    pub uuid: &'static str,
}

impl ExtensionFacts for RealFacts {
    fn deployed_files(&self) -> Result<Option<Vec<DeployedFile>>, String> {
        let entries = match fs::read_dir(&self.ext_dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.to_string()),
        };
        let mut out = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let meta = entry.metadata().map_err(|e| e.to_string())?;
            if !meta.is_file() {
                out.push(DeployedFile {
                    name: format!("{name}/"),
                    bytes: Vec::new(),
                    modified: UNIX_EPOCH,
                });
                continue;
            }
            out.push(DeployedFile {
                bytes: fs::read(entry.path()).map_err(|e| e.to_string())?,
                modified: meta.modified().map_err(|e| e.to_string())?,
                name,
            });
        }
        Ok(Some(out))
    }

    fn is_active(&self) -> Result<bool, String> {
        let o = Command::new("gnome-extensions")
            .args(["info", self.uuid])
            .output()
            .map_err(|e| e.to_string())?;
        if !o.status.success() {
            return Err(format!("gnome-extensions info exited with {}", o.status));
        }
        parse_info_active(&String::from_utf8_lossy(&o.stdout))
            .ok_or_else(|| "no State line in the gnome-extensions info output".to_string())
    }

    fn shell_started(&self) -> Result<SystemTime, String> {
        use std::os::unix::fs::MetadataExt;
        // SAFETY: getuid has no preconditions.
        let uid = unsafe { libc::getuid() };
        let mut pids = Vec::new();
        for entry in fs::read_dir("/proc").map_err(|e| e.to_string())?.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.bytes().all(|b| b.is_ascii_digit()) {
                continue;
            }
            let Ok(comm) = fs::read_to_string(entry.path().join("comm")) else {
                continue;
            };
            if comm.trim() != "gnome-shell" {
                continue;
            }
            if entry.metadata().map(|m| m.uid()).ok() == Some(uid) {
                pids.push(name);
            }
        }
        let [pid] = pids.as_slice() else {
            return Err(format!(
                "expected one gnome-shell process of this user, found {}",
                pids.len()
            ));
        };
        let stat = fs::read_to_string(format!("/proc/{pid}/stat")).map_err(|e| e.to_string())?;
        let proc_stat = fs::read_to_string("/proc/stat").map_err(|e| e.to_string())?;
        let btime: u64 = proc_stat
            .lines()
            .find_map(|l| l.strip_prefix("btime "))
            .and_then(|v| v.trim().parse().ok())
            .ok_or("no btime in /proc/stat")?;
        // SAFETY: sysconf has no preconditions.
        let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
        parse_proc_start(&stat, btime, ticks.max(0) as u64)
            .ok_or_else(|| "cannot parse /proc/<pid>/stat".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPECTED: &[(&str, &str)] = &[("a.js", "one"), ("b.js", "two")];

    struct Fake {
        deployed: Result<Option<Vec<DeployedFile>>, String>,
        active: Result<bool, String>,
        started: Result<SystemTime, String>,
    }

    impl ExtensionFacts for Fake {
        fn deployed_files(&self) -> Result<Option<Vec<DeployedFile>>, String> {
            self.deployed.clone()
        }
        fn is_active(&self) -> Result<bool, String> {
            self.active.clone()
        }
        fn shell_started(&self) -> Result<SystemTime, String> {
            self.started.clone()
        }
    }

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn file(name: &str, content: &str, modified: u64) -> DeployedFile {
        DeployedFile {
            name: name.into(),
            bytes: content.as_bytes().to_vec(),
            modified: at(modified),
        }
    }

    /// Identical files, written at t=100, Shell started at t=1000, extension active.
    fn good() -> Fake {
        Fake {
            deployed: Ok(Some(vec![
                file("a.js", "one", 100),
                file("b.js", "two", 90),
            ])),
            active: Ok(true),
            started: Ok(at(1000)),
        }
    }

    fn needed(f: &Fake) -> String {
        match decide(f, EXPECTED) {
            LogoutNeed::Needed(why) => why,
            LogoutNeed::NotNeeded => panic!("expected Needed"),
        }
    }

    /// Identical + active + older than the Shell start: the only case that skips the log-out.
    #[test]
    fn test_identical_active_older_than_shell_skips() {
        assert_eq!(decide(&good(), EXPECTED), LogoutNeed::NotNeeded);
    }

    /// No deployed directory: first install, keep the advice.
    #[test]
    fn test_first_install_needs_logout() {
        let mut f = good();
        f.deployed = Ok(None);
        assert!(needed(&f).contains("first install"));
    }

    /// One file differs by a single byte.
    #[test]
    fn test_changed_file_needs_logout() {
        let mut f = good();
        f.deployed = Ok(Some(vec![
            file("a.js", "onE", 100),
            file("b.js", "two", 90),
        ]));
        assert!(needed(&f).contains("a.js changed"));
    }

    /// Identical files written after the Shell started: the Shell has not loaded them.
    #[test]
    fn test_installed_after_shell_start_needs_logout() {
        let mut f = good();
        f.deployed = Ok(Some(vec![
            file("a.js", "one", 1500),
            file("b.js", "two", 90),
        ]));
        assert!(needed(&f).contains("newer than the running Shell"));
    }

    /// A file written within the 2 s margin before the Shell start does not count as older.
    #[test]
    fn test_file_inside_the_start_margin_needs_logout() {
        let mut f = good();
        f.deployed = Ok(Some(vec![
            file("a.js", "one", 999),
            file("b.js", "two", 90),
        ]));
        assert!(needed(&f).contains("newer than the running Shell"));
        f.deployed = Ok(Some(vec![
            file("a.js", "one", 998),
            file("b.js", "two", 90),
        ]));
        assert_eq!(decide(&f, EXPECTED), LogoutNeed::NotNeeded);
    }

    /// Identical files but the extension is not ACTIVE.
    #[test]
    fn test_not_active_needs_logout() {
        let mut f = good();
        f.active = Ok(false);
        assert!(needed(&f).contains("not active"));
    }

    /// An extra file, a missing file and a sub-directory all change the set.
    #[test]
    fn test_extra_or_missing_file_needs_logout() {
        let mut f = good();
        f.deployed = Ok(Some(vec![
            file("a.js", "one", 100),
            file("b.js", "two", 90),
            file("extra.js", "x", 90),
        ]));
        assert!(needed(&f).contains("extra or missing"));
        f.deployed = Ok(Some(vec![file("a.js", "one", 100)]));
        assert!(needed(&f).contains("extra or missing"));
        f.deployed = Ok(Some(vec![
            file("a.js", "one", 100),
            file("c.js", "two", 90),
        ]));
        assert!(needed(&f).contains("b.js is missing"));
        f.deployed = Ok(Some(vec![file("a.js", "one", 100), file("sub/", "", 0)]));
        assert!(needed(&f).contains("b.js is missing"));
    }

    /// Any fact that cannot be read keeps the advice, each with its own reason.
    #[test]
    fn test_read_errors_need_logout() {
        let mut f = good();
        f.deployed = Err("permission denied".into());
        assert!(needed(&f).contains("cannot read the deployed files"));
        let mut f = good();
        f.active = Err("no gnome-extensions".into());
        assert!(needed(&f).contains("cannot read the extension state"));
        let mut f = good();
        f.started = Err("no gnome-shell".into());
        assert!(needed(&f).contains("cannot read the Shell start time"));
    }

    /// The State line of gnome-extensions info decides; other states and a missing line do not
    /// count as active.
    #[test]
    fn test_parse_info_active() {
        let info =
            "spawn-at@harsh.local\n  Name: Spawn-At Interceptor\n  Enabled: Yes\n  State: ACTIVE\n";
        assert_eq!(parse_info_active(info), Some(true));
        assert_eq!(parse_info_active("  State: INACTIVE\n"), Some(false));
        assert_eq!(parse_info_active("  State: OUT OF DATE\n"), Some(false));
        assert_eq!(parse_info_active("  Name: x\n"), None);
    }

    /// starttime is field 22 of /proc/<pid>/stat, counted after the last ')' (the command name
    /// may contain spaces and parentheses). 123456 ticks at 100 Hz = 1234.56 s after btime.
    #[test]
    fn test_parse_proc_start() {
        let stat = "4242 (gnome (shell)) S 1 4242 4242 0 -1 4194560 100 0 0 0 5 6 0 0 20 0 9 0 123456 1000 100 18446744073709551615";
        assert_eq!(
            parse_proc_start(stat, 1_000_000, 100),
            Some(UNIX_EPOCH + Duration::from_millis(1_000_000_000 + 1_234_560))
        );
        assert_eq!(parse_proc_start("garbage", 1, 100), None);
        assert_eq!(parse_proc_start(stat, 1, 0), None);
    }

    /// The parser agrees with the clock on a real process: this test process started at most a
    /// minute ago and not in the future (reads only /proc/self/stat and /proc/stat).
    #[test]
    fn test_parse_proc_start_on_this_process() {
        let stat = fs::read_to_string("/proc/self/stat").unwrap();
        let btime: u64 = fs::read_to_string("/proc/stat")
            .unwrap()
            .lines()
            .find_map(|l| l.strip_prefix("btime "))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) } as u64;
        let started = parse_proc_start(&stat, btime, ticks).unwrap();
        let now = SystemTime::now();
        assert!(started <= now, "{started:?} > {now:?}");
        assert!(now.duration_since(started).unwrap() < Duration::from_secs(60));
    }

    /// RealFacts reads a real directory: missing means first install, files carry their bytes
    /// and times, a sub-directory shows up as an extra entry. Temporary directory only.
    #[test]
    fn test_real_facts_read_a_temp_directory() {
        let dir = std::env::temp_dir().join(format!("spawn-at-logout-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let facts = RealFacts {
            ext_dir: dir.join("ext"),
            uuid: "x@y",
        };
        assert_eq!(facts.deployed_files(), Ok(None));
        fs::create_dir_all(dir.join("ext/sub")).unwrap();
        fs::write(dir.join("ext/a.js"), "one").unwrap();
        let mut files = facts.deployed_files().unwrap().unwrap();
        files.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].name, "a.js");
        assert_eq!(files[0].bytes, b"one");
        assert_eq!(files[1].name, "sub/");
        let _ = fs::remove_dir_all(&dir);
    }
}
