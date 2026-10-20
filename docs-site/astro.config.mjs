// Config of the documentation site. Served from https://harshp2008.github.io/spawn-at/ .
// The site makes no third-party request: system fonts, no analytics, no CDN, and the search is
// Pagefind, which runs from files in the build output.
import { readFileSync } from 'node:fs';
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import codeBreaks from './src/integrations/code-breaks.mjs';
import records from './src/integrations/records.mjs';

// Inline script for the <head> of every page: the button that hides the "On this page" column.
// It is inline so the saved choice is applied before the first paint.
const tocToggle = readFileSync(new URL('./src/scripts/toc-toggle.js', import.meta.url), 'utf8');

export default defineConfig({
  site: 'https://harshp2008.github.io',
  base: '/spawn-at/',
  // Smart punctuation would turn `--id` in prose into an en dash and `...` into an ellipsis.
  // `cargo xtask docs-check-html` fails the build if a flag-like dash pair is rewritten.
  markdown: { smartypants: false },
  integrations: [
    starlight({
      title: 'spawn-at',
      description: 'Place windows at an exact position and size on GNOME Wayland.',
      social: [
        { icon: 'github', label: 'GitHub', href: 'https://github.com/harshp2008/spawn-at' },
      ],
      customCss: [
        './src/styles/custom.css',
        './src/styles/tables.css',
        './src/styles/records.css',
        './src/styles/scroll-hints.css',
        './src/styles/toc-toggle.css',
      ],
      head: [{ tag: 'script', content: tocToggle }],
      // The order follows the reader's path: the introduction, then installing and using it (with
      // the commands), where it works, the reference, the findings and experiments, the documents
      // of the project, and the pages for people who work on it last. Every group is `collapsed`:
      // Starlight opens only the group that holds the current page. The CLI reference group is
      // filled from the folder `reference`, so a new command needs no entry here.
      sidebar: [
        { slug: 'index', label: 'Introduction' },
        {
          label: 'Using spawn-at',
          collapsed: true,
          items: [
            'install',
            'requirements',
            'update',
            'installer-flags',
            'files-and-paths',
            'quick-start',
            'concepts',
            {
              label: 'Commands',
              collapsed: true,
              items: [
                { slug: 'commands', label: 'Overview' },
                'commands/spawn',
                'commands/transform',
                'commands/window-state',
                'commands/query',
                'commands/manage',
              ],
            },
            'configuration',
            'environment',
            'troubleshooting',
          ],
        },
        {
          label: 'Where it works',
          collapsed: true,
          items: [
            {
              label: 'Environments',
              collapsed: true,
              items: [
                { slug: 'environments', label: 'Overview' },
                'environments/gnome-wayland',
                { slug: 'environments/sway', label: 'Sway (experimental)' },
                'environments/x11-xwayland',
                'environments/unsupported-desktops',
              ],
            },
            'shortcomings',
          ],
        },
        {
          label: 'Reference',
          collapsed: true,
          items: [
            {
              label: 'CLI reference',
              collapsed: true,
              items: [{ autogenerate: { directory: 'reference' } }],
            },
            // The page is `rustdoc`, not `api`: the rustdoc output is copied to /api/, so a page
            // at /api/ would collide with it.
            { slug: 'rustdoc', label: 'API documentation (rustdoc)' },
            'known-limits',
          ],
        },
        {
          label: 'Findings and experiments',
          collapsed: true,
          items: [
            'findings',
            { slug: 'experiments', label: 'Index' },
            'experiments/6-10-placement-resize-specs',
            'experiments/6-11-late-resize-fix-live',
            'experiments/6-11-1-final-state-oracle',
            'experiments/6-12-why-revealed-at-pre-resize-size',
            'experiments/6-13-cloak-starves-frame-callbacks',
            'experiments/6-14-mutter-api-and-arrival-wait',
            'experiments/6-15-flash-aware-verdict',
            'experiments/6-16-flash-numeric-headline',
            'experiments/6-17-flash-baseline',
            'experiments/6-18-ready-wait-live-validation',
            'experiments/6-19-ready-wait-abc',
            'experiments/6-20-why-b-and-c-still-flash',
            'experiments/6-21-default-cap-250-wire-paint-time',
            'experiments/6-22-hostile-apps',
            'experiments/6-23-post-reinstall-live-check',
            'experiments/6-24-texture-reader',
            'experiments/6-25a-window-ping',
            'experiments/6-25b-capture-rig',
            'experiments/6-25c-x11-hold-damage-built',
            'experiments/6-26-x11-hold-damage-live',
            'experiments/6-27-premap-and-pixel-probe-built',
            'experiments/6-28-premap-and-pixel-probe-live',
            'experiments/6-29-pixel-probe-retry-shell-crash',
            'experiments/6-31-sync-counter-feasibility',
            'experiments/6-32-gtk4-client-legacy-rule',
            'experiments/6-33-sync-counter-moves-on-repaint',
            'experiments/6-34-self-sent-sync-request',
            'experiments/6-35-ld-preload-geometry-shim',
            'experiments/p8-sway-flicker-not-run',
          ],
        },
        // Written from docs/ at build time by `cargo xtask docs-mirror`; the order comes from the
        // `sidebar.order` of each page.
        {
          label: 'Project documents',
          collapsed: true,
          items: [{ autogenerate: { directory: 'project' } }],
        },
        {
          label: 'Developing',
          collapsed: true,
          items: [
            'testing',
            'contributing/verify-tui',
            'contributing/test-specs',
            'contributing',
            'contributing/writing-docs',
            'changelog',
          ],
        },
      ],
    }),
    // After Starlight on purpose: its build-done step makes the Pagefind index, and these then
    // rewrite the pages, so the search index is made from the text as written. Records come
    // before code-breaks, which then adds its break points inside the records as well.
    records(),
    codeBreaks(),
  ],
});
