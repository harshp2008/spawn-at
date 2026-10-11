//! Test loader and suite registry for spawn-at-verify.

pub mod filter;
pub mod registry;
pub mod tree;

#[cfg(test)]
mod tests;

pub use filter::{matches_glob, TestFilter};
pub use registry::TestRegistry;
pub use tree::format_test_tree;
