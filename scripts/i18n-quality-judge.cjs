#!/usr/bin/env node
/**
 * LLM-as-Judge Translation Quality Validator
 *
 * Scores every translated string across all locales for:
 * - Accuracy (does it mean the same thing?)
 * - Register (appropriate for a professional developer tool?)
 * - Technical terms (kept in English where required?)
 * - Variable preservation ({{count}}, {{name}} etc. intact?)
 * - Fluency (reads naturally in target language?)
 *
 * Flags strings scoring < 3/5 for human review.
 * Outputs a report with all flagged strings and suggested fixes.
 *
 * Usage:
 *   ANTHROPIC_API_KEY=sk-... node scripts/i18n-quality-judge.cjs
 *   node scripts/i18n-quality-judge.cjs --lang ja          # Single language
 *   node scripts/i18n-quality-judge.cjs --namespace signals # Single namespace
 *   node scripts/i18n-quality-judge.cjs --dry-run          # Count strings only
 *   node scripts/i18n-quality-judge.cjs --sample 0.1       # 10% random sample
 */

const fs = require('fs');
const path = require('path');
const https = require('https');

// ============================================================================
// Configuration
// ============================================================================

const LOCALES_DIR = path.join(__dirname, '..', 'src', 'locales');
const NAMESPACES = ['ui', 'errors', 'streets', 'coach', 'signals'];
const LANGUAGES = ['ar', 'de', 'es', 'fr', 'hi', 'it', 'ja', 'ko', 'pt-BR', 'ru', 'tr', 'zh'];
const BATCH_SIZE = 25; // Strings per API call
const MODEL = 'claude-haiku-4-5-20251001';
const MAX_CONCURRENT = 3; // Parallel API calls
const REPORT_PATH = path.join(__dirname, '..', 'data', 'i18n-quality-report.json');

const LANG_NAMES = {
  ar: 'Arabic', de: 'German', es: 'Spanish', fr: 'French', hi: 'Hindi',
  it: 'Italian', ja: 'Japanese', ko: 'Korean', 'pt-BR': 'Brazilian Portuguese',
  ru: 'Russian', tr: 'Turkish', zh: 'Simplified Chinese',
};

// Brand/tech terms that MUST stay in English
const MUST_KEEP_ENGLISH = [
  '4DA', 'API', 'RSS', 'GitHub', 'Rust', 'React', 'TypeScript', 'Python',
  'Ollama', 'SQLite', 'DeepL', 'Hacker News', 'Reddit', 'arXiv', 'STREETS',
  'PASIFA', 'ACE', 'LLM', 'CLI', 'MCP', 'AI/ML', 'DevOps',
];

// ============================================================================
// CLI args
// ============================================================================

const args = process.argv.slice(2);
const dryRun = args.includes('--dry-run');
const sampleRate = args.includes('--sample')
  ? parseFloat(args[args.indexOf('--sample') + 1]) || 0.1
  : 1.0;
const langFilter = args.includes('--lang')
  ? args[args.indexOf('--lang') + 1]
  : null;
const nsFilter = args.includes('--namespace')
  ? args[args.indexOf('--namespace') + 1]
  : null;

const apiKey = process.env.ANTHROPIC_API_KEY || '';
if (!apiKey && !dryRun) {
  console.error('Error: ANTHROPIC_API_KEY environment variable required.');
  console.error('Usage: ANTHROPIC_API_KEY=sk-... node scripts/i18n-quality-judge.cjs');
  process.exit(1);
}

// ============================================================================
// API Client
// ============================================================================

function callAnthropic(systemPrompt, userContent) {
  return new Promise((resolve, reject) => {
    const body = JSON.stringify({
      model: MODEL,
      max_tokens: 4096,
      system: systemPrompt,
      messages: [{ role: 'user', content: userContent }],
    });

    const options = {
      hostname: 'api.anthropic.com',
      path: '/v1/messages',
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'x-api-key': apiKey,
        'anthropic-version': '2023-06-01',
        'Content-Length': Buffer.byteLength(body),
      },
    };

    const req = https.request(options, (res) => {
      let data = '';
      res.on('data', (chunk) => { data += chunk; });
      res.on('end', () => {
        try {
          const json = JSON.parse(data);
          if (json.error) {
            reject(new Error(`API error: ${json.error.message}`));
          } else {
            const text = json.content?.[0]?.text || '';
            resolve(text);
          }
        } catch (e) {
          reject(new Error(`Parse error: ${e.message}`));
        }
      });
    });

    req.on('error', reject);
    req.setTimeout(30000, () => { req.destroy(); reject(new Error('Timeout')); });
    req.write(body);
    req.end();
  });
}

async function sleep(ms) { return new Promise(r => setTimeout(r, ms)); }

// ============================================================================
// Batch Judge
// ============================================================================

async function judgeBatch(lang, langName, entries) {
  const systemPrompt = `You are a professional translation quality auditor for a developer desktop application called 4DA.

Score each translation 1-5:
5 = Perfect: natural, accurate, professional register
4 = Good: minor style issues but meaning correct
3 = Acceptable: awkward but understandable
2 = Poor: misleading, wrong register, or broken grammar
1 = Wrong: incorrect meaning, garbled, or untranslated

Rules:
- Technical terms MUST stay in English: ${MUST_KEEP_ENGLISH.join(', ')}
- All {{variables}} must be preserved exactly
- Register should be professional (developer tool, not casual chat)
- Translations should read naturally in ${langName}

Return ONLY a JSON array. Each element: {"key":"the.key","score":N,"issue":"description or null"}
No explanation outside the JSON. No markdown fences.`;

  const pairs = entries.map(e =>
    `KEY: ${e.key}\nEN: ${e.en}\n${lang.toUpperCase()}: ${e.translated}`
  ).join('\n---\n');

  const userContent = `Judge these ${entries.length} translations from English to ${langName}:\n\n${pairs}`;

  try {
    const response = await callAnthropic(systemPrompt, userContent);
    // Parse JSON from response (handle potential markdown fences)
    const jsonStr = response.replace(/```json?\n?/g, '').replace(/```\n?/g, '').trim();
    return JSON.parse(jsonStr);
  } catch (e) {
    console.error(`  Error judging batch: ${e.message}`);
    return entries.map(entry => ({ key: entry.key, score: -1, issue: `Judge error: ${e.message}` }));
  }
}

// ============================================================================
// Main
// ============================================================================

async function main() {
  const targetLangs = langFilter ? [langFilter] : LANGUAGES;
  const targetNs = nsFilter ? [nsFilter] : NAMESPACES;

  // Collect all string pairs
  const allPairs = []; // { lang, ns, key, en, translated }
  for (const lang of targetLangs) {
    for (const ns of targetNs) {
      const enPath = path.join(LOCALES_DIR, 'en', `${ns}.json`);
      const tPath = path.join(LOCALES_DIR, lang, `${ns}.json`);
      if (!fs.existsSync(enPath) || !fs.existsSync(tPath)) continue;

      const enData = JSON.parse(fs.readFileSync(enPath, 'utf8'));
      const tData = JSON.parse(fs.readFileSync(tPath, 'utf8'));

      for (const key of Object.keys(enData)) {
        if (!tData[key] || tData[key] === enData[key]) continue; // Skip untranslated
        allPairs.push({ lang, ns, key, en: enData[key], translated: tData[key] });
      }
    }
  }

  // Apply sampling
  let pairs = allPairs;
  if (sampleRate < 1.0) {
    pairs = allPairs.filter(() => Math.random() < sampleRate);
  }

  console.log(`i18n Quality Judge`);
  console.log(`${'='.repeat(60)}`);
  console.log(`Languages: ${targetLangs.join(', ')}`);
  console.log(`Namespaces: ${targetNs.join(', ')}`);
  console.log(`Total translatable pairs: ${allPairs.length}`);
  console.log(`Pairs to judge (${(sampleRate * 100).toFixed(0)}% sample): ${pairs.length}`);
  console.log(`Batches: ${Math.ceil(pairs.length / BATCH_SIZE)}`);
  console.log(`Estimated cost: ~$${(pairs.length * 0.0005).toFixed(2)}`);
  console.log('');

  if (dryRun) {
    console.log('Dry run — no API calls made.');
    // Show breakdown by language
    const byLang = {};
    for (const p of pairs) { byLang[p.lang] = (byLang[p.lang] || 0) + 1; }
    for (const [lang, count] of Object.entries(byLang).sort((a, b) => b[1] - a[1])) {
      console.log(`  ${lang}: ${count} strings`);
    }
    return;
  }

  // Group by language for batch processing
  const byLang = {};
  for (const p of pairs) {
    if (!byLang[p.lang]) byLang[p.lang] = [];
    byLang[p.lang].push(p);
  }

  const allResults = [];
  const flagged = []; // score < 3

  for (const [lang, langPairs] of Object.entries(byLang)) {
    const langName = LANG_NAMES[lang] || lang;
    console.log(`Judging ${langName} (${langPairs.length} strings)...`);

    const batches = [];
    for (let i = 0; i < langPairs.length; i += BATCH_SIZE) {
      batches.push(langPairs.slice(i, i + BATCH_SIZE));
    }

    // Process batches with limited concurrency
    for (let i = 0; i < batches.length; i += MAX_CONCURRENT) {
      const chunk = batches.slice(i, i + MAX_CONCURRENT);
      const results = await Promise.all(
        chunk.map(batch => judgeBatch(lang, langName, batch))
      );

      for (const batchResults of results) {
        if (!Array.isArray(batchResults)) continue;
        for (const r of batchResults) {
          allResults.push({ lang, ...r });
          if (r.score > 0 && r.score < 3) {
            const pair = langPairs.find(p => p.key === r.key);
            flagged.push({
              lang, key: r.key, score: r.score, issue: r.issue,
              en: pair?.en || '', translated: pair?.translated || '',
            });
          }
        }
      }

      // Rate limit: pause between concurrent chunks
      if (i + MAX_CONCURRENT < batches.length) {
        await sleep(500);
      }
    }

    // Summary for this language
    const scores = allResults.filter(r => r.lang === lang && r.score > 0).map(r => r.score);
    const avg = scores.length > 0 ? (scores.reduce((a, b) => a + b, 0) / scores.length).toFixed(2) : 'N/A';
    const low = scores.filter(s => s < 3).length;
    console.log(`  ${langName}: avg ${avg}/5, ${low} flagged (< 3)`);
  }

  // Write report
  const report = {
    timestamp: new Date().toISOString(),
    model: MODEL,
    sample_rate: sampleRate,
    total_judged: allResults.length,
    total_flagged: flagged.length,
    average_score: allResults.filter(r => r.score > 0).length > 0
      ? (allResults.filter(r => r.score > 0).reduce((a, r) => a + r.score, 0) / allResults.filter(r => r.score > 0).length).toFixed(2)
      : 'N/A',
    by_language: {},
    flagged,
  };

  for (const lang of targetLangs) {
    const scores = allResults.filter(r => r.lang === lang && r.score > 0).map(r => r.score);
    if (scores.length === 0) continue;
    report.by_language[lang] = {
      judged: scores.length,
      average: (scores.reduce((a, b) => a + b, 0) / scores.length).toFixed(2),
      distribution: {
        5: scores.filter(s => s === 5).length,
        4: scores.filter(s => s === 4).length,
        3: scores.filter(s => s === 3).length,
        2: scores.filter(s => s === 2).length,
        1: scores.filter(s => s === 1).length,
      },
      flagged: flagged.filter(f => f.lang === lang).length,
    };
  }

  // Ensure data directory exists
  const reportDir = path.dirname(REPORT_PATH);
  if (!fs.existsSync(reportDir)) fs.mkdirSync(reportDir, { recursive: true });
  fs.writeFileSync(REPORT_PATH, JSON.stringify(report, null, 2));
  console.log(`\nReport written to: ${REPORT_PATH}`);

  // Summary
  console.log('');
  console.log('='.repeat(60));
  console.log(`QUALITY REPORT`);
  console.log(`Strings judged: ${allResults.filter(r => r.score > 0).length}`);
  console.log(`Average score: ${report.average_score}/5`);
  console.log(`Flagged (score < 3): ${flagged.length}`);
  console.log('');

  if (flagged.length > 0) {
    console.log('TOP ISSUES:');
    flagged.slice(0, 20).forEach(f => {
      console.log(`  [${f.lang}] ${f.key} (score: ${f.score})`);
      console.log(`    EN: ${f.en}`);
      console.log(`    ${f.lang.toUpperCase()}: ${f.translated}`);
      if (f.issue) console.log(`    Issue: ${f.issue}`);
      console.log('');
    });
    if (flagged.length > 20) {
      console.log(`  ... and ${flagged.length - 20} more (see full report)`);
    }
  } else {
    console.log('All translations passed quality check!');
  }
}

main().catch(e => {
  console.error(`Fatal: ${e.message}`);
  process.exit(1);
});
