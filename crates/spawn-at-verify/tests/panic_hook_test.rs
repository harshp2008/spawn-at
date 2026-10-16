//! The TUI's panic hook gives the terminal back before the panic message prints. Needs no
//! terminal: the "restore" step is a flag here, and it is a separate test binary so the
//! process-wide hook cannot touch other tests.
#![cfg(feature = "tui")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[test]
fn test_panic_runs_the_restore_step_and_keeps_the_previous_hook() {
    let restored = Arc::new(AtomicBool::new(false));
    let previous_ran = Arc::new(AtomicBool::new(false));
    let p = Arc::clone(&previous_ran);
    std::panic::set_hook(Box::new(move |_| p.store(true, Ordering::SeqCst)));
    let r = Arc::clone(&restored);
    spawn_at_verify::tui::install_panic_hook_with(Box::new(move || {
        // The message must come after this: when the restore runs, the previous hook has not.
        assert!(!r.load(Ordering::SeqCst));
        r.store(true, Ordering::SeqCst);
    }));
    let result = std::panic::catch_unwind(|| {
        panic!("boom in a draw");
    });
    let _ = std::panic::take_hook();
    assert!(result.is_err());
    assert!(restored.load(Ordering::SeqCst), "the restore step ran");
    assert!(
        previous_ran.load(Ordering::SeqCst),
        "the earlier hook still ran"
    );
}
