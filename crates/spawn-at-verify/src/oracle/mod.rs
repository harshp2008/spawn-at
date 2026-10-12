//! Mathematical placement oracle and golden vector cross-validation.

pub mod calculator;
pub mod check;

#[cfg(test)]
mod tests;

pub use calculator::{
    calculate_expected_rect, final_size_with_min, verify_rect_tolerance, OracleParams, Rect,
};
pub use check::{check_geometry, GeometryCheck, GeometryStatus, WorkareaProbe, WorkareaSource};
