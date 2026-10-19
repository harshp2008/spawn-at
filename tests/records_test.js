import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import os from 'node:os';
import path from 'node:path';
import records, {
    AUTO_MIN,
    SHORT_FIELD,
    STACK_KEY,
    convertTables,
    parseDirective,
    stepKey,
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
    assert.match(r240.html, /^<ul class="records" role="list">/);
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
    assert.ok(r.html.startsWith('<p>x</p>\n<ul class="records"'));
    assert.ok(r.html.endsWith('</ul>\n<p>y</p>'));
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
    // the default key is column 1 (style=cards, because keys 1 and 2 alone would make steps)
    const d = convertTables(`<!-- site: records style=cards -->\n${t}`).html;
    assert.match(d, /<div class="record-key">1<\/div>/);
});

test('bad directives and keys fail loudly', () => {
    const t = table(['a', 'b'], [['1', '2']]);
    for (const [comment, why] of [
        ['<!-- site: record -->', /unknown directive/],
        ['<!-- site: records key=3 -->', /key=3.*2 column/],
        ['<!-- site: records key=0 -->', /bad option/],
        ['<!-- site: records key=x -->', /bad option/],
        ['<!-- site: table key=1 -->', /takes no options/],
        ['<!-- site: records wide -->', /bad option/],
    ]) {
        assert.throws(() => convertTables(`${comment}\n${t}`), why, comment);
    }
    assert.throws(() => convertTables(`<!-- site: records -->\n${table(['a'], [['1']])}`), /1 column/);
    assert.deepEqual(parseDirective('<!-- site: records key=4 -->'), { mode: 'records', key: 4, style: null });
    assert.deepEqual(parseDirective('<!--site:table-->'), { mode: 'table', key: 1, style: null });
});

test('empty cells are left out, and a short row is padded', () => {
    const t = table(['K', 'A', 'B', 'C'], [['one', 'a1', '', 'c1'], ['two', '', '', ''], ['three', 'a3']]);
    const items = convertTables(`<!-- site: records -->\n${t}`).html.match(/<li class="record">[\s\S]*?<\/li>/g);
    assert.equal(items.length, 3, 'rows keep their order and none is dropped');
    assert.ok(!items[0].includes('<dt>B</dt>'));
    assert.equal((items[0].match(/<div class="field field-/g) || []).length, 2);
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
    assert.ok(run([10, 49, 60]).includes('<ul class="records records-stack"'), 'median 49');
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
    assert.equal(r.split('\n')[0], '<ul class="records" role="list">');
    assert.equal(r.split('\n').at(-1), '</ul>');
    assert.equal((r.match(/<li class="record">/g) || []).length, 2);
    assert.match(r, /<dl class="record-fields"><div class="field field-short"><dt>A<\/dt><dd>a1<\/dd><\/div><\/dl>/);
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

// ---- cards: short and long fields, steps, style= ----

const items = (html) => html.match(/<li class="record">[\s\S]*?<\/li>/g);
const cards = (header, rows, directive = '<!-- site: records -->') =>
    convertTables(`${directive}\n${table(header, rows)}`).html;

test('a field of 40 characters is short, one of 41 is long; markup does not count', () => {
    assert.equal(SHORT_FIELD, 40);
    const r = cards(['K', 'A', 'B'], [['k', long(40), long(41)]]);
    assert.ok(r.includes(`<div class="field field-short"><dt>A</dt><dd>${long(40)}</dd></div>`));
    assert.ok(r.includes(`<div class="field field-long"><dt>B</dt><dd>${long(41)}</dd></div>`));
    const marked = `<a href="/spawn-at/${long(80)}/" id="${long(80)}"><code>${long(20)}</code> &amp; <em>${long(17)}</em></a>`;
    assert.match(cards(['K', 'A'], [['k', marked]]), /field-short/, '20 + 3 + 17 = 40 characters of text');
    assert.match(cards(['K', 'A'], [['k', `${long(39)}&lt;`]]), /field-short/, 'an entity is one character');
    assert.match(cards(['K', 'A'], [['k', `${long(40)}&lt;`]]), /field-long/);
});

test('fields stay in the column order of the table, short and long alike', () => {
    const r = cards(['K', 'Long', 'Short', 'Long2'], [['k', long(60), 's', long(60)]]);
    assert.deepEqual([...r.matchAll(/<dt>(.*?)<\/dt>/g)].map((m) => m[1]), ['Long', 'Short', 'Long2']);
});

test('steps: keys that all start with a whole number give an ordered list of numbers and notes', () => {
    for (const [key, num, note] of [
        ['1', '1', ''],
        ['2 (fixed in step 1)', '2', 'fixed in step 1'],
        ['12 files', '12', 'files'],
        ['3(no space)', '3', 'no space'],
        ['0', '0', ''],
        ['7 ()', '7', ''],
        ['5 (a) and (b)', '5', '(a) and (b)'],
        ['4 (a (b) c)', '4', 'a (b) c'],
        ['6 (open', '6', '(open'],
    ]) {
        assert.deepEqual(stepKey(key), { num, note }, key);
    }
    const r = cards(['#', 'What'], [['1', 'a'], ['2 (fixed in step 1)', 'b'], ['12 files', 'c']]);
    assert.match(r, /^<ol class="records records-steps" role="list">/);
    assert.equal(items(r).length, 3);
    assert.match(r, /<div class="record-key"><span class="record-num">1<\/span><\/div>/);
    assert.match(r, /<div class="record-key"><span class="record-num">2<\/span><span class="record-note">fixed in step 1<\/span><\/div>/);
    assert.match(r, /<span class="record-num">12<\/span><span class="record-note">files<\/span>/);
    assert.ok(r.trimEnd().endsWith('</ol>'));
    assert.ok(!r.includes('<ul'));
});

test('steps: "6.22", "A1", a number after a word and a mixed list are not steps', () => {
    for (const key of ['6.22', 'A1', 'v2', '3x', '-1', '1,5', 'step 1', '', '<strong>1</strong>']) {
        assert.equal(stepKey(key), null, JSON.stringify(key));
    }
    for (const keys of [['6.22', '6.23'], ['A1', 'A2'], ['1', '2', 'three'], ['1', '2', '3x'], ['1', '', '3']]) {
        const r = cards(['#', 'What'], keys.map((k) => [k, 'x']));
        assert.match(r, /^<ul class="records" role="list">/, keys.join(','));
        assert.ok(!r.includes('record-num') && !r.includes('records-steps'), keys.join(','));
    }
});

test('steps: the note keeps its inner HTML, and parentheses around all of it are removed once', () => {
    const code = '<code dir="auto"><span class="code-nb">--wait</span></code>';
    const link = '<a href="/spawn-at/project/config/#x" id="n1">config</a>';
    const r = cards(['#', 'What'], [
        [`1 (see ${link} and ${code})`, 'a'],
        ['2 <strong>(done)</strong>', 'b'],
        [`3 ${code}`, 'c'],
    ]);
    const [a, b, c] = items(r);
    assert.ok(a.includes(`<span class="record-note">see ${link} and ${code}</span>`), a);
    assert.ok(b.includes('<span class="record-note"><strong>done</strong></span>'), b);
    assert.ok(c.includes(`<span class="record-note">${code}</span>`), c);
    assert.ok(!a.includes('<span class="record-num"><'), 'the number is plain digits');
});

test('style=steps and style=cards decide, alone or with key=N', () => {
    const t = [['1', 'first change', 'p1'], ['2', 'second', 'p2']];
    const h = ['#', 'Change', 'Proof'];
    assert.match(cards(h, t, '<!-- site: records style=cards -->'), /^<ul class="records"/, 'numbers, but cards');
    assert.match(cards(h, t, '<!-- site: records -->'), /^<ol class="records records-steps"/, 'no style: the keys decide');
    assert.match(cards(h, t, '<!-- site: records style=steps -->'), /^<ol class="records records-steps"/);
    // key=2 names a column of words; style=cards is what it would be anyway, steps would fail
    const k2 = cards(h, t, '<!-- site: records key=2 style=cards -->');
    assert.match(k2, /<div class="record-key">first change<\/div>/);
    assert.match(k2, /<dt>#<\/dt><dd>1<\/dd>/);
    assert.throws(() => cards(h, t, '<!-- site: records style=steps key=2 -->'), /every key \(column 2\)/);
    // key=1 on a table whose column 3 holds numbers
    const nums = cards(['Name', 'Text', 'N'], [['a', 'x', '10 (ten)'], ['b', 'y', '11']], '<!-- site: records key=3 style=steps -->');
    assert.match(nums, /<span class="record-num">10<\/span><span class="record-note">ten<\/span>/);
    assert.match(nums, /<dt>Name<\/dt><dd>a<\/dd>/);
    // style=steps on a table with an empty key is not allowed either
    assert.throws(() => cards(['k', 'v'], [['1', 'a'], ['', 'b']], '<!-- site: records style=steps -->'), /row 2/);
});

test('style=steps on keys that are not all numbers fails, naming the table, the row and the key', () => {
    const t = table(['Step', 'What'], [['1', 'a'], ['2', 'b'], ['six (late)', 'c']]);
    assert.throws(
        () => convertTables(`<!-- site: records style=steps -->\n${t}`),
        (e) => /table `Step \| What`/.test(e.message) && /style=steps/.test(e.message) &&
            /column 1/.test(e.message) && /row 3 has `six \(late\)`/.test(e.message),
    );
});

test('style options: parsing, errors, and style on site: table', () => {
    assert.deepEqual(parseDirective('<!-- site: records style=steps -->'), { mode: 'records', key: 1, style: 'steps' });
    assert.deepEqual(parseDirective('<!-- site: records key=2 style=cards -->'), { mode: 'records', key: 2, style: 'cards' });
    assert.deepEqual(parseDirective('<!-- site: records style=cards key=3 -->'), { mode: 'records', key: 3, style: 'cards' });
    for (const [comment, why] of [
        ['<!-- site: records style=rail -->', /unknown style `rail`/],
        ['<!-- site: records style= -->', /unknown style ``/],
        ['<!-- site: records style=Steps -->', /unknown style `Steps`/],
        ['<!-- site: table style=steps -->', /takes no options/],
        ['<!-- site: table style=cards -->', /takes no options/],
        ['<!-- site: records style=steps style=cards -->', /style given twice/],
        ['<!-- site: records style=cards style=cards -->', /style given twice/],
        ['<!-- site: records key=1 key=2 -->', /key given twice/],
    ]) {
        assert.throws(() => parseDirective(comment), why, comment);
    }
    const t = table(['a', 'b'], [['1', '2']]);
    assert.throws(() => convertTables(`<!-- site: table style=cards -->\n${t}`), /takes no options/);
});

test('the stacked key rule applies to steps as well, counting the whole key cell', () => {
    const note = (n) => `1 (${long(n)})`;
    const run = (keys) => cards(['#', 'v'], keys.map((k) => [k, 'v']));
    // "1 (" + n + ")" is n + 4 characters: 48 stays in the gutter, 49 stacks
    const at48 = run([note(44), note(44), note(44)]);
    assert.match(at48, /^<ol class="records records-steps" role="list">/);
    const at49 = run([note(45), note(45), note(45)]);
    assert.match(at49, /^<ol class="records records-stack records-steps" role="list">/);
    // cards: the same rule, unchanged
    assert.match(cards(['k', 'v'], [[long(49), 'v']]), /^<ul class="records records-stack"/);
    assert.match(cards(['k', 'v'], [[long(48), 'v']]), /^<ul class="records" /);
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
        assert.match(logs[0], /^1 of 4 tables became records \(0 of them steps\), in 1 pages/);

        await writeFile(path.join(dir, 'bad/index.html'), `<!-- site: nope -->\n${big}`);
        await assert.rejects(
            hook({ dir: pathToFileURL(dir + '/'), logger: { info() {} } }),
            /^Error: bad\/index\.html: unknown directive/,
        );
    } finally {
        await rm(dir, { recursive: true, force: true });
    }
});
