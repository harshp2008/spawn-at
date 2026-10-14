use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=crates");

    if std::path::Path::new(".git").exists() {
        println!("cargo:rerun-if-changed=.git/HEAD");
        println!("cargo:rerun-if-changed=.git/index");
        println!("cargo:rerun-if-changed=.git/refs");
        if let Ok(head_content) = std::fs::read_to_string(".git/HEAD") {
            if let Some(ref_path) = head_content.strip_prefix("ref: ") {
                let p = format!(".git/{}", ref_path.trim());
                println!("cargo:rerun-if-changed={}", p);
            }
        }
    }

    // Check if working tree is dirty (modified, staged, untracked)
    let is_dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);

    // Compute short hash of git diff HEAD
    let diff_hash = Command::new("git")
        .args(["diff", "HEAD"])
        .output()
        .ok()
        .map(|o| {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            o.stdout.hash(&mut hasher);
            format!("{:07x}", hasher.finish() & 0xfffffff)
        })
        .unwrap_or_else(|| "0000000".to_string());

    println!(
        "cargo:rustc-env=SPAWN_AT_DIRTY={}",
        if is_dirty { "true" } else { "false" }
    );
    println!("cargo:rustc-env=SPAWN_AT_DIFF_HASH={}", diff_hash);

    // Check if HEAD exactly matches a release git tag
    let exact_tag = Command::new("git")
        .args(["describe", "--tags", "--exact-match"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if !s.is_empty() {
                    Some(s)
                } else {
                    None
                }
            } else {
                None
            }
        });

    if let Some(tag) = exact_tag {
        let clean_tag = tag.strip_prefix('v').unwrap_or(&tag);
        let version_str = if is_dirty {
            format!("{}-dirty", clean_tag)
        } else {
            clean_tag.to_string()
        };
        println!("cargo:rustc-env=SPAWN_AT_VERSION={}", version_str);
    } else {
        // Query current commit ID
        let raw_commit = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "unknown".to_string());

        let short_commit = if raw_commit.len() >= 7 {
            &raw_commit[..7]
        } else {
            &raw_commit
        };

        let commit_id = if is_dirty {
            format!("{}-dirty", short_commit)
        } else {
            short_commit.to_string()
        };

        // Query last release tag
        let last_release = Command::new("git")
            .args(["describe", "--tags", "--abbrev=0"])
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "v0.1.0".to_string());

        println!("cargo:rustc-env=SPAWN_AT_COMMIT_ID={}", commit_id);
        println!("cargo:rustc-env=SPAWN_AT_RAW_COMMIT_ID={}", raw_commit);
        println!("cargo:rustc-env=SPAWN_AT_LAST_RELEASE={}", last_release);
    }
}
