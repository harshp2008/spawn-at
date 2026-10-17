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
