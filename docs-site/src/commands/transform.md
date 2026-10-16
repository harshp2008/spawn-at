# transform

```text
spawn-at transform [OPTIONS]
```

Moves, resizes, or both, a window that is already open. It takes a window selector (`--id`,
`--class`, `--title`, `--pid`, `--focused`) and the same placement options as
[`spawn`](spawn.md): `--area`, `--monitor`, `--anchor`, `--pivot`, `-p`/`-o`, `-s`, `--clamp`,
margins.

## Examples

From the README:

```bash
spawn-at transform --class org.gnome.Calculator --anchor center --size 400 500
```

From a test spec (`verify/README.md`, version 2 specs), acting on a window by the id the test
runner recorded:

```bash
spawn-at transform --id <ID> --size 600 450 --anchor center --margin 0 --no-focus
```

## What it does with partial options

- Neither `--anchor` nor `-p`/`-o`: nothing is moved. With `--size` alone the window keeps its
  top-left corner and is resized.
- `--pivot` without `--anchor` and without `-p`/`-o`: the window is resized around that point
  (`center` keeps the centre fixed, `bottom-right` keeps that corner fixed). Nothing else moves.
- With no `--monitor`, the target monitor is the one the window overlaps most; if it overlaps none,
  the primary monitor.
- `query windows` prints a `REL` position that `transform -p X Y` accepts back and turns into the
  same screen position, in all three `--area` modes.

## On GNOME

- The extension hides the window, resizes, waits for the app to answer, moves it and reveals it, as
  for `spawn`.
- On a **minimized** window, the usual glide from the dock is deliberately suppressed, so the window
  appears at the new geometry. On a window covered by others, normal focus transitions play.
- The `--focus`, `--no-focus` and `--defocus` options are read here (they control focus after the
  action). On `spawn` they are not.

## Not measured

Whether `transform` flashes. A test records an informational flash check for it on GNOME, but it has
not been run on a real GNOME session. See [Findings](../findings.md).

## Sources

- `README.md` ("What it looks like", "Placement defaults", CLI reference)
- `docs/behavior-notes.md`, sections 4, 5 and 6
- `verify/README.md`, section 2 ("Version 2 specs" example)
- `docs/TEST_COVERAGE_AUDIT.md`, sections 2, 4.3 and gap 14
