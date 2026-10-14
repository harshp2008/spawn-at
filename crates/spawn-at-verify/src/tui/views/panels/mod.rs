//! Main dashboard panel widgets and calculations.

pub mod description;
pub mod header;
pub mod result_box;
pub mod tests_list;

pub use description::{count_wrapped_lines, draw_details_pane};
pub use header::draw_header;
pub use result_box::{compute_result_box_height, draw_result_box, get_result_content};
pub use tests_list::{compute_list_items_count, draw_test_list};
