// Tests for check-retired-claims.cjs (AD-030 enforcement).
const { test } = require('node:test');
const assert = require('node:assert');
const { isRetiredClaim, scanText } = require('./check-retired-claims.cjs');

test('flags the canonical retired tagline', () => {
  assert.ok(isRetiredClaim('4DA reads the internet — and gets sharper every day.'));
  assert.ok(isRetiredClaim('Privately, locally, sharper every day.'));
});

test('flags the retired beat', () => {
  assert.ok(isRetiredClaim('It learns from how you engage with what it shows you.'));
});

test('flags compound-intelligence commercial claims', () => {
  assert.ok(isRetiredClaim('Signal adds compound intelligence to 4DA.'));
  assert.ok(isRetiredClaim('Intelligence that compounds. $12/mo.'));
  assert.ok(isRetiredClaim('scored content that compounds over time'));
});

test('flags behavior-learning as a feature name', () => {
  assert.ok(isRetiredClaim('<li>Behavior learning</li>'));
  assert.ok(isRetiredClaim('Behavioural learning kicks in'));
});

test('flags the interaction-learning promise family (GPT audit finding 5)', () => {
  // The three live strings found on 2026-08-23 — each must now trip the gate.
  assert.ok(isRetiredClaim('If it\'s relevant, the ACE engine will learn from your interaction.'));
  assert.ok(isRetiredClaim('Save items like this to train the system to surface similar content.'));
  assert.ok(isRetiredClaim('No specific technologies detected. 4DA will learn from your activity.'));
  // Variants of the same promise.
  assert.ok(isRetiredClaim('4DA learns from your behavior over time'));
  assert.ok(isRetiredClaim('the system learns from you'));
  assert.ok(isRetiredClaim('rate items — teaching the system what matters'));
});

test('allows true statements about explicit, user-authored mechanisms', () => {
  assert.ok(!isRetiredClaim('Add the technology as an interest in Settings > Interests.'));
  assert.ok(!isRetiredClaim('Explicit topic suppression works through exclusions.'));
  assert.ok(!isRetiredClaim('save, dismiss, and rate items')); // Learned Preferences (real feature)
  assert.ok(!isRetiredClaim('training the model locally with Ollama')); // not "the system"
});

test('allows the surviving true claim (re-judging)', () => {
  assert.ok(!isRetiredClaim("yesterday's noise becomes tomorrow's signal"));
  assert.ok(!isRetiredClaim('when the engine improves it re-judges the corpus'));
});

test('allows code identifiers', () => {
  assert.ok(!isRetiredClaim('const score = computeCompoundAdvantage();'));
  assert.ok(!isRetiredClaim('mod compound_score;'));
  assert.ok(!isRetiredClaim('"compound_advantage" — Measures intelligence leverage'));
  assert.ok(!isRetiredClaim('node scripts/compound-quality-check.cjs'));
});

test('allows neutral uses of compound', () => {
  assert.ok(!isRetiredClaim('interest compounds annually in finance'));
  assert.ok(!isRetiredClaim('a compound sentence'));
});

test('scanText honours the retired-ok escape hatch', () => {
  const text = [
    '<!-- retired-ok: quoting the old tagline as history -->',
    'We used to say it "gets sharper every day" — we retired that claim.',
    'But this line saying it gets sharper every day is a violation.',
  ].join('\n');
  const hits = scanText(text);
  assert.strictEqual(hits.length, 1);
  assert.strictEqual(hits[0].line, 3);
});

test('scanText reports line numbers and snippets', () => {
  const hits = scanText('clean line\nSignal sells compound intelligence here\n');
  assert.strictEqual(hits.length, 1);
  assert.strictEqual(hits[0].line, 2);
  assert.match(hits[0].snippet, /compound intelligence/);
});
