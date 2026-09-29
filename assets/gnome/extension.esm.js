/**
 * Spawn-At GNOME Shell Extension
 *
 * ARCHITECTURAL OVERVIEW & THE "WHY" BEHIND OPACITY CLOAKING ON WAYLAND:
 *
 * 1. The Wayland Security & Isolation Model:
 *    Under Wayland, unlike X11, client applications are explicitly barred from
 *    querying global display coordinates or setting their own absolute window
 *    positions. The compositor (Mutter in GNOME) maintains exclusive ownership
 *    of window placement. As a result, external CLI tools cannot move or position
 *    Wayland windows via standard protocols.
 *
 * 2. Why Traditional Post-Launch Repositioning Fails (The "Flash-and-Jump" Bug):
 *    In naive window positioning approaches, a CLI launches the application and
 *    polls window lists until the window appears, then sends a move/resize request.
 *    By the time the move request reaches Mutter, the window has already been:
 *      (a) Mapped into the Clutter scene graph,
 *      (b) Rendered at Mutter's default placement (often centered or cascading),
 *      (c) Flushed to the display hardware (KMS / DRM plane).
 *    When the resize/move takes effect several frames later, the user perceives an
 *    annoying, jarring flicker or visual jump.
 *
 * 3. The Zero-Flicker "Opacity Cloaking" Solution:
 *    This extension hooks directly into GNOME Shell's compositor internals via two
 *    critical events:
 *      - `window-created`: Triggered immediately when Mutter allocates a `MetaWindow`
 *        for a new Wayland toplevel surface, before any scene graph actor is mapped.
 *        We inspect the window's `wm_class` / `app_id` and match it with pending
 *        spawn targets registered via our D-Bus `Arm` method.
 *      - `map` (on `global.window_manager`): Triggered when Mutter constructs the
 *        `MetaWindowActor` (a ClutterActor) to begin displaying the window.
 *        Synchronously, within the very first execution turn of `map`, we set:
 *          `actor.opacity = 0`
 *        This ensures Mutter renders the window as completely transparent during the
 *        initial Wayland client `xdg_surface.configure` handshake.
 *
 * 4. Compositor Thread Timing & Frame Rect Verification:
 *    When `window.move_resize_frame()` is invoked, Wayland clients process the
 *    configure event asynchronously. The compositor might take 1 to 3 frame cycles
 *    (16ms-50ms) to negotiate buffer dimensions with the client.
 *    We periodically poll `window.get_frame_rect()`. Only when the actual rendered
 *    frame rectangle satisfies the target coordinates (or when dimension clamping is
 *    detected or a safe timeout threshold expires), we restore `actor.opacity = 255`.
 *    Result: The window appears instantly and atomically at the exact target location!
 */

import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';
import Gio from 'gi://Gio';
import Meta from 'gi://Meta';
import GLib from 'gi://GLib';

const DBUS_IFACE = `
<node>
  <interface name="org.gnome.Shell.Extensions.SpawnAt">
    <method name="Arm">
      <arg type="s" name="identifier" direction="in"/>
      <arg type="i" name="x" direction="in"/>
      <arg type="i" name="y" direction="in"/>
      <arg type="i" name="w" direction="in"/>
      <arg type="i" name="h" direction="in"/>
    </method>
    <method name="GetCursor">
      <arg type="i" name="x" direction="out"/>
      <arg type="i" name="y" direction="out"/>
    </method>
  </interface>
</node>`;

export default class SpawnAtExtension extends Extension {
    enable() {
        // Map of armed App IDs / WM_CLASS identifiers -> target geometries
        this._armedSpawns = new Map();

        // Wildcard target: used when armed with "*" to catch the next unmapped window
        this._wildcardTarget = null;
        this._wildcardTimeoutId = null;

        // 1. Hook into window creation
        // This is called synchronously by Mutter when a new MetaWindow is instantiated.
        this._windowCreatedId = global.display.connect('window-created', (display, window) => {
            this._handleWindowCreated(window);
        });

        // 2. Hook into compositor actor mapping
        // This is called when Mutter wraps the MetaWindow in a ClutterActor for display.
        this._mapId = global.window_manager.connect('map', (wm, actor) => {
            this._handleActorMap(actor);
        });

        // 3. Export D-Bus interface for the Rust CLI client
        this._dbusImpl = Gio.DBusExportedObject.wrapJSObject(DBUS_IFACE, this);
        this._dbusImpl.export(Gio.DBus.session, '/org/gnome/Shell/Extensions/SpawnAt');
    }

    /**
     * Inspects newly created windows to attach armed placement targets.
     */
    _handleWindowCreated(window) {
        const checkMatch = (id) => {
            let target = null;

            // Check exact App ID / WM_CLASS match
            if (id && this._armedSpawns.has(id)) {
                target = this._armedSpawns.get(id);
                this._armedSpawns.delete(id);
            } 
            // Fallback: Wildcard match catches the very next window within timeout
            else if (this._wildcardTarget) {
                target = this._wildcardTarget;
                this._clearWildcard();
            }

            if (target) {
                // Attach target geometry directly onto the MetaWindow instance
                window._spawnAtTarget = target;
            }
        };

        // Under Wayland, wm_class or gtk_application_id can be populated asynchronously
        // as the client completes initial surface negotiation.
        let cls = window.get_wm_class();
        if (!cls) {
            // Listen for late wm-class property notification
            let sigId = window.connect('notify::wm-class', () => {
                let lateCls = window.get_wm_class();
                if (lateCls) {
                    checkMatch(lateCls);
                    window.disconnect(sigId);
                }
            });

            // Also check wildcard immediately if armed
            if (this._wildcardTarget) {
                checkMatch(null);
            }
        } else {
            checkMatch(cls);
        }
    }

    /**
     * Intercepts window actor mapping to execute zero-flicker Opacity Cloaking.
     */
    _handleActorMap(actor) {
        let window = actor.meta_window;
        if (!window || !window._spawnAtTarget) {
            return;
        }

        const target = window._spawnAtTarget;
        delete window._spawnAtTarget;

        // STEP 1: OPACITY CLOAKING
        // Suppress actor visibility immediately in the map callback before the compositor
        // submits the frame to the GPU render pass.
        actor.opacity = 0;

        // STEP 2: Clear any maximized state that would restrict repositioning
        window.unmaximize(Meta.MaximizeFlags.BOTH);

        // STEP 3: Apply initial geometry frame mutation
        if (target.w > 0 && target.h > 0) {
            window.move_resize_frame(true, target.x, target.y, target.w, target.h);
        } else {
            window.move_frame(true, target.x, target.y);
        }

        // STEP 4: Asynchronous settling loop
        // Wayland clients configure surface buffers asynchronously. We poll the frame
        // rectangle every 10ms to verify that the geometry matches the target before
        // making the window visible.
        let checkCount = 0;
        const maxChecks = 35; // Maximum duration: ~350ms safety clamp

        GLib.timeout_add(GLib.PRIORITY_DEFAULT, 10, () => {
            checkCount++;

            // Ensure opacity stays cloaked while Mutter adjusts the geometry
            actor.opacity = 0;

            let frame = window.get_frame_rect();

            // Re-apply target position if Mutter drifted during client configure events
            if (frame.x !== target.x || frame.y !== target.y) {
                if (target.w > 0 && target.h > 0) {
                    window.move_resize_frame(true, target.x, target.y, target.w, target.h);
                } else {
                    window.move_frame(true, target.x, target.y);
                }
            }

            let posMatch = (frame.x === target.x && frame.y === target.y);
            let sizeMatch = (target.w === 0 && target.h === 0) || 
                            (frame.width === target.w && frame.height === target.h);

            // Some clients enforce min/max size geometry hints (e.g. terminals with grid increments).
            // If the position matches and we've waited at least 5 frames (~50ms), accept as clamped match.
            let clampedMatch = posMatch && (checkCount >= 5);

            // Timeout fallback: never leave a window permanently hidden if client misbehaves
            let timedOut = (checkCount >= maxChecks);

            if ((posMatch && sizeMatch) || clampedMatch || timedOut) {
                // Reveal the window atomically at its settled position
                actor.opacity = 255;
                return GLib.SOURCE_REMOVE;
            }

            return GLib.SOURCE_CONTINUE;
        });
    }

    /**
     * D-Bus Method: Arm(identifier, x, y, w, h)
     * Primes the compositor to intercept the specified application window.
     */
    Arm(identifier, x, y, w, h) {
        let rect = { x, y, w, h };

        if (!identifier || identifier === "*") {
            // Wildcard: intercept the very next unmapped window within 1200ms
            this._clearWildcard();
            this._wildcardTarget = rect;

            this._wildcardTimeoutId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 1200, () => {
                this._wildcardTarget = null;
                this._wildcardTimeoutId = null;
                return GLib.SOURCE_REMOVE;
            });
        } else {
            this._armedSpawns.set(identifier, rect);

            // Auto-clean stale armed spawns after 15 seconds if application never launched
            GLib.timeout_add(GLib.PRIORITY_DEFAULT, 15000, () => {
                if (this._armedSpawns.has(identifier)) {
                    this._armedSpawns.delete(identifier);
                }
                return GLib.SOURCE_REMOVE;
            });
        }
    }

    /**
     * D-Bus Method: GetCursor() -> (x, y)
     * Queries the pointer's current global screen coordinates.
     */
    GetCursor() {
        let [x, y] = global.get_pointer();
        return [x, y];
    }

    _clearWildcard() {
        if (this._wildcardTimeoutId) {
            GLib.Source.remove(this._wildcardTimeoutId);
            this._wildcardTimeoutId = null;
        }
        this._wildcardTarget = null;
    }

    disable() {
        if (this._windowCreatedId) {
            global.display.disconnect(this._windowCreatedId);
            this._windowCreatedId = null;
        }

        if (this._mapId) {
            global.window_manager.disconnect(this._mapId);
            this._mapId = null;
        }

        this._clearWildcard();

        if (this._dbusImpl) {
            this._dbusImpl.unexport();
            this._dbusImpl = null;
        }

        this._armedSpawns.clear();
    }
}
