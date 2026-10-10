# Verify Fixture Traces

Drop real captured session/journal logs into this directory as `real_trace.log`.

`tests/trace_analyser_test.rs::test_real_trace_drop_in_fixture` will automatically detect and validate `real_trace.log` when present.
