#!/usr/bin/env node
// Signal Terminal Integration Tests
// Run: node scripts/test-signal-terminal.cjs
// Requires: 4DA app running (localhost:4445)

const BASE = 'http://localhost:4445';
let passed = 0, failed = 0, total = 0;

async function test(name, fn) {
  total++;
  try {
    await fn();
    passed++;
    console.log(`  \u2713 ${name}`);
  } catch (e) {
    failed++;
    console.log(`  \u2717 ${name}: ${e.message}`);
  }
}

function assert(condition, msg) {
  if (!condition) throw new Error(msg || 'Assertion failed');
}

async function get(path) {
  const res = await fetch(BASE + path);
  return { status: res.status, body: await res.text(), json: () => JSON.parse(res.headers.get('content-type')?.includes('json') ? res.text() : '{}') };
}

async function getJson(path) {
  const res = await fetch(BASE + path);
  assert(res.status === 200, `Expected 200, got ${res.status}`);
  return res.json();
}

async function run() {
  console.log('\n4DA Signal Terminal Integration Tests\n');
  console.log('Testing ' + BASE + '...\n');

  // HTML pages
  await test('GET / returns HTML', async () => {
    const res = await fetch(BASE + '/');
    assert(res.status === 200);
    const text = await res.text();
    assert(text.includes('4DA'), 'Missing 4DA in HTML');
  });

  await test('GET /setup returns HTML', async () => {
    const res = await fetch(BASE + '/setup');
    assert(res.status === 200);
  });

  await test('GET /card returns HTML', async () => {
    const res = await fetch(BASE + '/card');
    assert(res.status === 200);
  });

  await test('GET /offline returns HTML', async () => {
    const res = await fetch(BASE + '/offline');
    assert(res.status === 200);
    const text = await res.text();
    assert(text.includes('Offline'), 'Missing offline content');
  });

  await test('GET /sw.js returns JavaScript', async () => {
    const res = await fetch(BASE + '/sw.js');
    assert(res.status === 200);
    const ct = res.headers.get('content-type');
    assert(ct && ct.includes('javascript'), 'Expected JavaScript content-type');
  });

  await test('GET /api/docs returns HTML', async () => {
    const res = await fetch(BASE + '/api/docs');
    assert(res.status === 200);
  });

  await test('GET /manifest.json returns valid JSON', async () => {
    const data = await getJson('/manifest.json');
    assert(data.name === '4DA Signal Terminal');
  });

  await test('GET /icon returns SVG', async () => {
    const res = await fetch(BASE + '/icon');
    assert(res.status === 200);
    assert(res.headers.get('content-type')?.includes('svg'));
  });

  // API endpoints
  await test('GET /api/boot returns all fields', async () => {
    const data = await getJson('/api/boot');
    assert(typeof data.db_items === 'number');
    assert(typeof data.monitoring === 'boolean');
    assert(typeof data.sources === 'number');
    assert(typeof data.tech_detected === 'number');
    assert(typeof data.threshold === 'number');
  });

  await test('GET /api/status returns monitoring state', async () => {
    const data = await getJson('/api/status');
    assert(typeof data.monitoring === 'boolean');
    assert(typeof data.signals_count === 'number');
    assert(typeof data.threshold === 'number');
  });

  await test('GET /api/signals returns signals array', async () => {
    const data = await getJson('/api/signals');
    assert(Array.isArray(data.signals));
    assert(typeof data.count === 'number');
  });

  await test('GET /api/briefing returns valid response', async () => {
    const data = await getJson('/api/briefing');
    assert(typeof data.success === 'boolean');
  });

  await test('GET /api/radar returns entries', async () => {
    const data = await getJson('/api/radar');
    assert(Array.isArray(data.entries));
  });

  await test('GET /api/decisions returns windows', async () => {
    const data = await getJson('/api/decisions');
    assert(Array.isArray(data.windows));
    assert(typeof data.count === 'number');
  });

  await test('GET /api/dna returns profile', async () => {
    const data = await getJson('/api/dna');
    assert(data.identity_summary || data.error);
  });

  await test('GET /api/gaps returns gaps array', async () => {
    const data = await getJson('/api/gaps');
    assert(typeof data.count === 'number');
  });

  await test('GET /api/search?q=rust returns results', async () => {
    const data = await getJson('/api/search?q=rust');
    assert(typeof data.count === 'number');
    assert(data.query === 'rust');
  });

  await test('GET /api/score?url=https://example.com returns found:false', async () => {
    const data = await getJson('/api/score?url=https://example.com');
    assert(data.found === false);
  });

  await test('GET /api/sources returns sources', async () => {
    const data = await getJson('/api/sources');
    assert(Array.isArray(data.sources));
    assert(typeof data.count === 'number');
  });

  await test('GET /api/simulate?add=python returns simulation', async () => {
    const data = await getJson('/api/simulate?add=python');
    assert(data.action === 'add');
    assert(data.technology === 'python');
    assert(typeof data.affected_count === 'number');
  });

  // Auth tests
  await test('Wrong token returns 401', async () => {
    const res = await fetch(BASE + '/api/status', {
      headers: { 'X-4DA-Token': 'wrong_token_12345' }
    });
    assert(res.status === 401);
  });

  await test('No token on localhost returns 200 (auto-trust)', async () => {
    const res = await fetch(BASE + '/api/status');
    assert(res.status === 200);
  });

  // SSE stream test
  await test('GET /api/stream returns SSE content-type', async () => {
    const controller = new AbortController();
    setTimeout(() => controller.abort(), 2000);
    try {
      const res = await fetch(BASE + '/api/stream', { signal: controller.signal });
      assert(res.status === 200);
      assert(res.headers.get('content-type')?.includes('text/event-stream'));
    } catch (e) {
      if (e.name !== 'AbortError') throw e;
    }
  });

  // Summary
  console.log('\n' + '\u2500'.repeat(40));
  console.log(`${passed}/${total} passed, ${failed} failed`);
  console.log('\u2500'.repeat(40) + '\n');
  process.exit(failed > 0 ? 1 : 0);
}

run().catch(e => { console.error('Test runner error:', e); process.exit(1); });
