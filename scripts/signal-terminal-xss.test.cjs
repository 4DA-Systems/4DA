// Signal Terminal (src-tauri/src/terminal/main.js) must never turn outside text
// into live markup. The page is a browser tab on 127.0.0.1 that keeps the API
// token in localStorage and renders source titles, URLs and Crucible output,
// so an injected element or handler there can read the token and drive the API.
//
// Each case loads the REAL page (body.html + main.js, exactly as
// signal_terminal.rs::serve_terminal concatenates them) into jsdom with a
// stubbed backend, drives it the way a user would, then inspects the DOM for
// anything executable. jsdom does not fetch images, so "onerror did not fire"
// proves nothing; the assertions are structural instead: no element outside the
// renderer's vocabulary, no on* attribute, no non-http(s) href.
//
// SIGNAL_TERMINAL_JS=<path> runs the suite against another copy of main.js —
// used to confirm these cases FAIL against the pre-fix file.

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { JSDOM } = require('jsdom');

const TERMINAL_DIR = path.join(__dirname, '..', 'src-tauri', 'src', 'terminal');
const BODY = fs.readFileSync(path.join(TERMINAL_DIR, 'body.html'), 'utf8');
const MAIN_JS = fs.readFileSync(process.env.SIGNAL_TERMINAL_JS || path.join(TERMINAL_DIR, 'main.js'), 'utf8');

const PAYLOAD = '<img src=x onerror="window.__pwned=1"><script>window.__pwned=2</script>';
const ALLOWED_TAGS = new Set(['DIV', 'SPAN', 'PRE', 'A', 'H3', 'BR', 'B', 'STRONG', 'EM', 'CODE']);

function json(body) {
  return Promise.resolve({ ok: true, status: 200, json: () => Promise.resolve(body) });
}

/** Boot the page with `routes` answering /api/* and `storage` pre-seeded. */
function boot(routes = {}, storage = {}) {
  const dom = new JSDOM(`<!DOCTYPE html><html><body>${BODY}</body></html>`, {
    url: 'http://127.0.0.1:4447/',
    runScripts: 'outside-only',
    pretendToBeVisual: true,
  });
  const win = dom.window;
  win.localStorage.setItem('4da_term_token', 'test-token');
  for (const [k, v] of Object.entries(storage)) win.localStorage.setItem(k, v);
  win.fetch = (input) => {
    const u = new URL(String(input), 'http://127.0.0.1:4447');
    const handler = routes[u.pathname];
    if (handler) return json(typeof handler === 'function' ? handler(u) : handler);
    return json({});
  };
  win.EventSource = class { close() {} };
  win.HTMLElement.prototype.scrollIntoView = () => {};
  win.eval(MAIN_JS);
  opened.push(dom);
  return dom;
}

function run(dom, command) {
  const inp = dom.window.document.getElementById('inp');
  inp.value = command;
  inp.dispatchEvent(new dom.window.KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
}

const opened = [];
test.afterEach(() => { while (opened.length) opened.pop().window.close(); });

const settle = (ms = 400) => new Promise((r) => setTimeout(r, ms));

/** Every way the rendered output could execute or navigate to script. */
function executableContent(dom) {
  const problems = [];
  const out = dom.window.document.getElementById('out');
  for (const el of out.querySelectorAll('*')) {
    if (!ALLOWED_TAGS.has(el.tagName)) problems.push(`<${el.tagName.toLowerCase()}> element`);
    for (const attr of el.attributes) {
      if (/^on/i.test(attr.name)) problems.push(`${attr.name}= on <${el.tagName.toLowerCase()}>`);
      if (attr.name === 'href' && !/^https?:\/\//i.test(attr.value)) problems.push(`href=${attr.value}`);
    }
  }
  if (dom.window.__pwned !== undefined) problems.push(`__pwned=${dom.window.__pwned}`);
  return problems;
}

test('typed input (compare <tech>) renders as text, not markup', async () => {
  const dom = boot({ '/api/radar': { entries: [] }, '/api/signals': { signals: [] } });
  await settle();
  run(dom, `compare ${PAYLOAD} vue`);
  await settle();
  assert.deepEqual(executableContent(dom), []);
  assert.match(dom.window.document.getElementById('out').textContent, /<img src=x/,
    'the typed text is still shown to the user, literally');
});

test('a poisoned stored theme cannot reach the neofetch output', async () => {
  const dom = boot({ '/api/status': { total_relevant: 1 } }, { '4da_term_theme': PAYLOAD });
  await settle();
  run(dom, 'neofetch');
  await settle();
  assert.deepEqual(executableContent(dom), []);
});

test('source titles and javascript: URLs from the API stay inert', async () => {
  const signals = [
    { title: PAYLOAD, url: 'javascript:window.__pwned=3', signal_priority: 'critical', score_raw: 0.9, source: 'rss' },
    { title: 'plain', url: ' JavaScript:alert(1)', signal_priority: 'low', score_raw: 0.4, source: 'rss' },
    { title: 'good link', url: 'https://example.com/a', signal_priority: 'low', score_raw: 0.4, source: 'rss' },
  ];
  const dom = boot({ '/api/signals': { signals } });
  await settle();
  run(dom, 'signals');
  await settle();
  assert.deepEqual(executableContent(dom), []);
  const hrefs = [...dom.window.document.querySelectorAll('#out a')].map((a) => a.getAttribute('href'));
  assert.ok(hrefs.includes('https://example.com/a'), 'a real http(s) link still renders as a link');
  for (const a of dom.window.document.querySelectorAll('#out a')) {
    assert.equal(a.getAttribute('rel'), 'noopener noreferrer');
  }
});

test('a Crucible summary cannot break out of the title attribute', async () => {
  const title = 'An idea worth scanning';
  const hash = title.replace(/[^a-zA-Z0-9]/g, '').substring(0, 40);
  const cache = { [hash]: { badge: 'risky', contradicted: 1, summary: '" onmouseover="window.__pwned=4' } };
  const dom = boot(
    { '/api/signals': { signals: [{ title, url: 'https://example.com', signal_priority: 'low', score_raw: 0.5, source: 'rss' }] } },
    { '4da_crucible_cache': JSON.stringify(cache) },
  );
  await settle();
  run(dom, 'signals');
  await settle();
  assert.deepEqual(executableContent(dom), []);
  const badge = dom.window.document.querySelector('#out span[title]');
  assert.ok(badge, 'the badge rendered');
  assert.equal(badge.getAttribute('title'), '" onmouseover="window.__pwned=4', 'the summary is kept verbatim as the title');
});

test('the sink itself strips markup a caller forgot to escape', async () => {
  // cmdGaps interpolates d.count raw — the class of slip the sink must survive.
  const dom = boot({
    '/api/gaps': {
      count: `${PAYLOAD}<a href="javascript:window.__pwned=5">x</a><svg onload="window.__pwned=6"></svg>`,
      gaps: [{ dependency: 'dep', severity: 'high', days_since_engagement: 1, missed_items_count: 2 }],
    },
  });
  await settle();
  run(dom, 'gaps');
  await settle();
  assert.deepEqual(executableContent(dom), []);
});
