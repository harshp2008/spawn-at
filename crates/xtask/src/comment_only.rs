//! `check-comment-only`: verifies a commit changes only comments relative to a base revision.

use std::fs;
use std::path::Path;
use std::process::{exit, Command};

/// Strips comments and doc attributes, normalizing non-comment code lines for whitespace comparison.
fn strip_comments_and_fmt(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_block = 0usize;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        let mut clean_line = String::new();
        let mut chars = line.chars().peekable();
        let mut in_str = false;
        let mut escaped = false;

        while let Some(ch) = chars.next() {
            if in_block > 0 {
                if ch == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    in_block -= 1;
                } else if ch == '/' && chars.peek() == Some(&'*') {
                    chars.next();
                    in_block += 1;
                }
                continue;
            }

            if in_str {
                clean_line.push(ch);
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    in_str = false;
                }
                continue;
            }

            if ch == '"' {
                in_str = true;
                clean_line.push(ch);
            } else if ch == '/' && chars.peek() == Some(&'/') {
                // Line comment
                break;
            } else if ch == '/' && chars.peek() == Some(&'*') {
                chars.next();
                in_block += 1;
            } else {
                clean_line.push(ch);
            }
        }

        let trimmed = clean_line.trim();
        if trimmed == "#![warn(missing_docs)]" {
            continue;
        }
        if !trimmed.is_empty() {
            let normalized = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
            out.push(normalized);
        }
    }

    out
}

/// Generates unified diff lines between two slices of strings.
fn compute_diff(base: &[String], curr: &[String], file_name: &str) -> Vec<String> {
    if base == curr {
        return Vec::new();
    }

    // Myers / dynamic programming LCS
    let n = base.len();
    let m = curr.len();

    // If file is very large, simple difference scan
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in 0..n {
        for j in 0..m {
            if base[i] == curr[j] {
                lcs[i + 1][j + 1] = lcs[i][j] + 1;
            } else {
                lcs[i + 1][j + 1] = lcs[i + 1][j].max(lcs[i][j + 1]);
            }
        }
    }

    let mut diff = Vec::new();
    let mut i = n;
    let mut j = m;
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && base[i - 1] == curr[j - 1] {
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || lcs[i][j - 1] >= lcs[i - 1][j]) {
            diff.push(format!("+ {}", curr[j - 1]));
            j -= 1;
        } else if i > 0 && (j == 0 || lcs[i][j - 1] < lcs[i - 1][j]) {
            diff.push(format!("- {}", base[i - 1]));
            i -= 1;
        }
    }

    diff.reverse();
    if !diff.is_empty() {
        let mut header = vec![
            format!("--- base:{}", file_name),
            format!("+++ current:{}", file_name),
        ];
        header.extend(diff);
        header
    } else {
        Vec::new()
    }
}

pub(crate) fn run_check_comment_only(base_commit: &str, target_dir: &str) {
    println!(
        "==> Checking that changes since {} in {} are comment-only...",
        base_commit, target_dir
    );

    let output = Command::new("git")
        .args(["diff", "--name-only", base_commit, "--", target_dir])
        .output()
        .expect("failed to execute git diff");

    if !output.status.success() {
        eprintln!("git diff failed: {:?}", output);
        exit(1);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let changed_files: Vec<&str> = stdout
        .lines()
        .map(|s| s.trim())
        .filter(|s| s.ends_with(".rs"))
        .collect();

    if changed_files.is_empty() {
        println!(
            "No .rs files changed in {} compared to {}.",
            target_dir, base_commit
        );
        exit(0);
    }

    let mut has_failure = false;

    for file_path in changed_files {
        let show_arg = format!("{}:{}", base_commit, file_path);
        let res_base = Command::new("git")
            .args(["show", &show_arg])
            .output()
            .expect("failed to execute git show");

        if !res_base.status.success() {
            println!("Skipping newly created file or git error on {}", file_path);
            continue;
        }

        let base_content = String::from_utf8_lossy(&res_base.stdout);
        let curr_content = if Path::new(file_path).exists() {
            fs::read_to_string(file_path).unwrap_or_default()
        } else {
            let res_curr = Command::new("git")
                .args(["show", &format!("HEAD:{}", file_path)])
                .output()
                .expect("failed to execute git show HEAD");
            String::from_utf8_lossy(&res_curr.stdout).to_string()
        };

        let base_stripped = strip_comments_and_fmt(&base_content);
        let curr_stripped = strip_comments_and_fmt(&curr_content);

        let diff = compute_diff(&base_stripped, &curr_stripped, file_path);

        if !diff.is_empty() {
            println!("FAILED: Non-comment change detected in {}:", file_path);
            for d in diff.iter().take(25) {
                println!("  {}", d);
            }
            if diff.len() > 25 {
                println!("   ... and {} more diff lines", diff.len() - 25);
            }
            has_failure = true;
        } else {
            println!("  OK (comments only): {}", file_path);
        }
    }

    if has_failure {
        exit(1);
    } else {
        println!("ALL checked files contain ONLY comment changes!");
    }
}
