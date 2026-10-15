import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, chmodSync, mkdirSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

// install.sh against a stand-in curl and a stand-in spawn-at, without a terminal on stdin or
// stderr (so the "Install anyway?" question can never be asked here: the no-terminal rules are
// what these tests pin; the answers y / N / empty are covered by the scratch tty matrix).
// Each run gets its own PATH directory and a clean desktop environment on the child only.

const INSTALL_SH = fileURLToPath(new URL('../install.sh', import.meta.url));

function sandbox() {
    const dir = mkdtempSync(join(tmpdir(), 'spawn-at-install-test-'));
    const bin = join(dir, 'bin');
    const pkg = join(dir, 'pkg');
    mkdirSync(bin);
    mkdirSync(pkg);
    writeFileSync(join(pkg, 'spawn-at'),
        '#!/bin/sh\necho "FAKE spawn-at args: $* BACKEND=${SPAWN_AT_BACKEND:-unset}"\n');
    chmodSync(join(pkg, 'spawn-at'), 0o755);
    const tgz = join(dir, 'pkg.tgz');
    const tar = spawnSync('tar', ['-czf', tgz, '-C', pkg, './spawn-at']);
    assert.equal(tar.status, 0);
    // Every URL "downloads" the package and logs the call.
    writeFileSync(join(bin, 'curl'), `#!/bin/sh
out=""; hdr=""; url=""
while [ $# -gt 0 ]; do case "$1" in
  -o) out="$2"; shift 2;; -D) hdr="$2"; shift 2;;
  -w|-H|--connect-timeout|--max-time|--retry|--retry-delay|--speed-time|--speed-limit|-r) shift 2;;
  -*) shift;; *) url="$1"; shift;; esac; done
echo "$url" >> "${dir}/calls.log"
cp "${tgz}" "$out"; printf 'HTTP/2 200\\r\\n' > "$hdr"; printf '200 %s' "$url"
`);
    chmodSync(join(bin, 'curl'), 0o755);
    return { dir, bin };
}

function run({ desktop, session, backend }, args = []) {
    const { dir, bin } = sandbox();
    const env = { PATH: `${bin}:/usr/bin:/bin`, HOME: dir };
    if (desktop !== undefined) env.XDG_CURRENT_DESKTOP = desktop;
    if (session !== undefined) env.XDG_SESSION_TYPE = session;
    if (backend !== undefined) env.SPAWN_AT_BACKEND = backend;
    const r = spawnSync('bash', [INSTALL_SH, '--version', 'v1.2.3', ...args],
        { env, stdio: ['ignore', 'pipe', 'pipe'], encoding: 'utf8' });
    let calls = '';
    try { calls = readFileSync(join(dir, 'calls.log'), 'utf8'); } catch { /* none */ }
    const out = r.stdout.replace(/\x1b\[[0-9;]*m/g, '');
    return { status: r.status, out, err: r.stderr, calls };
}

const NOTICE = 'is not supported yet. spawn-at only fully supports GNOME on Wayland. ' +
    'An X11 fallback exists but may or may not work here.';

test('non-GNOME Wayland without a terminal: notice, exit 1, nothing downloaded', () => {
    for (const [desktop, session] of [['KDE', 'wayland'], ['sway', 'wayland'], [undefined, undefined]]) {
        const r = run({ desktop, session });
        assert.equal(r.status, 1, r.out + r.err);
        assert.ok(r.out.includes(`Your desktop (${desktop ?? 'unknown'}, ${session ?? 'unknown'}) ${NOTICE}`), r.out);
        assert.ok(r.out.includes('re-run with --allow-unsupported to install anyway'), r.out);
        assert.equal(r.calls, '', 'nothing may be downloaded before the answer');
        assert.ok(!r.out.includes('FAKE spawn-at'), r.out);
    }
});

test('--allow-unsupported installs, runs the installer with SPAWN_AT_BACKEND=x11 and does not forward the flag', () => {
    const r = run({ desktop: 'KDE', session: 'wayland' }, ['--allow-unsupported']);
    assert.equal(r.status, 0, r.out + r.err);
    assert.ok(r.out.includes(NOTICE), r.out);
    assert.ok(r.out.includes('FAKE spawn-at args: install BACKEND=x11'), r.out);
    assert.ok(!/args: .*allow-unsupported/.test(r.out), 'flag must not reach spawn-at');
    assert.ok(r.out.includes('Reminder: on this desktop spawn-at only runs with SPAWN_AT_BACKEND=x11'), r.out);
});

test('supported environments print no notice, ask nothing and leave the backend variable alone', () => {
    const cases = [
        { desktop: 'ubuntu:GNOME', session: 'wayland' },
        { desktop: 'gnome', session: undefined },
        { desktop: 'KDE', session: 'x11' },
        { desktop: 'KDE', session: 'wayland', backend: 'x11' },
        { desktop: 'KDE', session: 'wayland', backend: 'sway' },
    ];
    for (const c of cases) {
        for (const args of [[], ['--allow-unsupported']]) {
            const r = run(c, args);
            assert.equal(r.status, 0, JSON.stringify(c) + r.out + r.err);
            assert.ok(!r.out.includes('not supported yet') && !r.out.includes('Reminder'), r.out);
            assert.ok(r.out.includes(`FAKE spawn-at args: install BACKEND=${c.backend ?? 'unset'}`), r.out);
        }
    }
});
