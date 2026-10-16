//! The category order of the Tests sidebar, built from the real `verify/` catalog (not a list
//! made up here). The order is defined once, in `tui::app::category_rank`; an earlier change
//! (734ffe5) reordered the drawer sections instead and left this order as it was.

use super::{create_test_app, dump_buffer_to_string};
use ratatui::{backend::TestBackend, Terminal};
use spawn_at_verify::tui::views::{draw_ui, get_test_category};

const EXPECTED: [&str; 8] = [
    "PREFLIGHT",
    "FOCUS",
    "QUERIES",
    "ROBUSTNESS",
    "PLACEMENT",
    "CLOAK & DELAYED PAINT",
    "TRANSFORMS",
    "GENERAL",
];

/// The groups, in order, that the app's filtered list yields from the real catalog.
#[test]
fn test_sidebar_groups_come_in_the_chosen_order() {
    let app = create_test_app();
    let mut seen: Vec<String> = Vec::new();
    for t in app.filtered_tests() {
        let name = get_test_category(t.category()).to_uppercase();
        if seen.last() != Some(&name) {
            assert!(
                !seen.contains(&name),
                "{name} appears in two places: {seen:?}"
            );
            seen.push(name);
        }
    }
    assert_eq!(seen, EXPECTED);
}

/// The headings as drawn: FOCUS is the second heading, directly after PREFLIGHT, and the rest
/// follow in the same order, top to bottom of the sidebar (all groups expanded, tall terminal).
#[test]
fn test_drawn_sidebar_headings_are_in_order() {
    let mut app = create_test_app();
    app.fixture_exists = true;
    let (w, h) = (150u16, 120u16);
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    terminal.draw(|f| draw_ui(f, &app)).unwrap();
    let text = dump_buffer_to_string(&terminal, w, h);
    let mut at = Vec::new();
    for name in EXPECTED {
        let row = text
            .lines()
            .position(|l| l.contains(&format!("│  {name} ")) || l.contains(&format!("│  {name}  ")))
            .unwrap_or_else(|| panic!("no {name} heading in:\n{text}"));
        at.push(row);
    }
    assert!(at.windows(2).all(|p| p[0] < p[1]), "{at:?}");
}
