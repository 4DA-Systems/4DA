#!/usr/bin/env node
/**
 * i18n Sync — The single command to keep translations bulletproof.
 *
 * This is the ONE script to run when maintaining translations.
 * It handles everything: detection, generation, validation, quality.
 *
 * Commands:
 *   node scripts/i18n-sync.cjs status       — Show coverage dashboard
 *   node scripts/i18n-sync.cjs check        — Validate all locales (CI mode)
 *   node scripts/i18n-sync.cjs fill         — Generate missing keys for all locales
 *   node scripts/i18n-sync.cjs quality      — Run LLM quality judge (needs ANTHROPIC_API_KEY)
 *   node scripts/i18n-sync.cjs fix          — Auto-fix flagged quality issues
 *   node scripts/i18n-sync.cjs add-lang XX  — Add a new language (full pipeline)
 *   node scripts/i18n-sync.cjs help         — Show this help
 *
 * The "fill" command uses the Anthropic API to translate missing keys.
 * Set ANTHROPIC_API_KEY=sk-... for fill, quality, and fix commands.
 *
 * Designed for autonomous maintenance:
 *   - Pre-commit hook calls "check --summary"
 *   - CI calls "check" (fails on errors)
 *   - Nightly cron calls "fill" (auto-generates new translations)
 *   - Pre-launch calls "quality" + "fix" (validates everything)
 */

const fs = require('fs');
const path = require('path');
const https = require('https');
const { execSync } = require('child_process');

// ============================================================================
// Config
// ============================================================================

const LOCALES_DIR = path.join(__dirname, '..', 'src', 'locales');
const NAMESPACES = ['ui', 'errors', 'streets', 'coach', 'signals'];
const SUPPORTED_LANGS = ['ar', 'de', 'es', 'fr', 'hi', 'it', 'ja', 'ko', 'pt-BR', 'ru', 'tr', 'zh'];
const DATA_DIR = path.join(__dirname, '..', 'data');
const MODEL = 'claude-haiku-4-5-20251001';
const BATCH_SIZE = 30;

const LANG_NAMES = {
  en: 'English', ar: 'Arabic', de: 'German', es: 'Spanish', fr: 'French',
  hi: 'Hindi', it: 'Italian', ja: 'Japanese', ko: 'Korean',
  'pt-BR': 'Brazilian Portuguese', ru: 'Russian', tr: 'Turkish', zh: 'Simplified Chinese',
};

const BRAND_TERMS = new Set([
  '4DA', 'Signal', 'STREETS', 'PASIFA', 'ACE', 'API', 'CLI', 'MCP',
  'RSS', 'CVE', 'SSO', 'SCIM', 'DNA', 'Feed', 'Radar', 'Ollama', 'Tauri',
  'SQLite', 'GitHub', 'Enterprise', 'Team', 'Playbook', 'Hacker News',
  'Reddit', 'arXiv', 'OpenAI', 'Anthropic', 'Claude', 'Developer DNA',
  'Rust', 'React', 'TypeScript', 'Python', 'Go', 'Ruby',
]);

// ============================================================================
// Helpers
// ============================================================================

function loadJson(filePath) {
  try { return JSON.parse(fs.readFileSync(filePath, 'utf8')); }
  catch { return null; }
}

function saveJson(filePath, data) {
  fs.writeFileSync(filePath, JSON.stringify(data, null, 2) + '\n');
}

function extractVars(str) {
  return (str.match(/\{\{[^}]+\}\}/g) || []).sort().join(',');
}

function callApi(systemPrompt, userContent) {
  const apiKey = process.env.ANTHROPIC_API_KEY;
  if (!apiKey) throw new Error('ANTHROPIC_API_KEY required');

  return new Promise((resolve, reject) => {
    const body = JSON.stringify({
      model: MODEL, max_tokens: 4096, system: systemPrompt,
      messages: [{ role: 'user', content: userContent }],
    });
    const req = https.request({
      hostname: 'api.anthropic.com', path: '/v1/messages', method: 'POST',
      headers: {
        'Content-Type': 'application/json', 'x-api-key': apiKey,
        'anthropic-version': '2023-06-01', 'Content-Length': Buffer.byteLength(body),
      },
    }, (res) => {
      let data = '';
      res.on('data', (c) => { data += c; });
      res.on('end', () => {
        try {
          const json = JSON.parse(data);
          if (json.error) reject(new Error(json.error.message));
          else resolve(json.content?.[0]?.text || '');
        } catch (e) { reject(e); }
      });
    });
    req.on('error', reject);
    req.setTimeout(60000, () => { req.destroy(); reject(new Error('Timeout')); });
    req.write(body); req.end();
  });
}

async function sleep(ms) { return new Promise(r => setTimeout(r, ms)); }

// ============================================================================
// STATUS — Coverage dashboard
// ============================================================================

function cmdStatus() {
  console.log('╔══════════════════════════════════════════════════════════════╗');
  console.log('║                   4DA Translation Status                    ║');
  console.log('╚══════════════════════════════════════════════════════════════╝');
  console.log('');

  const enTotals = {};
  let totalKeys = 0;
  for (const ns of NAMESPACES) {
    const en = loadJson(path.join(LOCALES_DIR, 'en', `${ns}.json`));
    if (!en) continue;
    enTotals[ns] = Object.keys(en).length;
    totalKeys += enTotals[ns];
  }

  console.log(`Source: English (${totalKeys} keys across ${NAMESPACES.length} namespaces)`);
  console.log(`  ${NAMESPACES.map(ns => `${ns}: ${enTotals[ns]}`).join(' | ')}`);
  console.log('');

  console.log('Language        Keys    Missing  Translated  Coverage');
  console.log('─'.repeat(60));

  for (const lang of SUPPORTED_LANGS) {
    let langKeys = 0, langMissing = 0, langUntranslated = 0;
    for (const ns of NAMESPACES) {
      const en = loadJson(path.join(LOCALES_DIR, 'en', `${ns}.json`));
      const target = loadJson(path.join(LOCALES_DIR, lang, `${ns}.json`));
      if (!en) continue;
      const enKeys = Object.keys(en);
      langKeys += enKeys.length;
      if (!target) { langMissing += enKeys.length; continue; }
      for (const k of enKeys) {
        if (!(k in target)) langMissing++;
        else if (target[k] === en[k] && !BRAND_TERMS.has(en[k]) && en[k].length > 3) langUntranslated++;
      }
    }
    const translated = langKeys - langMissing - langUntranslated;
    const pct = ((translated / langKeys) * 100).toFixed(1);
    const bar = '█'.repeat(Math.floor(parseFloat(pct) / 5)) + '░'.repeat(20 - Math.floor(parseFloat(pct) / 5));
    const name = (LANG_NAMES[lang] || lang).padEnd(14);
    console.log(`${name}  ${String(langKeys).padStart(5)}  ${String(langMissing).padStart(7)}  ${String(translated).padStart(10)}  ${bar} ${pct}%`);
  }

  console.log('');

  // Quality report if exists
  const reportPath = path.join(DATA_DIR, 'i18n-quality-report.json');
  if (fs.existsSync(reportPath)) {
    const report = loadJson(reportPath);
    if (report) {
      console.log(`Last quality audit: ${report.timestamp}`);
      console.log(`  Strings judged: ${report.total_judged} | Avg: ${report.average_score}/5 | Flagged: ${report.total_flagged}`);
    }
  } else {
    console.log('No quality audit run yet. Use: pnpm i18n:quality:sample');
  }
}

// ============================================================================
// CHECK — Validate (used by CI and pre-commit)
// ============================================================================

function cmdCheck() {
  // Delegate to existing validator
  try {
    const args = process.argv.includes('--summary') ? '--summary' : '';
    execSync(`node scripts/validate-translations.cjs ${args}`, { stdio: 'inherit', cwd: path.join(__dirname, '..') });
  } catch (e) {
    process.exit(e.status || 1);
  }
}

// ============================================================================
// FILL — Generate missing translations via LLM
// ============================================================================

async function cmdFill() {
  if (!process.env.ANTHROPIC_API_KEY) {
    console.error('ANTHROPIC_API_KEY required for fill command.');
    console.error('Usage: ANTHROPIC_API_KEY=sk-... node scripts/i18n-sync.cjs fill');
    process.exit(1);
  }

  console.log('Scanning for missing translations...');

  const toTranslate = {}; // lang -> [{ ns, key, en }]
  for (const lang of SUPPORTED_LANGS) {
    toTranslate[lang] = [];
    for (const ns of NAMESPACES) {
      const en = loadJson(path.join(LOCALES_DIR, 'en', `${ns}.json`));
      const target = loadJson(path.join(LOCALES_DIR, lang, `${ns}.json`));
      if (!en) continue;
      for (const key of Object.keys(en)) {
        if (!target || !(key in target)) {
          toTranslate[lang].push({ ns, key, en: en[key] });
        }
      }
    }
  }

  const totalMissing = Object.values(toTranslate).reduce((a, b) => a + b.length, 0);
  if (totalMissing === 0) {
    console.log('All translations up to date! Nothing to fill.');
    return;
  }

  console.log(`Found ${totalMissing} missing keys across ${SUPPORTED_LANGS.length} languages.`);
  console.log(`Estimated cost: ~$${(totalMissing * 0.0005).toFixed(2)}`);
  console.log('');

  for (const [lang, items] of Object.entries(toTranslate)) {
    if (items.length === 0) continue;
    const langName = LANG_NAMES[lang] || lang;
    console.log(`Translating ${langName} (${items.length} keys)...`);

    // Batch translate
    const batches = [];
    for (let i = 0; i < items.length; i += BATCH_SIZE) {
      batches.push(items.slice(i, i + BATCH_SIZE));
    }

    const allTranslated = {};
    for (const batch of batches) {
      const systemPrompt = `You are a professional translator for 4DA, a developer desktop application.
Translate from English to ${langName}.
Rules:
- Keep technical terms in English: 4DA, API, RSS, GitHub, Rust, React, TypeScript, Ollama, SQLite, DeepL, LLM, CLI, STREETS, PASIFA, ACE, AI/ML, DevOps, HN
- Preserve ALL {{variables}} exactly
- Professional register for developer tools
- "Agent" means AI software agent, NOT client/customer
- Return ONLY a JSON object: {"key":"translation","key2":"translation2"}
- No markdown fences, no explanation`;

      const pairs = batch.map(item => `"${item.key}": "${item.en.replace(/"/g, '\\"')}"`).join(',\n');
      const userContent = `Translate these ${batch.length} strings to ${langName}:\n\n{\n${pairs}\n}`;

      try {
        const response = await callApi(systemPrompt, userContent);
        const jsonStr = response.replace(/```json?\n?/g, '').replace(/```\n?/g, '').trim();
        const translated = JSON.parse(jsonStr);
        Object.assign(allTranslated, translated);
      } catch (e) {
        console.error(`  Batch error: ${e.message}`);
      }
      await sleep(300);
    }

    // Write to locale files
    const byNs = {};
    for (const item of items) {
      if (allTranslated[item.key]) {
        if (!byNs[item.ns]) byNs[item.ns] = {};
        byNs[item.ns][item.key] = allTranslated[item.key];
      }
    }

    for (const [ns, translations] of Object.entries(byNs)) {
      const filePath = path.join(LOCALES_DIR, lang, `${ns}.json`);
      const existing = loadJson(filePath) || {};
      Object.assign(existing, translations);
      saveJson(filePath, existing);
      console.log(`  ${lang}/${ns}.json: +${Object.keys(translations).length} keys`);
    }
  }

  console.log('\nDone. Run "node scripts/i18n-sync.cjs check" to verify.');
}

// ============================================================================
// ADD-LANG — Add a new language
// ============================================================================

async function cmdAddLang(langCode) {
  if (!langCode) {
    console.error('Usage: node scripts/i18n-sync.cjs add-lang XX');
    console.error('Example: node scripts/i18n-sync.cjs add-lang sv  (Swedish)');
    process.exit(1);
  }

  if (!process.env.ANTHROPIC_API_KEY) {
    console.error('ANTHROPIC_API_KEY required for add-lang command.');
    process.exit(1);
  }

  const langDir = path.join(LOCALES_DIR, langCode);
  if (fs.existsSync(langDir)) {
    console.log(`Language ${langCode} already exists. Use "fill" to add missing keys.`);
    process.exit(1);
  }

  console.log(`Adding new language: ${langCode}`);
  console.log('');

  // Step 1: Create directory and copy EN files as templates
  fs.mkdirSync(langDir, { recursive: true });
  for (const ns of NAMESPACES) {
    const enPath = path.join(LOCALES_DIR, 'en', `${ns}.json`);
    const targetPath = path.join(langDir, `${ns}.json`);
    if (fs.existsSync(enPath)) {
      fs.copyFileSync(enPath, targetPath);
    }
  }
  console.log('1. Created locale directory with EN templates');

  // Step 2: Translate all files
  console.log('2. Translating...');
  // Temporarily add to SUPPORTED_LANGS for fill
  SUPPORTED_LANGS.push(langCode);
  await cmdFill();

  // Step 3: Print registration checklist
  console.log('');
  console.log('═'.repeat(60));
  console.log('NEW LANGUAGE CHECKLIST');
  console.log('═'.repeat(60));
  console.log(`
Register "${langCode}" in these files:

  1. src/i18n/index.ts
     Add '${langCode}' to SUPPORTED_LANGS set

  2. src/utils/format-date.ts
     Add '${langCode}': '${langCode}-XX' to localeMap

  3. src/components/settings/LocaleSection.tsx
     Add { code: '${langCode}', name: 'NATIVE_NAME' } to LANGUAGES array

  4. src/i18n/rtl.ts (if RTL language)
     Add '${langCode}' to RTL_LANGUAGES set

  5. scripts/validate-translations.cjs
     Automatically detected (reads locale directories)

  6. Run: node scripts/i18n-sync.cjs check
     Verify 0 errors
`);
}

// ============================================================================
// QUALITY + FIX — Delegate to existing scripts
// ============================================================================

function cmdQuality() {
  try {
    execSync('node scripts/i18n-quality-judge.cjs ' + process.argv.slice(3).join(' '), {
      stdio: 'inherit', cwd: path.join(__dirname, '..'),
      env: { ...process.env },
    });
  } catch (e) { process.exit(e.status || 1); }
}

function cmdFix() {
  try {
    execSync('node scripts/i18n-auto-fix.cjs', {
      stdio: 'inherit', cwd: path.join(__dirname, '..'),
      env: { ...process.env },
    });
  } catch (e) { process.exit(e.status || 1); }
}

// ============================================================================
// HELP
// ============================================================================

function cmdHelp() {
  console.log(`
4DA i18n Sync — Translation maintenance tool

Commands:
  status              Show translation coverage dashboard
  check               Validate all locales (0 = pass, 1 = fail)
  check --summary     One-line summary (for pre-commit hook)
  fill                Generate missing translations via LLM API
  quality             Run LLM quality judge on all translations
  quality --sample N  Judge N% random sample (e.g., --sample 0.1)
  quality --lang XX   Judge single language
  fix                 Auto-fix flagged quality issues via LLM
  add-lang XX         Add a new language (creates + translates + checklist)
  help                Show this help

Environment:
  ANTHROPIC_API_KEY   Required for fill, quality, fix, add-lang commands

Examples:
  node scripts/i18n-sync.cjs status
  ANTHROPIC_API_KEY=sk-... node scripts/i18n-sync.cjs fill
  ANTHROPIC_API_KEY=sk-... node scripts/i18n-sync.cjs quality --sample 0.1
  ANTHROPIC_API_KEY=sk-... node scripts/i18n-sync.cjs add-lang sv

npm scripts:
  pnpm i18n:status          → status
  pnpm i18n:validate        → check
  pnpm i18n:quality         → quality (full)
  pnpm i18n:quality:sample  → quality --sample 0.1
`);
}

// ============================================================================
// Main
// ============================================================================

const command = process.argv[2] || 'help';

switch (command) {
  case 'status': cmdStatus(); break;
  case 'check': cmdCheck(); break;
  case 'fill': cmdFill().catch(e => { console.error(e.message); process.exit(1); }); break;
  case 'quality': cmdQuality(); break;
  case 'fix': cmdFix(); break;
  case 'add-lang': cmdAddLang(process.argv[3]).catch(e => { console.error(e.message); process.exit(1); }); break;
  case 'help': case '--help': case '-h': cmdHelp(); break;
  default: console.error(`Unknown command: ${command}`); cmdHelp(); process.exit(1);
}
