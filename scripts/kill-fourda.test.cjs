// SPDX-License-Identifier: FSL-1.1-Apache-2.0

const assert = require('node:assert/strict');
const path = require('node:path');
const test = require('node:test');

const { isPathInsideTree } = require('./kill-fourda.cjs');

test('isPathInsideTree accepts executables under the current tree only', () => {
  const root = path.resolve('tmp', '4DA');

  assert.equal(
    isPathInsideTree(path.join(root, 'src-tauri', 'target', 'debug', 'fourda.exe'), root),
    true,
  );
  assert.equal(isPathInsideTree(path.join(root, 'fourda.exe'), root), true);
});

test('isPathInsideTree rejects sibling paths with the same prefix', () => {
  const root = path.resolve('tmp', '4DA');

  assert.equal(
    isPathInsideTree(path.join(`${root}-sibling`, 'src-tauri', 'target', 'debug', 'fourda.exe'), root),
    false,
  );
  assert.equal(
    isPathInsideTree(path.join(path.dirname(root), '4DA2', 'src-tauri', 'target', 'debug', 'fourda.exe'), root),
    false,
  );
});

test('isPathInsideTree rejects empty and parent paths', () => {
  const root = path.resolve('tmp', '4DA');

  assert.equal(isPathInsideTree('', root), false);
  assert.equal(isPathInsideTree(path.join(path.dirname(root), 'fourda.exe'), root), false);
});
