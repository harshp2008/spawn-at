//! Unit tests for window tracker attribution, cap limits, and cleanup rules.

use super::state::{WindowRecord, WindowTracker, MAX_CONCURRENT_WINDOWS};
use std::collections::HashSet;

#[test]
fn test_tracker_never_closes_preexisting_windows() {
    let fake_preexisting = vec![101, 102, 103];
    let mut tracker = WindowTracker::new_with_preexisting(&fake_preexisting);

    // Harness spawns window 201 and 202
    assert!(tracker.register_window(201).is_ok());
    assert!(tracker.register_window(202).is_ok());

    let mut closed_ids = Vec::new();
    let failed = tracker.cleanup_with_closer(|id| {
        closed_ids.push(id);
        true
    });

    assert!(failed.is_empty());
    // Verify only test windows 201 and 202 were targeted
    assert_eq!(closed_ids, vec![201, 202]);
    // Verify preexisting windows 101, 102, 103 were never passed to closer
    for pre in &fake_preexisting {
        assert!(!closed_ids.contains(pre));
        assert!(tracker.known_preexisting.contains(pre));
    }
}

#[test]
fn test_runner_cleanup_only_ever_passes_registered_ids_to_close() {
    // Behavioral test: The runner cleanup path only ever passes registered IDs to close.
    // It never passes pre-existing window IDs, never passes external/unregistered window IDs,
    // and only takes strict numeric u64 IDs (cannot pass class names or PIDs).
    let preexisting = vec![101, 102];
    let mut tracker = WindowTracker::new_with_preexisting(&preexisting);

    // System has preexisting windows (101, 102), a registered test window (201),
    // and an external window (999) that appeared mid-test.
    let current_windows = vec![
        WindowRecord {
            id: 101,
            pid: Some(10),
            app_id: Some("preexisting".into()),
            class: Some("preexisting".into()),
            title: Some("Preexisting 1".into()),
        },
        WindowRecord {
            id: 201,
            pid: Some(20),
            app_id: Some("spawn-at-fixture".into()),
            class: Some("spawn-at-fixture".into()),
            title: Some("Fixture".into()),
        },
        WindowRecord {
            id: 999,
            pid: Some(30),
            app_id: Some("chrome".into()),
            class: Some("chrome".into()),
            title: Some("External Chrome".into()),
        },
    ];

    // Attribute windows for fixture spawn
    let newly = tracker
        .attribute_and_register_test_windows(&current_windows, None, Some("fixture"))
        .unwrap();
    assert_eq!(newly, vec![201]);

    // Execute the cleanup path (identical to runner::cleanup_test_windows)
    let mut closed_ids: Vec<u64> = Vec::new();
    let failed = tracker.cleanup_with_closer(|id| {
        closed_ids.push(id);
        true
    });

    assert!(failed.is_empty());
    // Verify ONLY the registered window ID 201 was passed to the closer
    assert_eq!(closed_ids, vec![201]);
    // Preexisting and external windows were never passed to closer
    assert!(!closed_ids.contains(&101));
    assert!(!closed_ids.contains(&102));
    assert!(!closed_ids.contains(&999));
}

#[test]
fn test_tracker_ignores_external_window_appearing_mid_test() {
    // A window that appears mid-test from another source is never registered or closed.
    let preexisting = vec![10, 20];
    let mut tracker = WindowTracker::new_with_preexisting(&preexisting);

    // Current windows in system during a test with subject = "fixture":
    // - 10: preexisting
    // - 20: preexisting
    // - 100: spawned fixture window
    // - 999: external window opened mid-test (e.g. user opened Chrome or terminal externally)
    let current_windows = vec![
        WindowRecord {
            id: 10,
            pid: Some(1000),
            app_id: Some("app1".into()),
            class: Some("app1".into()),
            title: Some("App 1".into()),
        },
        WindowRecord {
            id: 20,
            pid: Some(1001),
            app_id: Some("app2".into()),
            class: Some("app2".into()),
            title: Some("App 2".into()),
        },
        WindowRecord {
            id: 100,
            pid: Some(2000),
            app_id: Some("spawn-at-fixture".into()),
            class: Some("spawn-at-fixture".into()),
            title: Some("Fixture Window".into()),
        },
        WindowRecord {
            id: 999,
            pid: Some(3000),
            app_id: Some("google-chrome".into()),
            class: Some("google-chrome".into()),
            title: Some("Chrome Window".into()),
        },
    ];

    // Attribute windows for fixture test
    let registered = tracker
        .attribute_and_register_test_windows(&current_windows, None, Some("fixture"))
        .expect("Registration should succeed");

    // Only fixture window 100 is registered
    assert_eq!(registered, vec![100]);
    assert_eq!(tracker.tracked_test_windows, vec![100]);
    // External window 999 is NEVER registered
    assert!(!tracker.tracked_test_windows.contains(&999));

    // When cleanup runs, external window 999 is NEVER closed
    let mut closed_ids = Vec::new();
    tracker.cleanup_with_closer(|id| {
        closed_ids.push(id);
        true
    });

    assert_eq!(closed_ids, vec![100]);
    assert!(!closed_ids.contains(&999));
}

#[test]
fn test_tracker_respects_20_window_cap() {
    let mut tracker = WindowTracker::new_with_preexisting(&[]);

    // Register exactly MAX_CONCURRENT_WINDOWS (20)
    for i in 1..=MAX_CONCURRENT_WINDOWS as u64 {
        assert!(tracker.register_window(i).is_ok());
    }

    // 21st window registration must fail with concurrency cap error
    let err = tracker.register_window(999);
    assert!(err.is_err());
    assert!(err.unwrap_err().contains("Concurrency cap reached"));
}

#[test]
fn test_tracker_does_not_close_unlaunched_windows() {
    let fake_preexisting = vec![1, 2, 3];
    let mut tracker = WindowTracker::new_with_preexisting(&fake_preexisting);

    // Current system state includes pre-existing, one test-launched window,
    // and an external unlaunched window (e.g. user opened a browser outside test)
    let current_windows = vec![
        WindowRecord {
            id: 1,
            pid: Some(10),
            app_id: None,
            class: None,
            title: None,
        },
        WindowRecord {
            id: 2,
            pid: Some(11),
            app_id: None,
            class: None,
            title: None,
        },
        WindowRecord {
            id: 3,
            pid: Some(12),
            app_id: None,
            class: None,
            title: None,
        },
        WindowRecord {
            id: 50,
            pid: Some(500),
            app_id: Some("fixture".into()),
            class: Some("fixture".into()),
            title: None,
        },
        WindowRecord {
            id: 99,
            pid: Some(990),
            app_id: Some("unrelated".into()),
            class: Some("unrelated".into()),
            title: None,
        },
    ];

    let newly = tracker
        .attribute_and_register_test_windows(&current_windows, None, Some("fixture"))
        .unwrap();
    assert_eq!(newly, vec![50]);

    let mut closed_ids = Vec::new();
    tracker.cleanup_with_closer(|id| {
        closed_ids.push(id);
        true
    });

    // 50 was closed, but 99 was never registered and never closed
    assert_eq!(closed_ids, vec![50]);
    assert!(!closed_ids.contains(&99));
}

#[test]
fn test_window_of_same_class_opened_between_session_start_and_action_never_closed() {
    // Startup snapshot has window 10
    let startup_preexisting = vec![10];
    let mut tracker = WindowTracker::new_with_preexisting(&startup_preexisting);

    // User / external process opens a window of the SAME class (terminal)
    // after session start, but BEFORE this test's action runs.
    let external_terminal = WindowRecord {
        id: 20,
        pid: Some(2000),
        app_id: Some("org.gnome.Terminal".into()),
        class: Some("org.gnome.Terminal".into()),
        title: Some("User Terminal".into()),
    };

    // Fresh snapshot taken immediately before this test's action:
    let mut pre_action_snapshot = HashSet::new();
    pre_action_snapshot.insert(10);
    pre_action_snapshot.insert(20);

    // Action runs: spawns a new terminal window 30
    let test_terminal = WindowRecord {
        id: 30,
        pid: Some(3000),
        app_id: Some("org.gnome.Terminal".into()),
        class: Some("org.gnome.Terminal".into()),
        title: Some("Spawned Test Terminal".into()),
    };

    let current_windows = vec![
        WindowRecord {
            id: 10,
            pid: Some(1000),
            app_id: None,
            class: None,
            title: None,
        },
        external_terminal,
        test_terminal,
    ];

    let outcome = tracker
        .attribute_action_windows(
            &pre_action_snapshot,
            &current_windows,
            None,
            Some("system:org.gnome.Terminal"),
            None,
        )
        .expect("Attribution should succeed");

    // Window 20 was in pre_action_snapshot, so ONLY window 30 was a candidate and registered.
    assert_eq!(outcome.registered_id, Some(30));
    assert_eq!(outcome.candidate_ids, vec![30]);
    assert_eq!(outcome.error_message, None);

    // Verify cleanup: window 20 is NEVER closed!
    let mut closed_ids = Vec::new();
    tracker.cleanup_with_closer(|id| {
        closed_ids.push(id);
        true
    });

    assert_eq!(closed_ids, vec![30]);
    assert!(!closed_ids.contains(&20));
    assert!(!closed_ids.contains(&10));
}

#[test]
fn test_two_candidates_nothing_closed() {
    let startup_preexisting = vec![1];
    let mut tracker = WindowTracker::new_with_preexisting(&startup_preexisting);
    let pre_action_snapshot = HashSet::from([1]);

    // Action results in TWO candidate windows of matching subject
    let current_windows = vec![
        WindowRecord {
            id: 1,
            pid: Some(10),
            app_id: None,
            class: None,
            title: None,
        },
        WindowRecord {
            id: 101,
            pid: Some(1010),
            app_id: Some("gnome-calculator".into()),
            class: Some("gnome-calculator".into()),
            title: Some("Calc 1".into()),
        },
        WindowRecord {
            id: 102,
            pid: Some(1020),
            app_id: Some("gnome-calculator".into()),
            class: Some("gnome-calculator".into()),
            title: Some("Calc 2".into()),
        },
    ];

    let outcome = tracker
        .attribute_action_windows(
            &pre_action_snapshot,
            &current_windows,
            None,
            Some("system:gnome-calculator"),
            None,
        )
        .expect("Attribution evaluation should succeed");

    // When multiple candidates appear: register nothing
    assert_eq!(outcome.registered_id, None);
    assert_eq!(outcome.candidate_ids, vec![101, 102]);
    assert!(outcome.error_message.is_some());
    let err = outcome.error_message.unwrap();
    assert!(err.contains("could not attribute; windows left open"));
    assert!(err.contains("101") && err.contains("102"));

    // Cleanup: NOTHING closed
    let mut closed_ids = Vec::new();
    tracker.cleanup_with_closer(|id| {
        closed_ids.push(id);
        true
    });
    assert!(closed_ids.is_empty());
}

#[test]
fn test_zero_candidates_nothing_closed() {
    let startup_preexisting = vec![1];
    let mut tracker = WindowTracker::new_with_preexisting(&startup_preexisting);
    let pre_action_snapshot = HashSet::from([1]);

    // Action results in NO new windows matching subject
    let current_windows = vec![WindowRecord {
        id: 1,
        pid: Some(10),
        app_id: None,
        class: None,
        title: None,
    }];

    let outcome = tracker
        .attribute_action_windows(
            &pre_action_snapshot,
            &current_windows,
            None,
            Some("system:gnome-calculator"),
            None,
        )
        .expect("Attribution evaluation should succeed");

    // When zero candidates appear: register nothing
    assert_eq!(outcome.registered_id, None);
    assert!(outcome.candidate_ids.is_empty());
    assert!(outcome.error_message.is_some());
    let err = outcome.error_message.unwrap();
    assert!(err.contains("could not attribute; windows left open"));

    // Cleanup: NOTHING closed
    let mut closed_ids = Vec::new();
    tracker.cleanup_with_closer(|id| {
        closed_ids.push(id);
        true
    });
    assert!(closed_ids.is_empty());
}

#[test]
fn test_fixture_attribution_by_pid() {
    let startup_preexisting = vec![10];
    let mut tracker = WindowTracker::new_with_preexisting(&startup_preexisting);
    let pre_action_snapshot = HashSet::from([10]);

    // Harness spawns fixture with PID 4000
    let fixture_pid = 4000;

    let current_windows = vec![
        WindowRecord {
            id: 10,
            pid: Some(100),
            app_id: None,
            class: None,
            title: None,
        },
        // Unrelated window created during test
        WindowRecord {
            id: 50,
            pid: Some(3000),
            app_id: Some("other".into()),
            class: Some("other".into()),
            title: None,
        },
        // Fixture window with matching PID
        WindowRecord {
            id: 42,
            pid: Some(fixture_pid),
            app_id: Some("fixture".into()),
            class: Some("fixture".into()),
            title: Some("Fixture Window".into()),
        },
    ];

    let outcome = tracker
        .attribute_action_windows(
            &pre_action_snapshot,
            &current_windows,
            Some(fixture_pid),
            Some("fixture"),
            None,
        )
        .expect("Attribution should succeed");

    assert_eq!(outcome.registered_id, Some(42));
    assert_eq!(outcome.candidate_ids, vec![42]);

    let mut closed_ids = Vec::new();
    tracker.cleanup_with_closer(|id| {
        closed_ids.push(id);
        true
    });
    assert_eq!(closed_ids, vec![42]);
}
