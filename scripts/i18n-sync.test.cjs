// The translation request must carry the English source strings byte-for-byte
// (CodeQL js/incomplete-sanitization #18). The old hand-built JSON escaped `"`
// but not `\`, so en/ui.json's real placeholder "~/notes, D:\research" reached
// the model as "D:<carriage return>esearch", and a lone trailing backslash
// made the whole request unparseable.

'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');

const { translationRequestJson } = require('./i18n-sync.cjs');

test('backslashes, quotes and newlines survive the round trip', () => {
  const batch = [
    { key: 'settings.context.addDirPlaceholder', en: 'Or add specific directory: ~/notes, D:\\research' },
    { key: 'quote', en: 'No tools match "{{query}}"' },
    { key: 'trailing', en: 'ends with a backslash \\' },
    { key: 'regex', en: 'match \\d+ then \\n literally' },
    { key: 'multi', en: 'line one\nline two' },
    { key: 'key "with" quotes', en: 'x' },
  ];
  const parsed = JSON.parse(translationRequestJson(batch));
  assert.deepEqual(parsed, Object.fromEntries(batch.map((b) => [b.key, b.en])));
});

test('a backslash is sent escaped, so it cannot form an escape sequence', () => {
  const text = translationRequestJson([{ key: 'k', en: 'D:\\research' }]);
  // Literal JSON text: "D:\\research" (escaped backslash), never "D:\research" (= CR).
  assert.ok(text.includes('"D:\\\\research"'), text);
});
