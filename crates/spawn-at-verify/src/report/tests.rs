//! Unit tests for report structures, redaction, and formatting.

use super::*;
use std::time::SystemTime;

use super::*;
use crate::runner::{AutoStatus, FinalStatus, HumanVerdict, TestResult};
use crate::schema::TestKind;

/// Tests qualification rule that skipped required tests prevent platform certification.
#[test]
fn test_skipped_is_never_a_pass() {
    let results = vec![
        TestResult {
            id: "preflight/binary-head".into(),
            title: "Preflight".into(),
            kind: TestKind::Auto,
            required: true,
            auto_status: AutoStatus::AutoPass,
            human_verdict: HumanVerdict::default(),
            final_status: FinalStatus::Pass,
            disagreement: false,
            duration_ms: 5,
            exit_code: Some(0),
            stdout: "".into(),
            stderr: "".into(),
            message: None,
            log_slice: None,
        },
        TestResult {
            id: "cloak/delayed-paint".into(),
            title: "Cloak test".into(),
            kind: TestKind::Both,
            required: true,
            auto_status: AutoStatus::Skipped,
            human_verdict: HumanVerdict::default(),
            final_status: FinalStatus::Skipped,
            disagreement: false,
            duration_ms: 0,
            exit_code: None,
            stdout: "".into(),
            stderr: "".into(),
            message: Some("Wayland only".into()),
            log_slice: None,
        },
    ];

    let qual = evaluate_qualification(&results);
    assert_eq!(qual.total_required, 2);
    assert_eq!(qual.passed_required, 1);
    assert_eq!(qual.skipped_required.len(), 1);
    // RULE: Skipped test prevents platform support qualification
    assert!(!qual.supported);
}

/// Tests command argument title redaction when include_titles is false.
#[test]
fn test_title_redaction() {
    let unredacted = "args: --title MyPrivateDocumentWindow --app-id org.test";
    let redacted = redact_titles(unredacted, false);
    assert!(redacted.contains("[REDACTED]"));
    assert!(!redacted.contains("MyPrivateDocumentWindow"));

    let preserved = redact_titles(unredacted, true);
    assert!(preserved.contains("MyPrivateDocumentWindow"));
}

/// Tests title redaction isolates only the title argument without altering other flags.
#[test]
fn test_title_redaction_does_not_alter_other_command_arguments() {
    let cmd = "spawn-at spawn --title \"Sensitive Document Title\" --anchor top-right --size 800 600 -m 20 --class org.gnome.Terminal --clamp true";
    let redacted = redact_titles(cmd, false);

    // Title itself must be redacted
    assert!(redacted.contains("--title \"[REDACTED]\""));
    assert!(!redacted.contains("Sensitive Document Title"));

    // All other arguments MUST be preserved exactly
    assert!(redacted.contains("--anchor top-right"));
    assert!(redacted.contains("--size 800 600"));
    assert!(redacted.contains("-m 20"));
    assert!(redacted.contains("--class org.gnome.Terminal"));
    assert!(redacted.contains("--clamp true"));

    // Check -t short flag with single-quoted title
    let short_cmd = "spawn-at spawn -t 'Private Budget' --monitor primary";
    let short_redacted = redact_titles(short_cmd, false);
    assert!(short_redacted.contains("-t \"[REDACTED]\""));
    assert!(!short_redacted.contains("Private Budget"));
    assert!(short_redacted.contains("--monitor primary"));

    // Check that flags containing 'title' in their name are NOT altered
    let title_flag_cmd = "spawn-at spawn --title-format standard --class title-editor";
    let title_flag_redacted = redact_titles(title_flag_cmd, false);
    assert_eq!(title_flag_cmd, title_flag_redacted);

    // Check JSON title redaction
    let json_text = r#"{"id": 42, "title": "Secret Bank Statement", "class": "calc", "w": 300}"#;
    let json_redacted = redact_titles(json_text, false);
    assert!(json_redacted.contains(r#""title": "[REDACTED]""#));
    assert!(!json_redacted.contains("Secret Bank Statement"));
    assert!(json_redacted.contains(r#""id": 42"#));
    assert!(json_redacted.contains(r#""class": "calc""#));
    assert!(json_redacted.contains(r#""w": 300"#));
}

/// Tests that real system hostname is queried successfully.
#[test]
fn test_get_real_hostname_returns_valid_name() {
    let host = get_real_hostname();
    assert!(!host.is_empty());
    assert!(!host.contains('\n'));
    assert!(!host.contains(' '));
}

/// Tests format and lexicographical sorting order of sortable timestamps.
#[test]
fn test_format_sortable_timestamp() {
    let t1 = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1700000000);
    let t2 = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1700000060);

    let s1 = format_sortable_timestamp(t1);
    let s2 = format_sortable_timestamp(t2);

    // Regex check for YYYYMMDD-HHMMSS (8 digits, hyphen, 6 digits)
    let re = regex::Regex::new(r"^\d{8}-\d{6}$").unwrap();
    assert!(re.is_match(&s1), "s1 '{}' should match YYYYMMDD-HHMMSS", s1);
    assert!(re.is_match(&s2), "s2 '{}' should match YYYYMMDD-HHMMSS", s2);

    // Sort order check: later time must sort lexicographically after earlier time
    assert!(
        s2 > s1,
        "Timestamp must preserve chronological sort order: {} > {}",
        s2,
        s1
    );
}

/// Tests sortable timestamp format across specific historical and leap-year test vectors.
#[test]
fn test_format_epoch_secs_sortable_test_vectors() {
    // Test vectors for sortable UTC timestamp format YYYYMMDD-HHMMSS:
    // 1. User test vector: epoch 1791559108 -> 20261009-151828 UTC
    assert_eq!(format_epoch_secs_sortable(1791559108), "20261009-151828");

    // 2. Epoch 0 -> 19700101-000000 UTC
    assert_eq!(format_epoch_secs_sortable(0), "19700101-000000");

    // 3. Leap-day test vector: epoch 1709208000 -> 20240229-120000 UTC
    // (2024 is a leap year; 2024-02-29 12:00:00 UTC = 1709208000)
    assert_eq!(format_epoch_secs_sortable(1709208000), "20240229-120000");
}
