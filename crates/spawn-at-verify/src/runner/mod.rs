//! Test execution engine, results aggregation, and validation checks.

pub mod engine;
pub mod executor;
pub mod models;
pub mod validation;

#[cfg(test)]
mod tests;

pub use engine::TestRunner;
pub use models::{
    resolve_final_status, AutoStatus, FinalStatus, HumanVerdict, HumanVerdictKind, RunOptions,
    TestResult, MAX_REPEAT_CAP,
};
pub use validation::{
    build_action_command_args, check_pattern_match, extract_claimed_window_id, pass_assessment,
    validate_expectations, validate_geometry,
};
