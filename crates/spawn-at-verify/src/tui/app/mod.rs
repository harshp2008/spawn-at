//! Interactive TUI application architecture (state, input, execution queue, clipboard).

pub mod clipboard;
pub mod input;
pub mod queue;
pub mod state;

pub use clipboard::system_copy_to_clipboard;
pub use state::{category_rank, ActivePane, FilterMode, ModalState, RunLoad, TuiApp};
