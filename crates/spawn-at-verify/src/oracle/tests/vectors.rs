//! Shared golden placement vector file cross-validation.

use crate::oracle::calculator::{
    calculate_expected_rect, verify_rect_tolerance, OracleParams, Rect,
};

/// Asserts that oracle placement matches shared golden test vectors.
#[test]
fn test_oracle_reads_shared_golden_placement_vectors() {
    use std::fs;
    use std::path::PathBuf;

    #[derive(serde::Deserialize)]
    struct GoldenFile {
        vectors: Vec<GoldenVector>,
    }

    #[derive(serde::Deserialize)]
    struct GoldenVector {
        name: String,
        workarea: RectJson,
        window_size: SizeJson,
        anchor: Option<String>,
        pivot: Option<String>,
        pos: Option<[i32; 2]>,
        offset: Option<[i32; 2]>,
        area: Option<String>,
        margin: i32,
        margin_top: Option<i32>,
        margin_bottom: Option<i32>,
        margin_left: Option<i32>,
        margin_right: Option<i32>,
        clamp: bool,
        expected: RectJson,
        #[allow(dead_code)]
        arithmetic: String,
    }

    #[derive(serde::Deserialize)]
    struct RectJson {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    }

    #[derive(serde::Deserialize)]
    struct SizeJson {
        width: u32,
        height: u32,
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let golden_path = manifest_dir.join("../../verify/golden/placement.json");
    let content = fs::read_to_string(&golden_path)
        .unwrap_or_else(|e| panic!("Failed to read golden vectors at {:?}: {}", golden_path, e));

    let parsed: GoldenFile = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse golden placement JSON: {}", e));

    for vec in &parsed.vectors {
        let wa = Rect::new(
            vec.workarea.x,
            vec.workarea.y,
            vec.workarea.width,
            vec.workarea.height,
        );

        let params = OracleParams {
            anchor: vec.anchor.clone(),
            pivot: vec.pivot.clone(),
            explicit_pos: vec.pos.map(|p| (p[0], p[1])),
            offset: vec.offset.map(|o| (o[0], o[1])),
            area: vec.area.clone(),
            cursor_pos: None,
            size: (vec.window_size.width, vec.window_size.height),
            margin: vec.margin,
            margin_top: vec.margin_top,
            margin_bottom: vec.margin_bottom,
            margin_left: vec.margin_left,
            margin_right: vec.margin_right,
            clamp: vec.clamp,
        };

        let calculated = calculate_expected_rect(wa, &params)
            .unwrap_or_else(|e| panic!("Failed calculating vector '{}': {}", vec.name, e));

        let expected = Rect::new(
            vec.expected.x,
            vec.expected.y,
            vec.expected.width,
            vec.expected.height,
        );

        // Assert exact 0-pixel tolerance for mathematical placement vectors
        verify_rect_tolerance(calculated, expected, 0)
            .unwrap_or_else(|e| panic!("Vector '{}' failed at tolerance 0: {}", vec.name, e));
    }
}
