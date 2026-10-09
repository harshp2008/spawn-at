/**
 * Node.js ESM custom loader to stub `gi://` and `resource:///` imports for unit/integration testing.
 */

export async function resolve(specifier, context, nextResolve) {
    if (specifier.startsWith('gi://') || specifier.startsWith('resource:///')) {
        let mockSource = '';

        if (specifier === 'resource:///org/gnome/shell/extensions/extension.js') {
            mockSource = `
                export class Extension {
                    constructor(metadata) {
                        this.metadata = metadata;
                    }
                    enable() {}
                    disable() {}
                }
            `;
        } else if (specifier === 'gi://Meta') {
            mockSource = `
                export default {
                    WindowClientType: { WAYLAND: 0, X11: 1 },
                    WindowType: { NORMAL: 0 },
                    MaximizeFlags: { BOTH: 3 },
                    is_wayland_compositor: () => false,
                };
            `;
        } else if (specifier === 'gi://GLib') {
            mockSource = `
                export default {
                    PRIORITY_DEFAULT: 0,
                    PRIORITY_DEFAULT_IDLE: 100,
                    SOURCE_REMOVE: false,
                    SOURCE_CONTINUE: true,
                    get_monotonic_time: () => Date.now() * 1000,
                    get_user_cache_dir: () => '/tmp',
                    get_user_state_dir: () => '/tmp',
                    get_user_runtime_dir: () => '/tmp',
                    getenv: () => null,
                    Bytes: class { constructor() {} },
                    timeout_add: (_p, ms, cb) => setTimeout(cb, Math.min(ms, 5)),
                    source_remove: (id) => clearTimeout(id),
                    idle_add: (_p, cb) => { setImmediate(cb); return 1; },
                    build_filenamev: (parts) => parts.join('/'),
                    file_get_contents: () => [false, new Uint8Array()],
                    mkdir_with_parents: () => 0,
                    Source: {
                        remove: (id) => clearTimeout(id),
                    },
                };
            `;
        } else if (specifier === 'gi://Gio') {
            mockSource = `
                export default {
                    DBus: {
                        session: {},
                    },
                    DBusExportedObject: {
                        wrapJSObject: () => ({
                            export: () => {},
                            unexport: () => {},
                            emit_signal: () => {},
                        }),
                    },
                    DBusNodeInfo: {
                        new_for_xml: () => ({ interfaces: [{ name: 'org.gnome.Shell.Extensions.SpawnAt' }] }),
                    },
                    File: {
                        new_for_path: (path) => ({
                            get_path: () => path,
                            query_exists: () => false,
                            query_info: () => ({ get_size: () => 0 }),
                            make_directory_with_parents: () => {},
                            append_to: () => ({ write_all: () => {}, close: () => {} }),
                            replace: () => ({ write_all: () => {}, close: () => {} }),
                            delete: () => {},
                        }),
                    },
                };
            `;
        } else if (specifier === 'gi://Clutter') {
            mockSource = `
                export default {
                    Clone: class {},
                    Actor: class {
                        constructor(props = {}) {
                            Object.assign(this, props);
                        }
                    },
                };
            `;
        } else {
            mockSource = `export default {};`;
        }

        return {
            shortCircuit: true,
            url: `data:text/javascript,${encodeURIComponent(mockSource)}`,
        };
    }

    return nextResolve(specifier, context);
}
