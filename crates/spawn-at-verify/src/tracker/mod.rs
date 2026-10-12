//! Window tracking and window leak cleanup prevention across test sessions.

pub mod state;

#[cfg(test)]
mod tests;

pub use state::{
    query_window_ids, query_window_records, query_window_rect, AttributionOutcome, WindowRecord,
    WindowTracker, MAX_CONCURRENT_WINDOWS,
};
