// Records: a table whose cells are paragraphs, built as one block per row. A table with a long cell
// is unreadable as a table in a 45rem column (the cells get 25 characters each), so this
// integration rewrites such a <table> in the built HTML as
//
//   <ol class="records [records-stack]" role="list">
//     <li class="record">
//       <div class="record-key">first column</div>
//       <dl class="record-fields"><div class="field"><dt>header</dt><dd>cell</dd></div> ...</dl>
//     </li> ...
//   </ol>
//
// The layout is src/styles/records.css. The cells keep their inner HTML, so links, ids, inline code
// and the break points of code-breaks.mjs come along; the text of the page does not change.
//
// Which tables:
//   - on a page under project/ (the pages mirrored from docs/, not project/index.html), a table
//     whose longest body cell has AUTO_MIN characters of text or more;
//   - any table, on any page, that has a comment on the line directly above it:
//       <!-- site: records -->          records
//       <!-- site: table -->            a plain table, even where the rule above would convert it
//       <!-- site: records key=N -->    records with column N (1-based) as the key; default 1
//   GitHub shows the comment as nothing, so docs/ still renders as a normal table there.
// A directive that is not one of these, or a key outside the columns, fails the build; so does
// `cargo xtask docs-check-src`, which reads the same comments in the sources.
//
// Empty cells are left out. When the median key is longer than STACK_KEY characters, the key goes
// above the fields in the whole list (a gutter of 11rem cannot hold a sentence); the style also
// stacks below 40rem. The integration runs after Starlight, whose build step makes the Pagefind
// index from the tables as written, and before code-breaks.mjs, which then sees the records.
// Node built-ins only.
import { readdir, readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

// A table converts by itself when a body cell has this many characters of text or more. Measured
// on the 13 mirrored pages (86 tables): 240 gives records for 36 tables, 160 for 48 and 120 for 61,
// and below 240 the records were taller than the table for most of them.
export const AUTO_MIN = 240;

// A list whose median key is longer than this many characters stacks the key above the fields.
export const STACK_KEY = 48;

// A directive comment (the only comments that start with `site:`) followed by the table. The
// `(?!-->)` keeps a comment from reaching past its own `-->` to a later table.
const TABLE =
  /(<!--\s*site:(?:(?!-->)[\s\S])*-->\s*)?<table(?:\s[^>]*)?>([\s\S]*?)<\/table>/g;

const decode = (s) =>
  s
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#(?:39|x27);/g, "'")
    .replace(/&amp;/g, '&');

// Characters of text in a piece of cell HTML: tags (and so <wbr> and the code-break spans) add none.
const textLength = (html) => [...decode(html.replace(/<[^>]+>/g, ''))].length;

// { mode: 'records' | 'table', key } from the comment text; throws on anything else.
export function parseDirective(comment) {
  const m = /^<!--\s*site:\s*(.*?)\s*-->/s.exec(comment.trim());
  const words = m[1].split(/\s+/).filter(Boolean);
  const mode = words[0];
  if (mode !== 'records' && mode !== 'table') {
    throw new Error(`unknown directive \`site: ${m[1]}\` (use records, table or records key=N)`);
  }
  let key = 1;
  for (const w of words.slice(1)) {
    const k = /^key=(\d+)$/.exec(w);
    if (mode !== 'records' || !k || Number(k[1]) < 1) {
      throw new Error(`bad option \`${w}\` in \`site: ${m[1]}\` (only records takes key=N, N >= 1)`);
    }
    key = Number(k[1]);
  }
  return { mode, key };
}

function parseTable(inner) {
  const rows = [...inner.matchAll(/<tr>([\s\S]*?)<\/tr>/g)].map((r) =>
    [...r[1].matchAll(/<(th|td)(?:\s[^>]*)?>([\s\S]*?)<\/\1>/g)].map((c) => c[2].trim()),
  );
  return { header: rows[0] ?? [], body: rows.slice(1) };
}

const median = (nums) => {
  const s = [...nums].sort((a, b) => a - b);
  const mid = s.length >> 1;
  return s.length % 2 ? s[mid] : (s[mid - 1] + s[mid]) / 2;
};

export function recordsHtml({ header, body }, key) {
  const k = key - 1;
  const stack = median(body.map((r) => textLength(r[k] ?? ''))) > STACK_KEY;
  const out = [`<ol class="records${stack ? ' records-stack' : ''}" role="list">`];
  for (const row of body) {
    const cells = header.map((_, i) => row[i] ?? ''); // a short row has empty cells
    const fields = header
      .map((h, i) =>
        i === k || textLength(cells[i]) === 0
          ? ''
          : `<div class="field"><dt>${h || `Column ${i + 1}`}</dt><dd>${cells[i]}</dd></div>`,
      )
      .join('');
    out.push(
      `<li class="record"><div class="record-key">${cells[k]}</div>` +
        (fields ? `<dl class="record-fields">${fields}</dl>` : '') +
        '</li>',
    );
  }
  out.push('</ol>');
  return out.join('\n');
}

// Returns { html, converted, tables } for one built page. `project` is true for a page that is
// mirrored from docs/, where the automatic rule applies. Throws on a bad directive or key.
export function convertTables(html, { project = false } = {}) {
  let converted = 0;
  let tables = 0;
  const out = html.replace(TABLE, (all, comment, inner) => {
    tables++;
    const t = parseTable(inner);
    if (t.header.length === 0 || t.body.length === 0) return all;
    const directive = comment ? parseDirective(comment) : null;
    let key = 1;
    let records;
    if (directive) {
      if (directive.mode === 'table') return all;
      key = directive.key;
      records = true;
      if (t.header.length < 2 || key > t.header.length) {
        throw new Error(
          `\`site: records key=${key}\` on a table with ${t.header.length} column(s)`,
        );
      }
    } else {
      records =
        project &&
        t.header.length >= 2 &&
        Math.max(0, ...t.body.flat().map(textLength)) >= AUTO_MIN;
    }
    if (!records) return all;
    converted++;
    return recordsHtml(t, key);
  });
  return { html: out, converted, tables };
}

async function* htmlFiles(dir) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const file = path.join(dir, entry.name);
    if (entry.isDirectory()) yield* htmlFiles(file);
    else if (entry.name.endsWith('.html')) yield file;
  }
}

export default function records() {
  return {
    name: 'spawn-at-records',
    hooks: {
      'astro:build:done': async ({ dir, logger }) => {
        const root = fileURLToPath(dir);
        let pages = 0;
        let converted = 0;
        let tables = 0;
        for await (const file of htmlFiles(root)) {
          const rel = path.relative(root, file).split(path.sep).join('/');
          const before = await readFile(file, 'utf8');
          let result;
          try {
            result = convertTables(before, {
              project: rel.startsWith('project/') && rel !== 'project/index.html',
            });
          } catch (e) {
            throw new Error(`${rel}: ${e.message}`);
          }
          tables += result.tables;
          if (result.converted > 0) {
            await writeFile(file, result.html);
            pages++;
            converted += result.converted;
          }
        }
        logger.info(`${converted} of ${tables} tables became records, in ${pages} pages`);
      },
    },
  };
}
