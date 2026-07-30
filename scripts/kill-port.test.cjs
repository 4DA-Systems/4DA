// SPDX-License-Identifier: FSL-1.1-Apache-2.0

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const { resolvePortArg, resolveViteConfigPort } = require('./kill-port.cjs');

test('resolveViteConfigPort reads the top-level Vite server port', () => {
  assert.equal(resolveViteConfigPort('export default { server: { port: 4444 } }'), '4444');
});

test('resolveViteConfigPort ignores nested hmr.port even when it appears first', () => {
  const config = `
    export default {
      server: {
        hmr: { port: 4445 },
        port: 4444,
      },
    };
  `;

  assert.equal(resolveViteConfigPort(config), '4444');
});

test('resolveViteConfigPort ignores comments and strings containing port-like text', () => {
  const config = `
    export default {
      server: {
        // port: 1111
        label: "port: 2222",
        port: 4444,
      },
    };
  `;

  assert.equal(resolveViteConfigPort(config), '4444');
});

test('resolvePortArg accepts numeric ports and rejects invalid values', () => {
  assert.equal(resolvePortArg('4444'), '4444');
  assert.throws(() => resolvePortArg('abc'), /invalid port/);
  assert.throws(() => resolvePortArg('70000'), /invalid port/);
});

test('resolvePortArg reads vite.config.ts when requested', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), '4da-kill-port-'));
  fs.writeFileSync(
    path.join(root, 'vite.config.ts'),
    'export default { server: { hmr: { port: 4445 }, port: 4444 } }',
  );

  assert.equal(resolvePortArg('vite-config', root), '4444');
});
