// Tests for the shared transactional-email chrome.
//
// The shell had no tests of its own — its properties were only asserted
// indirectly, through the licence-email suite. That works until someone edits
// the shell for a different message type and silently breaks the licence email,
// so the chrome's own invariants are pinned here.
//
// The invariant that motivated this file: the header is a FIXED WIDTH BUDGET.
// Email has no media queries worth relying on (Gmail strips <style> in several
// contexts), so the three header cells must fit unaided on the narrowest common
// phone. `Subscription renewed` (20 chars) overflowed and squeezed the wordmark.

import test from 'node:test';
import assert from 'node:assert/strict';

import { renderShell, emailButton, keyPanel, BADGE_MAX_CHARS, BRAND } from './email-shell.js';

const shell = (over = {}) =>
  renderShell({
    title: 'T',
    preheader: 'P',
    badge: 'Signal licence',
    contentHtml: '<p>body</p>',
    footerHtml: '<p>foot</p>',
    ...over,
  });

test('the header cells refuse to wrap', () => {
  // Without nowrap an overlong badge fractures the row mid-word and drags the
  // wordmark out of alignment. With it, an overflow pushes instead of breaking —
  // a visibly wide header beats a mangled one, and it degrades predictably.
  const html = shell();
  const header = html.slice(html.indexOf('<td width="46"'), html.indexOf('</table>'));
  const nowraps = (header.match(/white-space: nowrap/g) || []).length;
  assert.equal(nowraps, 2, 'both the wordmark and badge cells are nowrap');
});

test('BADGE_MAX_CHARS reflects a real width budget, not a round number', () => {
  // 320px viewport - 64px card padding = ~256px. Sun 46 + gap 12 + wordmark ~45
  // leaves ~150px; at 11px uppercase with 0.16em tracking a char costs ~8.5px.
  assert.ok(BADGE_MAX_CHARS >= 12, 'not so tight that honest labels are impossible');
  assert.ok(BADGE_MAX_CHARS <= 17, 'not so loose that it stops catching overflow');
});

test('the preheader precedes all body content', () => {
  // If body text renders first the client fills the inbox preview from it, which
  // is how the raw licence key ended up in the preview line originally.
  const html = shell({ preheader: 'PREVIEW-ME', contentHtml: '<p>BODY-TEXT</p>' });
  assert.ok(html.indexOf('PREVIEW-ME') < html.indexOf('BODY-TEXT'));
});

test('the shell declares light colour-scheme in both required forms', () => {
  // Gmail and Apple Mail auto-invert undeclared light email, and the inversion is
  // not colour-aware — it can render the key panel near-black behind dark text.
  const html = shell();
  assert.match(html, /<meta name="color-scheme" content="light">/);
  assert.match(html, /<meta name="supported-color-schemes" content="light">/);
});

test('branding never depends on an image loading', () => {
  // Most clients block remote images by default. Any <img> must be decorative,
  // so the wordmark has to survive on its own.
  const html = shell();
  assert.ok(html.includes('>4DA</td>'), 'the text wordmark is present');
  for (const img of html.match(/<img[^>]*>/g) || []) {
    assert.match(img, /alt=""/, 'every image is decorative');
    assert.match(img, /width="\d+"[^>]*height="\d+"/, 'and reserves its box when blocked');
  }
});

test('the button paints its colour on a td, not the anchor', () => {
  // A padded <a> collapses to bare underlined text in Word-rendered Outlook.
  const b = emailButton('https://example.test', 'Go');
  assert.match(b, /<td[^>]*bgcolor="#D4AF37"/);
  assert.ok(b.includes('>Go</a>'));
});

test('the key panel keeps a long unbroken key inside the card', () => {
  // A licence key is one ~200-char token with no spaces; without break-all it
  // blows the table out and the buyer cannot read their own key.
  const p = keyPanel('4DA-' + 'x'.repeat(200));
  assert.match(p, /word-break: break-all/);
  assert.ok(p.includes(BRAND.gold), 'and carries the gold spine that marks it as the artifact');
});

test('renderShell escapes nothing itself — callers must', () => {
  // Documents the contract rather than pretending to a safety the shell lacks:
  // every interpolated value arrives pre-escaped from recovery-email.js.
  const html = shell({ badge: '<b>x</b>' });
  assert.ok(html.includes('<b>x</b>'), 'raw passthrough is the documented contract');
});
