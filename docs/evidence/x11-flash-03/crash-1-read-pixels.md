# Private Shell crash 1: `Clutter.Stage.read_pixels` (X11-FLASH-03a)

Date 2026-10-29 21:10 (local). Private headless Shell only (sandbox `/tmp/xflash-priv.*`, DING not
loaded, extensions `spawn-at@harsh.local` and `xflash-probe@local`). Live Shell pid 2718897 / start
81373971 unchanged before and after.

- Trigger: the probe's pixel-route test (`pixels: "test"`) called `global.stage.read_pixels(x, y, w, h)`
  at the first visible frame of a plain xterm window.
- Result: the private Shell aborted (SIGABRT from a GJS `Bail out!`). Last lines of its log:

```
Gjs:ERROR:./obj-x86_64-linux-gnu/../gi/arg.cpp:2134:bool gjs_array_from_fixed_size_array(JSContext*, JS::MutableHandleValue, GITypeInfo*, GITransfer, void*): assertion failed: (length != -1)
Bail out! Gjs:ERROR:./obj-x86_64-linux-gnu/../gi/arg.cpp:2134:bool gjs_array_from_fixed_size_array(JSContext*, JS::MutableHandleValue, GITypeInfo*, GITransfer, void*): assertion failed: (length != -1)
(EE) failed to read Wayland events: Broken pipe
```

- Cause (read from `Clutter-14.gir`): the typelib declares the return value of
  `clutter_stage_read_pixels` as `<array zero-terminated="0" c:type="guchar*">` with no length, so
  GJS cannot build a JS array from it and asserts. No Mutter window or focus assertion; no line
  mentions spawn-at.
- Decision (maintainer, in chat): drop this route; stop on any unexplained private crash, any Mutter
  window or focus assertion, or a live-Shell change; if another pixel route aborts the Shell, log it
  and drop it, and if two routes in a row abort it, stop and report.
- Count of consecutive pixel-route aborts after this one: 1.
