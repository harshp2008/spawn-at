//! Mathematical placement oracle and golden vector cross-validation.

pub mod calculator;

#[cfg(test)]
mod tests;

pub use calculator::{calculate_expected_rect, verify_rect_tolerance, OracleParams, Rect};
