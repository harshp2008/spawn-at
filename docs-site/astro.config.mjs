// Config of the documentation site. Served from https://harshp2008.github.io/spawn-at/ .
// The site makes no third-party request: system fonts, no analytics, no CDN, and the search is
// Pagefind, which runs from files in the build output.
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

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
      customCss: ['./src/styles/custom.css'],
      // Same order and groups as the mdBook table of contents this replaces. The CLI reference
      // group is filled from the folder `reference`, so a new command needs no entry here.
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
            'findings',
          ],
        },
        {
          label: 'Experiments',
          collapsed: true,
          items: [
            { slug: 'experiments', label: 'Index' },
            'experiments/6-10-placement-resize-specs',
            'experiments/6-11-late-resize-fix-live',
            'experiments/6-11-1-final-state-oracle',
            'experiments/6-12-why-revealed-at-pre-resize-size',
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
          label: 'Developing',
          collapsed: true,
          items: ['testing', 'contributing', 'changelog'],
        },
      ],
    }),
  ],
});
