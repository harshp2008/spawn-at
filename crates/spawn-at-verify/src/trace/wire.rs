//! Paint times read from the fixture's Wayland protocol dump.
//!
//! The fixture's `FIRST_PAINT` stamp is written by the fixture itself after `buffer.present()`
//! returns, so a fixture thread that is not scheduled right after the commit stamps late (TEST_PLAN
//! 6.20: 14 to 23 ms behind the commit under load). With `SPAWN_AT_FIXTURE_TRACE_DIR` the fixture
//! also runs with `WAYLAND_DEBUG=client`, and libwayland prints every request it marshals with its
//! own timestamp. The client's commit of a buffer is the moment the content can reach the
//! compositor, so this module takes the paint time from there.
//!
//! The dump prefixes a line with `[<ms>.<us>]`, which is `CLOCK_REALTIME` in microseconds modulo
//! 2^32 split at the thousands. The fixture's first trace line, `<mono_ns> FIXTURE TRACE_START
//! realtime_ns=<ns>`, gives one reading of both clocks, which turns every dump time into
//! `CLOCK_MONOTONIC` nanoseconds, the clock the flash check already compares in.
//!
//! Limits: libwayland stamps a request when it is marshalled, before the connection is flushed,
//! so the stamp is the earliest time the compositor could have the commit; a thread that is
//! descheduled between marshalling and flush delivers later than the stamp says. Realtime and
//! monotonic clocks can differ in rate by NTP slew (parts per million), which over the few
//! seconds of a run is microseconds.

use std::collections::HashMap;

/// One commit that put a buffer of a new size on the window's surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WirePaint {
    /// `CLOCK_MONOTONIC` nanoseconds at which libwayland marshalled the `wl_surface.commit`.
    pub t_ns: u64,
    /// Width of the committed buffer in pixels.
    pub w: u32,
    /// Height of the committed buffer in pixels.
    pub h: u32,
}

/// The dump's `[<ms>.<us>]` prefix as microseconds modulo 2^32, or `None` when the line has none.
fn dump_us(line: &str) -> Option<u64> {
    let rest = line.strip_prefix('[')?;
    let (stamp, _) = rest.split_once(']')?;
    let (ms, us) = stamp.trim().split_once('.')?;
    Some(ms.parse::<u64>().ok()? * 1000 + us.parse::<u64>().ok()?)
}

/// The number in `name@<id>` at the start of `s`, and the rest of `s` after the id.
fn object_id<'a>(s: &'a str, name: &str) -> Option<(u32, &'a str)> {
    let rest = s.strip_prefix(name)?.strip_prefix('@')?;
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    Some((rest[..end].parse().ok()?, &rest[end..]))
}

/// Arguments of the call in `s` (`.name(<args>)`), split at `", "`.
fn call_args<'a>(s: &'a str, name: &str) -> Option<Vec<&'a str>> {
    let inner = s.strip_prefix('.')?.strip_prefix(name)?.strip_prefix('(')?;
    let inner = inner.strip_suffix(')')?;
    Some(if inner.is_empty() {
        Vec::new()
    } else {
        inner.split(", ").collect()
    })
}

/// Reads the first commit at each new buffer size from the text of a fixture trace file
/// (`fixture-<pid>.log`: the `TRACE_START` line, the fixture's own lines and the protocol dump).
///
/// A paint is a `wl_surface.commit` that follows a `wl_surface.attach` of a buffer whose size is
/// known from `wl_shm_pool.create_buffer`; it is listed when that size differs from the one the
/// same surface committed before (the first commit with a buffer always is). A commit without an
/// attach shows the buffer already there and is not a paint. Empty when the text has no
/// `TRACE_START` line, because then the dump's clock cannot be related to the monotonic one.
pub fn parse_wire_paints(text: &str) -> Vec<WirePaint> {
    let mut origin: Option<(u64, u64)> = None;
    for line in text.lines() {
        let mut it = line.split_whitespace();
        if let (Some(mono), Some("FIXTURE"), Some("TRACE_START"), Some(real)) =
            (it.next(), it.next(), it.next(), it.next())
        {
            let real_ns = real
                .strip_prefix("realtime_ns=")
                .and_then(|v| v.parse::<u64>().ok());
            if let (Ok(mono), Some(real_ns)) = (mono.parse::<u64>(), real_ns) {
                origin = Some((mono, (real_ns / 1000) % (1u64 << 32)));
                break;
            }
        }
    }
    let Some((mono0, real_mod_us)) = origin else {
        return Vec::new();
    };

    let mut sizes: HashMap<u32, (u32, u32)> = HashMap::new();
    let mut pending: HashMap<u32, (u32, u32)> = HashMap::new();
    let mut shown: HashMap<u32, (u32, u32)> = HashMap::new();
    let mut out = Vec::new();
    for line in text.lines() {
        let Some(us) = dump_us(line) else { continue };
        let Some(body) = line.split_once("-> ").map(|(_, b)| b) else {
            continue;
        };
        if let Some((_, rest)) = object_id(body, "wl_shm_pool") {
            if let Some(args) = call_args(rest, "create_buffer") {
                // create_buffer(new id wl_buffer@N, offset, width, height, stride, format)
                let id = args
                    .first()
                    .and_then(|a| a.strip_prefix("new id "))
                    .and_then(|a| object_id(a, "wl_buffer"));
                let dims = (
                    args.get(2).and_then(|v| v.parse::<u32>().ok()),
                    args.get(3).and_then(|v| v.parse::<u32>().ok()),
                );
                if let (Some((id, _)), (Some(w), Some(h))) = (id, dims) {
                    sizes.insert(id, (w, h));
                }
            }
        } else if let Some((surface, rest)) = object_id(body, "wl_surface") {
            if let Some(args) = call_args(rest, "attach") {
                match args.first().and_then(|a| object_id(a, "wl_buffer")) {
                    Some((id, _)) => match sizes.get(&id) {
                        Some(sz) => {
                            pending.insert(surface, *sz);
                        }
                        None => {
                            pending.remove(&surface);
                        }
                    },
                    None => {
                        pending.remove(&surface);
                    }
                }
            } else if call_args(rest, "commit").is_some() {
                let Some(sz) = pending.remove(&surface) else {
                    continue;
                };
                if shown.insert(surface, sz) != Some(sz) {
                    // Signed distance to the TRACE_START reading, wrapped into +-2^31 us so a
                    // dump clock that rolled over inside the run still lands in order.
                    let span = 1i64 << 32;
                    let delta =
                        (us as i64 - real_mod_us as i64 + span / 2).rem_euclid(span) - span / 2;
                    out.push(WirePaint {
                        t_ns: (mono0 as i64 + delta * 1000) as u64,
                        w: sz.0,
                        h: sz.1,
                    });
                }
            }
        }
    }
    out.sort_by_key(|p| p.t_ns);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `TRACE_START` reading of monotonic 1_000_000_000 ns against realtime 1_000_000_000_000 ns
    /// (1000 s), so the dump stamp `[1000000.000]` is realtime 1000 s and monotonic 1_000_000_000.
    const START: &str = "1000000000 FIXTURE TRACE_START realtime_ns=1000000000000\n";

    /// The two commits of a window that painted 400x300 and then 800x600: both times are the
    /// dump's stamp shifted onto the monotonic clock (`[1000000.250]` is 250 us after the reading,
    /// monotonic 1_000_250_000 ns), and the attach-less commit between them is not a paint.
    #[test]
    fn test_a_commit_per_new_size_with_its_monotonic_time() {
        let text = format!(
            "{START}\
             [1000000.100]  -> wl_shm_pool@10.create_buffer(new id wl_buffer@11, 0, 400, 300, 1600, 1)\n\
             [1000000.200]  -> wl_shm_pool@10.create_buffer(new id wl_buffer@12, 0, 800, 600, 3200, 1)\n\
             [1000000.250]  -> wl_surface@5.attach(wl_buffer@11, 0, 0)\n\
             [1000000.260]  -> wl_surface@5.commit()\n\
             [1000001.000]  -> wl_surface@5.commit()\n\
             [1000003.500]  -> wl_surface@5.attach(wl_buffer@12, 0, 0)\n\
             [1000003.750]  -> wl_surface@5.commit()\n"
        );
        assert_eq!(
            parse_wire_paints(&text),
            vec![
                WirePaint {
                    t_ns: 1_000_260_000,
                    w: 400,
                    h: 300
                },
                WirePaint {
                    t_ns: 1_003_750_000,
                    w: 800,
                    h: 600
                },
            ]
        );
    }

    /// Committing a second buffer of the size already shown is not a new paint, and a commit whose
    /// attach named a buffer the dump never created (or `nil`) is ignored instead of guessed.
    #[test]
    fn test_same_size_unknown_buffer_and_null_attach_are_not_paints() {
        let text = format!(
            "{START}\
             [1000000.100]  -> wl_shm_pool@10.create_buffer(new id wl_buffer@11, 0, 400, 300, 1600, 1)\n\
             [1000000.110]  -> wl_shm_pool@10.create_buffer(new id wl_buffer@12, 0, 400, 300, 1600, 1)\n\
             [1000000.200]  -> wl_surface@5.attach(wl_buffer@11, 0, 0)\n\
             [1000000.210]  -> wl_surface@5.commit()\n\
             [1000000.300]  -> wl_surface@5.attach(wl_buffer@12, 0, 0)\n\
             [1000000.310]  -> wl_surface@5.commit()\n\
             [1000000.400]  -> wl_surface@5.attach(wl_buffer@99, 0, 0)\n\
             [1000000.410]  -> wl_surface@5.commit()\n\
             [1000000.500]  -> wl_surface@5.attach(nil, 0, 0)\n\
             [1000000.510]  -> wl_surface@5.commit()\n"
        );
        let paints = parse_wire_paints(&text);
        assert_eq!(
            paints,
            vec![WirePaint {
                t_ns: 1_000_210_000,
                w: 400,
                h: 300
            }]
        );
    }

    /// Two surfaces keep their own last size, and events the client received (no `-> `) are never
    /// read as requests.
    #[test]
    fn test_surfaces_are_tracked_apart_and_events_are_skipped() {
        let text = format!(
            "{START}\
             [1000000.100]  -> wl_shm_pool@10.create_buffer(new id wl_buffer@11, 0, 400, 300, 1600, 1)\n\
             [1000000.200]  -> wl_surface@5.attach(wl_buffer@11, 0, 0)\n\
             [1000000.210]  -> wl_surface@5.commit()\n\
             [1000000.300]  -> wl_surface@6.attach(wl_buffer@11, 0, 0)\n\
             [1000000.310]  -> wl_surface@6.commit()\n\
             [1000000.400] wl_surface@5.commit()\n"
        );
        let paints = parse_wire_paints(&text);
        assert_eq!(paints.len(), 2);
        assert_eq!(paints[1].t_ns, 1_000_310_000);
    }

    /// The dump clock is 32 bits of microseconds: with the reading 100 us before a rollover, a
    /// stamp 154 us after the rollover is 254 us after the reading, not 2^32 us before it.
    #[test]
    fn test_a_dump_clock_rollover_inside_the_run_stays_in_order() {
        // reading at 4_294_967_196 us (100 us before the rollover); the stamps below are after it
        let text = "1000000000 FIXTURE TRACE_START realtime_ns=4294967196000\n\
                    [4294967.196]  -> wl_shm_pool@10.create_buffer(new id wl_buffer@11, 0, 400, 300, 1600, 1)\n\
                    [0.054]  -> wl_surface@5.attach(wl_buffer@11, 0, 0)\n\
                    [0.154]  -> wl_surface@5.commit()\n";
        assert_eq!(
            parse_wire_paints(text),
            vec![WirePaint {
                t_ns: 1_000_254_000,
                w: 400,
                h: 300
            }]
        );
    }

    /// Without the `TRACE_START` line the dump cannot be put on the monotonic clock: nothing is
    /// returned rather than times on an unknown origin.
    #[test]
    fn test_no_trace_start_means_no_paints() {
        let text = "[1000000.100]  -> wl_surface@5.commit()\n";
        assert!(parse_wire_paints(text).is_empty());
    }
}
