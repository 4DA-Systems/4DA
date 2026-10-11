// The Stripe API version is pinned, and every client goes through createStripe().
// No `stripe` import here: CI runs site tests without the site's dependency tree.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, dirname, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

import { STRIPE_API_VERSION } from './stripe-api-version.js';

const SITE = join(dirname(fileURLToPath(import.meta.url)), '..');

function sources(dir) {
  const out = [];
  for (const name of readdirSync(dir)) {
    if (name === 'node_modules' || name === '_site') continue;
    const p = join(dir, name);
    if (statSync(p).isDirectory()) out.push(...sources(p));
    else if (/\.(m?js)$/.test(name) && !/\.test\.m?js$/.test(name)) out.push(p);
  }
  return out;
}

test('the pinned version is a dated Stripe release name', () => {
  assert.match(STRIPE_API_VERSION, /^\d{4}-\d{2}-\d{2}\.[a-z]+$/);
});

test('createStripe passes the pinned apiVersion', () => {
  const src = readFileSync(join(SITE, 'lib', 'stripe-client.js'), 'utf8');
  assert.match(src, /apiVersion:\s*STRIPE_API_VERSION/);
});

test('no Stripe client is built outside createStripe (an unpinned client follows the SDK default)', () => {
  const offenders = [...sources(join(SITE, 'functions')), ...sources(join(SITE, 'lib'))]
    .filter((p) => !p.endsWith(join('lib', 'stripe-client.js')))
    .filter((p) => /new\s+Stripe\s*\(/.test(readFileSync(p, 'utf8')))
    .map((p) => relative(SITE, p));
  assert.deepEqual(offenders, []);
});
