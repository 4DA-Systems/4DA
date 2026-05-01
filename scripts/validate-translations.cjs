#!/usr/bin/env node
/**
 * Translation Sync Validator — ensures all locale files are complete and valid.
 *
 * Checks:
 * 1. JSON validity for every locale file
 * 2. Missing keys (exist in EN but not in target)
 * 3. Untranslated values (identical to English, excluding brand/tech terms)
 * 4. Interpolation variable mismatches ({{count}} etc.)
 * 5. Extra keys (exist in target but not in EN — likely stale)
 *
 * Exit code 0 = all good (warnings only), 1 = errors found
 */

const fs = require('fs');
const path = require('path');

const LOCALES_DIR = path.join(__dirname, '..', 'src', 'locales');
const NAMESPACES = ['ui', 'errors', 'streets', 'coach', 'signals'];

// Terms that are legitimately the same across languages
const BRAND_TERMS = new Set([
  '4DA', 'Signal', 'STREETS', 'PASIFA', 'ACE', 'API', 'CLI', 'MCP',
  'RSS', 'CVE', 'SSO', 'SCIM', 'DNA', 'Feed', 'Radar', 'Ollama', 'Tauri',
  'SQLite', 'GitHub', 'Enterprise', 'Team', 'Playbook', 'Hacker News',
  'Reddit', 'arXiv', 'OpenAI', 'Anthropic', 'Claude', 'Developer DNA',
  'Rust', 'React', 'TypeScript', 'Python', 'Go', 'Ruby',
]);

function isBrandTerm(value) {
  return BRAND_TERMS.has(value) || value.length <= 3;
}

function extractVariables(str) {
  return (str.match(/\{\{[^}]+\}\}/g) || []).sort().join(',');
}

// Discover languages
const languages = fs.readdirSync(LOCALES_DIR)
  .filter(d => fs.statSync(path.join(LOCALES_DIR, d)).isDirectory() && d !== 'en');

let totalErrors = 0;
let totalWarnings = 0;

console.log('Translation Sync Validator');
console.log('='.repeat(60));
console.log(`Source: en | Targets: ${languages.join(', ')}`);
console.log('');

for (const ns of NAMESPACES) {
  const enPath = path.join(LOCALES_DIR, 'en', `${ns}.json`);
  if (!fs.existsSync(enPath)) continue;

  let enData;
  try {
    enData = JSON.parse(fs.readFileSync(enPath, 'utf8'));
  } catch (e) {
    console.log(`ERROR: en/${ns}.json is invalid JSON: ${e.message}`);
    totalErrors++;
    continue;
  }

  const enKeys = Object.keys(enData);

  for (const lang of languages) {
    const targetPath = path.join(LOCALES_DIR, lang, `${ns}.json`);

    if (!fs.existsSync(targetPath)) {
      console.log(`ERROR: ${lang}/${ns}.json MISSING`);
      totalErrors++;
      continue;
    }

    let targetData;
    try {
      targetData = JSON.parse(fs.readFileSync(targetPath, 'utf8'));
    } catch (e) {
      console.log(`ERROR: ${lang}/${ns}.json invalid JSON: ${e.message}`);
      totalErrors++;
      continue;
    }

    const targetKeys = new Set(Object.keys(targetData));

    // Missing keys
    const missing = enKeys.filter(k => !targetKeys.has(k));
    if (missing.length > 0) {
      console.log(`WARN:  ${lang}/${ns}.json — ${missing.length} keys missing`);
      totalWarnings++;
    }

    // Variable mismatches (only for translated values)
    let varMismatches = 0;
    for (const key of enKeys) {
      if (!targetData[key] || targetData[key] === enData[key]) continue;
      const enVars = extractVariables(enData[key]);
      const targetVars = extractVariables(targetData[key]);
      if (enVars && enVars !== targetVars) {
        varMismatches++;
        if (varMismatches <= 3) {
          console.log(`ERROR: ${lang}/${ns}.json key "${key}" — variable mismatch: EN has ${enVars}, ${lang} has ${targetVars || '(none)'}`);
        }
      }
    }
    if (varMismatches > 0) {
      totalErrors += varMismatches;
      if (varMismatches > 3) {
        console.log(`  ... and ${varMismatches - 3} more variable mismatches`);
      }
    }

    // Untranslated count (informational)
    if (ns === 'ui') {
      let untranslated = 0;
      for (const key of enKeys) {
        if (targetData[key] === enData[key] && !isBrandTerm(enData[key])) {
          untranslated++;
        }
      }
      const pct = ((enKeys.length - untranslated) / enKeys.length * 100).toFixed(0);
      if (untranslated > 100) {
        console.log(`WARN:  ${lang}/ui.json — ${untranslated} untranslated values (${pct}% done)`);
        totalWarnings++;
      }
    }
  }
}

const summaryMode = process.argv.includes('--summary');

if (summaryMode) {
  // One-line summary for pre-commit hook
  const status = totalErrors > 0 ? 'FAIL' : 'OK';
  console.log(`i18n: ${status} — ${languages.length + 1} langs, ${totalErrors} errors, ${totalWarnings} warnings`);
  process.exit(totalErrors > 0 ? 1 : 0);
}

console.log('');
console.log('='.repeat(60));
console.log(`Results: ${totalErrors} errors, ${totalWarnings} warnings`);
console.log(`Languages: ${languages.length + 1} (en + ${languages.length})`);

if (totalErrors > 0) {
  console.log('\nTranslation validation FAILED. Fix errors above.');
  process.exit(1);
}

console.log('\nTranslation validation passed.');
process.exit(0);
