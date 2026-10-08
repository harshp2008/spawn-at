use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs");

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
        println!("cargo:rustc-env=SPAWN_AT_VERSION={}", clean_tag);
    } else {
        // Query current commit ID
        let commit_id = Command::new("git")
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
        println!("cargo:rustc-env=SPAWN_AT_LAST_RELEASE={}", last_release);
    }
}
