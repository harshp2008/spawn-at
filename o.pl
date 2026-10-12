undef $/; $_=<STDIN>;
s|(        Some\("no-trace"\),\n    \),\n)|$1    (\n        "real_placement_top_left_run.json",\n        include_str!("../../tests/fixtures/real_placement_top_left_run.json"),\n        None,\n    ),\n    (\n        "real_placement_center_200x150_fail_run.json",\n        include_str!("../../tests/fixtures/real_placement_center_200x150_fail_run.json"),\n        None,\n    ),\n|;
print;
