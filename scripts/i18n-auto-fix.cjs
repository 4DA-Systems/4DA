#!/usr/bin/env node
/**
 * Auto-fix flagged translation quality issues using LLM.
 * Reads the quality report, sends flagged strings for re-translation, applies fixes.
 *
 * Usage: ANTHROPIC_API_KEY=sk-... node scripts/i18n-auto-fix.cjs
 */

const fs = require('fs');
const path = require('path');
const https = require('https');

const LOCALES_DIR = path.join(__dirname, '..', 'src', 'locales');
const REPORT_PATH = path.join(__dirname, '..', 'data', 'i18n-quality-report.json');
const MODEL = 'claude-haiku-4-5-20251001';
const BATCH_SIZE = 20;
const MAX_CONCURRENT = 3;

const apiKey = process.env.ANTHROPIC_API_KEY || '';
if (!apiKey) { console.error('ANTHROPIC_API_KEY required'); process.exit(1); }

const LANG_NAMES = {
  ar: 'Arabic', de: 'German', es: 'Spanish', fr: 'French', hi: 'Hindi',
  it: 'Italian', ja: 'Japanese', ko: 'Korean', 'pt-BR': 'Brazilian Portuguese',
  ru: 'Russian', tr: 'Turkish', zh: 'Simplified Chinese',
};

function callAnthropic(systemPrompt, userContent) {
  return new Promise((resolve, reject) => {
    const body = JSON.stringify({
      model: MODEL, max_tokens: 4096,
      system: systemPrompt,
      messages: [{ role: 'user', content: userContent }],
    });
    const options = {
      hostname: 'api.anthropic.com', path: '/v1/messages', method: 'POST',
      headers: {
        'Content-Type': 'application/json', 'x-api-key': apiKey,
        'anthropic-version': '2023-06-01', 'Content-Length': Buffer.byteLength(body),
      },
    };
    const req = https.request(options, (res) => {
      let data = '';
      res.on('data', (chunk) => { data += chunk; });
      res.on('end', () => {
        try {
          const json = JSON.parse(data);
          if (json.error) reject(new Error(json.error.message));
          else resolve(json.content?.[0]?.text || '');
        } catch (e) { reject(e); }
      });
    });
    req.on('error', reject);
    req.setTimeout(30000, () => { req.destroy(); reject(new Error('Timeout')); });
    req.write(body);
    req.end();
  });
}

async function sleep(ms) { return new Promise(r => setTimeout(r, ms)); }

async function fixBatch(lang, langName, entries) {
  const systemPrompt = `You are a professional translator for 4DA, a developer desktop application.
Provide corrected translations for the flagged strings below.
Rules:
- Technical terms stay in English: 4DA, API, RSS, GitHub, Rust, React, TypeScript, Ollama, SQLite, DeepL, LLM, CLI, STREETS, PASIFA, ACE, AI/ML, DevOps, HN
- Preserve ALL {{variables}} exactly as written
- Use professional register appropriate for ${langName} developer tools
- Fix the specific issue noted for each string
- "Agent" in this app means AI agent (autonomous software), NOT client/customer

Return ONLY a JSON object mapping keys to corrected translations. No explanation. No markdown fences.
Example: {"key1":"corrected value","key2":"corrected value"}`;

  const items = entries.map(e =>
    `KEY: ${e.key}\nEN: ${e.en}\nCURRENT ${lang.toUpperCase()}: ${e.translated}\nISSUE: ${e.issue}`
  ).join('\n---\n');

  try {
    const response = await callAnthropic(systemPrompt, `Fix these ${entries.length} ${langName} translations:\n\n${items}`);
    const jsonStr = response.replace(/```json?\n?/g, '').replace(/```\n?/g, '').trim();
    return JSON.parse(jsonStr);
  } catch (e) {
    console.error(`  Fix error: ${e.message}`);
    return {};
  }
}

async function main() {
  if (!fs.existsSync(REPORT_PATH)) {
    console.error('No quality report found. Run i18n-quality-judge.cjs first.');
    process.exit(1);
  }

  const report = JSON.parse(fs.readFileSync(REPORT_PATH, 'utf8'));
  const flagged = report.flagged;

  console.log(`i18n Auto-Fix`);
  console.log(`${'='.repeat(60)}`);
  console.log(`Flagged strings to fix: ${flagged.length}`);

  // Group by language
  const byLang = {};
  for (const f of flagged) {
    if (!byLang[f.lang]) byLang[f.lang] = [];
    byLang[f.lang].push(f);
  }

  let totalFixed = 0;
  const allFixes = {};

  for (const [lang, items] of Object.entries(byLang)) {
    const langName = LANG_NAMES[lang] || lang;
    console.log(`\nFixing ${langName} (${items.length} strings)...`);

    // Process in batches
    const batches = [];
    for (let i = 0; i < items.length; i += BATCH_SIZE) {
      batches.push(items.slice(i, i + BATCH_SIZE));
    }

    const langFixes = {};
    for (let i = 0; i < batches.length; i += MAX_CONCURRENT) {
      const chunk = batches.slice(i, i + MAX_CONCURRENT);
      const results = await Promise.all(chunk.map(b => fixBatch(lang, langName, b)));
      for (const r of results) Object.assign(langFixes, r);
      if (i + MAX_CONCURRENT < batches.length) await sleep(500);
    }

    // Apply fixes to locale files
    // Group fixes by namespace (detect from flagged items)
    const byNs = {};
    for (const f of items) {
      if (!langFixes[f.key]) continue;
      // Determine namespace by checking which file contains the key
      for (const ns of ['ui', 'errors', 'streets', 'coach', 'signals']) {
        const nsPath = path.join(LOCALES_DIR, lang, `${ns}.json`);
        if (!fs.existsSync(nsPath)) continue;
        const nsData = JSON.parse(fs.readFileSync(nsPath, 'utf8'));
        if (f.key in nsData) {
          if (!byNs[ns]) byNs[ns] = {};
          byNs[ns][f.key] = langFixes[f.key];
          break;
        }
      }
    }

    for (const [ns, fixes] of Object.entries(byNs)) {
      const filePath = path.join(LOCALES_DIR, lang, `${ns}.json`);
      const data = JSON.parse(fs.readFileSync(filePath, 'utf8'));
      let count = 0;
      for (const [key, value] of Object.entries(fixes)) {
        if (data[key] !== value) {
          data[key] = value;
          count++;
        }
      }
      if (count > 0) {
        fs.writeFileSync(filePath, JSON.stringify(data, null, 2) + '\n');
        totalFixed += count;
        console.log(`  ${lang}/${ns}.json: ${count} strings fixed`);
      }
    }

    allFixes[lang] = langFixes;
  }

  console.log(`\n${'='.repeat(60)}`);
  console.log(`Total fixes applied: ${totalFixed}`);
  console.log(`\nRun 'node scripts/validate-translations.cjs' to verify.`);
}

main().catch(e => { console.error(`Fatal: ${e.message}`); process.exit(1); });
