//! Keys bar completeness at every width: no shortcut action may silently disappear.

use super::{create_test_app, dump_buffer_to_string};
use ratatui::{backend::TestBackend, Terminal};
use spawn_at_verify::tui::views::{draw_ui, render_key_grid, DRAWER_KEYS, VERDICT_KEYS};

/// Every normal-mode action as rendered: key, one space, label.
const NORMAL_ACTIONS: &[&str] = &[
    "r Run",
    "a All",
    "c Run Cat",
    "j/k Move",
    "Tab Panel",
    "Space Fold",
    "Enter/d Drawer",
    "f Filter",
    "? Help",
    "q Quit",
];

/// Renders the full UI at `w`x30 and returns only the last `rows` terminal rows.
fn bottom_rows(app: &spawn_at_verify::tui::app::TuiApp, w: u16, rows: usize) -> String {
    let h = 30u16;
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    terminal.draw(|f| draw_ui(f, app)).unwrap();
    let full = dump_buffer_to_string(&terminal, w, h);
    let all: Vec<&str> = full.lines().collect();
    all[all.len() - rows..].join("\n")
}

/// Normal mode: all 10 actions at every width from 60 to 140. The spec calls out widths
/// 100..=104, where the one-row/two-row switch happens at 105.
#[test]
fn test_normal_footer_shows_every_action_at_every_width_60_to_140() {
    let mut app = create_test_app();
    app.fixture_exists = true;
    for w in 60u16..=140 {
        let bar = bottom_rows(&app, w, 4);
        for action in NORMAL_ACTIONS {
            assert!(
                bar.contains(action),
                "normal keys bar missing {:?} at width {}:\n{}",
                action,
                w,
                bar
            );
        }
    }
}

/// Drawer mode: all 6 drawer actions at every width from 60 to 140.
#[test]
fn test_drawer_footer_shows_every_action_at_every_width_60_to_140() {
    let mut app = create_test_app();
    app.fixture_exists = true;
    app.detail_drawer_open = true;
    for w in 60u16..=140 {
        let bar = bottom_rows(&app, w, 6);
        for (key, label) in DRAWER_KEYS {
            assert!(
                bar.contains(label) && bar.contains(key),
                "drawer keys bar missing [{}] {} at width {}:\n{}",
                key,
                label,
                w,
                bar
            );
        }
    }
}

/// Verdict mode: all 6 verdict actions at every width from 60 to 140.
#[test]
fn test_verdict_footer_shows_every_action_at_every_width_60_to_140() {
    let mut app = create_test_app();
    app.fixture_exists = true;
    app.waiting_for_human = Some("placement/bottom-right".to_string());
    for w in 60u16..=140 {
        let bar = bottom_rows(&app, w, 6);
        for (key, label) in VERDICT_KEYS {
            assert!(
                bar.contains(label) && bar.contains(key),
                "verdict keys bar missing [{}] {} at width {}:\n{}",
                key,
                label,
                w,
                bar
            );
        }
    }
}

/// State 1 is one row of unpadded `[k] Label` chips joined by exactly 3 spaces; State 2 is
/// 2 rows x 3 columns whose chips are all the same width with the key centered.
#[test]
fn test_key_grid_states_one_row_compact_or_two_rows_equal_width_chips() {
    for entries in [DRAWER_KEYS, VERDICT_KEYS] {
        for inner in 58usize..=138 {
            let lines = render_key_grid(entries, inner);
            let texts: Vec<String> = lines
                .iter()
                .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
                .collect();
            let joined = texts.join("\n");
            for (_, label) in entries {
                assert!(
                    joined.contains(label),
                    "{:?} missing at inner width {}:\n{}",
                    label,
                    inner,
                    joined
                );
            }
            match texts.len() {
                1 => {
                    let expected = entries
                        .iter()
                        .map(|(k, l)| format!("[{}] {}", k, l))
                        .collect::<Vec<_>>()
                        .join("   ");
                    assert_eq!(texts[0], format!(" {}", expected), "State 1 at {}", inner);
                }
                2 => {
                    let widest = entries.iter().map(|(k, _)| k.chars().count()).max().unwrap();
                    for t in &texts {
                        let chips: Vec<&str> = t
                            .split(['[', ']'])
                            .enumerate()
                            .filter(|(i, _)| i % 2 == 1)
                            .map(|(_, s)| s)
                            .collect();
                        assert!(
                            chips.iter().all(|c| c.chars().count() == widest),
                            "State 2 chips must all be {} wide at inner {}: {:?}",
                            widest,
                            inner,
                            t
                        );
                    }
                    assert_eq!(
                        texts[0].matches('[').count(),
                        3,
                        "State 2 row 1 must hold 3 chips at inner {}",
                        inner
                    );
                }
                n => panic!("key grid returned {} rows at inner width {}", n, inner),
            }
        }
    }
}
