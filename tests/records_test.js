import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import os from 'node:os';
import path from 'node:path';
import records, {
    AUTO_MIN,
    STACK_KEY,
    convertTables,
    parseDirective,
} from '../docs-site/src/integrations/records.mjs';

// The docs site (docs-site/) turns a table of the built HTML into "records" when it has a long
// cell or when a comment above it says so. These tests feed the integration the table markup
// that the site's Markdown processor writes (`<th style="text-align: left">`), not a real build.

const th = (h) => `<th style="text-align: left">${h}</th>`;
const td = (c) => `<td style="text-align: left">${c}</td>`;
function table(header, rows) {
    return (
        '<table>\n<thead>\n<tr>\n' + header.map(th).join('\n') + '\n</tr>\n</thead>\n<tbody>\n' +
        rows.map((r) => '<tr>\n' + r.map(td).join('\n') + '\n</tr>').join('\n') +
        '\n</tbody>\n</table>'
    );
}
const text = (html) => html.replace(/<[^>]+>/g, '').replace(/\s+/g, ' ').trim();
const project = { project: true };
const long = (n) => 'x'.repeat(n);

test('automatic threshold: 239 characters stay a table, 240 become records', () => {
    assert.equal(AUTO_MIN, 240);
    const t239 = table(['k', 'v'], [['a', long(239)]]);
    const t240 = table(['k', 'v'], [['a', long(240)]]);
    const r239 = convertTables(t239, project);
    const r240 = convertTables(t240, project);
    assert.equal(r239.html, t239);
    assert.equal(r239.converted, 0);
    assert.equal(r240.converted, 1);
    assert.match(r240.html, /^<ol class="records" role="list">/);
    assert.ok(!r240.html.includes('<table'));
});

test('the threshold counts the text of a cell, not its markup', () => {
    const marked = `<a href="/spawn-at/project/config/" class="${long(300)}">${long(100)}</a>`;
    const t = table(['k', 'v'], [['a', marked]]);
    assert.equal(convertTables(t, project).converted, 0);
    // an entity is one character, <wbr> and the code-break spans add none
    const code = `<code dir="auto"><span class="code-nb">${long(120)}</span><wbr>${long(100)}</code>&lt;${long(19)}`;
    assert.equal(convertTables(table(['k', 'v'], [['a', code]]), project).converted, 1);
    const header = table([long(500), 'v'], [['a', 'short']]);
    assert.equal(convertTables(header, project).converted, 0, 'header cells do not count');
});

test('outside project/ nothing converts by itself', () => {
    const t = table(['k', 'v'], [['a', long(2000)]]);
    const r = convertTables(t, { project: false });
    assert.equal(r.html, t);
    assert.equal(convertTables(t).html, t, 'default is not a project page');
    assert.equal(r.tables, 1);
});

test('a table with one column never converts by itself', () => {
    const t = table(['only'], [[long(500)]]);
    assert.equal(convertTables(t, project).html, t);
});

test('records directive converts any table on any page, and the comment is consumed', () => {
    const t = table(['k', 'v'], [['a', 'short']]);
    const r = convertTables(`<p>x</p>\n<!-- site: records -->\n${t}\n<p>y</p>`, { project: false });
    assert.equal(r.converted, 1);
    assert.ok(!r.html.includes('site:'));
    assert.ok(r.html.startsWith('<p>x</p>\n<ol class="records"'));
    assert.ok(r.html.endsWith('</ol>\n<p>y</p>'));
    // a blank line between the comment and the table is the same markup in the built page
    assert.equal(convertTables(`<!-- site: records -->\n\n${t}`).converted, 1);
});

test('table directive beats the automatic rule and keeps the table and its comment', () => {
    const t = table(['k', 'v'], [['a', long(900)]]);
    const html = `<!-- site: table -->\n${t}`;
    const r = convertTables(html, project);
    assert.equal(r.html, html);
    assert.equal(r.converted, 0);
});

test('key=N uses column N as the key and the other columns as fields', () => {
    const t = table(['#', 'Change', 'Proof'], [['1', 'first change', 'p1'], ['2', 'second', 'p2']]);
    const r = convertTables(`<!-- site: records key=2 -->\n${t}`);
    assert.equal(r.converted, 1);
    const items = r.html.match(/<li class="record">[\s\S]*?<\/li>/g);
    assert.equal(items.length, 2);
    assert.match(items[0], /<div class="record-key">first change<\/div>/);
    assert.match(items[0], /<dt>#<\/dt><dd>1<\/dd>/);
    assert.match(items[0], /<dt>Proof<\/dt><dd>p1<\/dd>/);
    assert.ok(!items[0].includes('<dt>Change</dt>'), 'the key column is not a field');
    // the default key is column 1
    const d = convertTables(`<!-- site: records -->\n${t}`).html;
    assert.match(d, /<div class="record-key">1<\/div>/);
});

test('bad directives and keys fail loudly', () => {
    const t = table(['a', 'b'], [['1', '2']]);
    for (const [comment, why] of [
        ['<!-- site: record -->', /unknown directive/],
        ['<!-- site: records key=3 -->', /key=3.*2 column/],
        ['<!-- site: records key=0 -->', /bad option/],
        ['<!-- site: records key=x -->', /bad option/],
        ['<!-- site: table key=1 -->', /bad option/],
        ['<!-- site: records wide -->', /bad option/],
    ]) {
        assert.throws(() => convertTables(`${comment}\n${t}`), why, comment);
    }
    assert.throws(() => convertTables(`<!-- site: records -->\n${table(['a'], [['1']])}`), /1 column/);
    assert.deepEqual(parseDirective('<!-- site: records key=4 -->'), { mode: 'records', key: 4 });
    assert.deepEqual(parseDirective('<!--site:table-->'), { mode: 'table', key: 1 });
});

test('empty cells are left out, and a short row is padded', () => {
    const t = table(['K', 'A', 'B', 'C'], [['one', 'a1', '', 'c1'], ['two', '', '', ''], ['three', 'a3']]);
    const items = convertTables(`<!-- site: records -->\n${t}`).html.match(/<li class="record">[\s\S]*?<\/li>/g);
    assert.equal(items.length, 3, 'rows keep their order and none is dropped');
    assert.ok(!items[0].includes('<dt>B</dt>'));
    assert.equal((items[0].match(/<div class="field">/g) || []).length, 2);
    assert.ok(!items[1].includes('<dl'), 'a row with no field has no description list');
    assert.match(items[1], /<div class="record-key">two<\/div>/);
    assert.ok(items[2].includes('<dt>A</dt><dd>a3</dd>'));
    assert.ok(!items[2].includes('<dt>C</dt>'));
    assert.ok(items.map((i) => text(i)[0]).join('') === 'ott', 'order one, two, three');
});

test('a column without a header text is labelled by its number', () => {
    const t = table(['k', ''], [['a', 'b']]);
    assert.match(convertTables(`<!-- site: records -->\n${t}`).html, /<dt>Column 2<\/dt>/);
});

test('the key stacks above the fields when the median key is over 48 characters', () => {
    assert.equal(STACK_KEY, 48);
    const mk = (lens) => table(['k', 'v'], lens.map((n) => [long(n), 'v']));
    const run = (lens) => convertTables(`<!-- site: records -->\n${mk(lens)}`).html;
    assert.ok(!run([10, 48, 60]).includes('records-stack'), 'median 48');
    assert.ok(run([10, 49, 60]).includes('<ol class="records records-stack"'), 'median 49');
    assert.ok(!run([48, 48]).includes('records-stack'));
    assert.ok(run([48, 49]).includes('records-stack'), 'even count: the mean of the middle two, 48.5');
    assert.ok(!run([1, 1, 200]).includes('records-stack'), 'one long key does not stack the list');
    assert.ok(run([200, 200, 1]).includes('records-stack'));
});

test('links, ids, inline code, emphasis and break points are carried over unchanged', () => {
    const key = '<strong>11</strong> (<a href="#row-11" id="k11">see</a>)';
    const cell =
        '<a href="/spawn-at/project/config/">link</a> <code dir="auto"><span class="code-nb">crates/</span><wbr>' +
        '<span class="code-any">' + long(30) + '</span></code> <em>em</em> <span id="inner">s</span>';
    const t = table(['K', 'Where'], [[key, cell]]);
    const r = convertTables(`<!-- site: records -->\n${t}`).html;
    assert.ok(r.includes(`<div class="record-key">${key}</div>`));
    assert.ok(r.includes(`<dd>${cell}</dd>`));
    // no character is added or lost: key, then the label, then the cell
    const squeeze = (s) => s.replace(/\s+/g, '');
    assert.equal(squeeze(text(r)), squeeze(`${text(key)}Where${text(cell)}`));
});

test('the list is a role=list of list items with a description list per item', () => {
    const t = table(['K', 'A'], [['k1', 'a1'], ['k2', 'a2']]);
    const r = convertTables(`<!-- site: records -->\n${t}`).html;
    assert.equal(r.split('\n')[0], '<ol class="records" role="list">');
    assert.equal((r.match(/<li class="record">/g) || []).length, 2);
    assert.match(r, /<dl class="record-fields"><div class="field"><dt>A<\/dt><dd>a1<\/dd><\/div><\/dl>/);
});

test('without a table, or without a directive on a short table, the page is unchanged', () => {
    const page = '<p>text</p>\n<!-- site: records -->\n<p>not a table</p>\n<pre><code>&lt;table&gt;</code></pre>';
    const r = convertTables(page, project);
    assert.equal(r.html, page);
    assert.equal(r.tables, 0);
    const short = table(['k', 'v'], [['a', 'b']]);
    assert.equal(convertTables(short, project).html, short);
    // a directive that is not directly followed by a table does not reach the next one
    const gap = `<!-- site: records -->\n<p>x</p>\n${short}`;
    assert.equal(convertTables(gap).html, gap);
});

test('two tables on a page are decided one by one', () => {
    const a = table(['k', 'v'], [['a', long(300)]]);
    const b = table(['k', 'v'], [['a', 'b']]);
    const r = convertTables(`${a}\n<!-- site: table -->\n${a}\n${b}`, project);
    assert.equal(r.tables, 3);
    assert.equal(r.converted, 1);
    assert.equal((r.html.match(/<table>/g) || []).length, 2);
});

test('the build hook converts under project/ only, skips project/index.html, and reports a bad page', async () => {
    const dir = await mkdtemp(path.join(os.tmpdir(), 'records-test-'));
    try {
        const big = table(['k', 'v'], [['a', long(500)]]);
        for (const rel of ['project/design.html', 'project/index.html', 'install/index.html', 'index.html']) {
            await mkdir(path.dirname(path.join(dir, rel)), { recursive: true });
            await writeFile(path.join(dir, rel), `<main>${big}</main>`);
        }
        await mkdir(path.join(dir, 'bad'), { recursive: true });
        const logs = [];
        const hook = records().hooks['astro:build:done'];
        await hook({ dir: pathToFileURL(dir + '/'), logger: { info: (m) => logs.push(m) } });
        const read = (rel) => readFile(path.join(dir, rel), 'utf8');
        assert.ok((await read('project/design.html')).includes('class="records"'));
        for (const rel of ['project/index.html', 'install/index.html', 'index.html']) {
            assert.ok((await read(rel)).includes('<table>'), rel);
        }
        assert.equal(logs.length, 1);
        assert.match(logs[0], /^1 of 4 tables became records, in 1 pages/);

        await writeFile(path.join(dir, 'bad/index.html'), `<!-- site: nope -->\n${big}`);
        await assert.rejects(
            hook({ dir: pathToFileURL(dir + '/'), logger: { info() {} } }),
            /^Error: bad\/index\.html: unknown directive/,
        );
    } finally {
        await rm(dir, { recursive: true, force: true });
    }
});
