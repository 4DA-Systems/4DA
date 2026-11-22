// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// The boot screen (src/boot) is on screen before App.css has loaded, so it
// carries its own copy of the design tokens it uses. This test fails when a
// copy stops matching App.css, for either theme: without it a token change
// would leave the first second of every launch in the old colours.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { describe, it } from 'node:test';

const read = (relative) => readFileSync(new URL(`../${relative}`, import.meta.url), 'utf8');

/** `--color-x: #hex;` (CSS) or `'--color-x': '#hex',` (TS) pairs inside `block`. */
function tokens(block) {
  const found = {};
  for (const m of block.matchAll(/'?(--color-[\w-]+)'?:\s*'?(#[0-9A-Fa-f]{3,8})'?[;,]/g)) {
    found[m[1]] = m[2].toUpperCase();
  }
  return found;
}

/** The text from `opener` to the next closing brace. */
function blockAfter(text, opener) {
  const start = text.indexOf(opener);
  assert.ok(start >= 0, `"${opener}" not found`);
  return text.slice(start, text.indexOf('}', start));
}

const css = read('src/App.css');
const boot = read('src/boot/boot-theme.ts');

const THEMES = [
  { name: 'dark', css: '@theme {', boot: 'dark: {' },
  { name: 'light', css: ":root[data-theme='light'] {", boot: 'light: {' },
];

describe('boot screen design tokens', () => {
  for (const theme of THEMES) {
    it(`match App.css for the ${theme.name} theme`, () => {
      const expected = tokens(blockAfter(css, theme.css));
      const actual = tokens(blockAfter(boot, theme.boot));
      assert.ok(Object.keys(actual).length >= 8, 'the boot token block was not parsed');
      for (const [name, value] of Object.entries(actual)) {
        assert.equal(value, expected[name], `${theme.name} ${name}`);
      }
    });
  }
});
