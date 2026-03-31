#!/usr/bin/env node

/**
 * STREETS Content Translation Script
 *
 * Pre-translates STREETS playbook modules at build/development time.
 * Translations ship as static markdown files — zero runtime dependency.
 *
 * Usage:
 *   node scripts/translate-streets.cjs                    # Status: show what needs translation
 *   node scripts/translate-streets.cjs --status           # Same as above
 *   node scripts/translate-streets.cjs --validate         # Check template syntax preserved
 *   node scripts/translate-streets.cjs --validate --lang ja  # Validate specific language
 *
 * This script does NOT perform translations itself — translations are done
 * during development (by Claude Code agents or human translators) and committed
 * to the repo. This script validates and reports on translation coverage.
 */

const fs = require('fs');
const path = require('path');

const STREETS_DIR = path.join(__dirname, '..', 'docs', 'streets');

const MODULES = [
  { id: 'S', file: 'module-s-sovereign-setup.md', name: 'Sovereign Setup' },
  { id: 'T', file: 'module-t-technical-moats.md', name: 'Technical Moats' },
  { id: 'R', file: 'module-r-revenue-engines.md', name: 'Revenue Engines' },
  { id: 'E1', file: 'module-e1-execution-playbook.md', name: 'Execution Playbook' },
  { id: 'E2', file: 'module-e2-evolving-edge.md', name: 'Evolving Edge' },
  { id: 'T2', file: 'module-t2-tactical-automation.md', name: 'Tactical Automation' },
  { id: 'S2', file: 'module-s2-stacking-streams.md', name: 'Stacking Streams' },
];

const LANGUAGES = [
  { code: 'ja', name: 'Japanese' },
  { code: 'zh', name: 'Chinese' },
  { code: 'ko', name: 'Korean' },
  { code: 'es', name: 'Spanish' },
  { code: 'de', name: 'German' },
  { code: 'fr', name: 'French' },
  { code: 'ar', name: 'Arabic' },
  { code: 'hi', name: 'Hindi' },
  { code: 'it', name: 'Italian' },
  { code: 'pt-BR', name: 'Portuguese' },
  { code: 'ru', name: 'Russian' },
  { code: 'tr', name: 'Turkish' },
];

// Template patterns that MUST be preserved during translation
const TEMPLATE_PATTERNS = [
  /\{@\s*mirror\s+\w+\s*@\}/g,                    // {@ mirror name @}
  /\{\?\s*if\s+[^?]+\?\}/g,                         // {? if condition ?}
  /\{\?\s*endif\s*\?\}/g,                            // {? endif ?}
  /\{=\s*[^=]+\|[^=]*=\}/g,                         // {= var | fallback("...") =}
];

function getEnglishTemplates(content) {
  const templates = new Set();
  for (const pattern of TEMPLATE_PATTERNS) {
    const matches = content.match(pattern);
    if (matches) {
      for (const m of matches) {
        templates.add(m.trim());
      }
    }
  }
  return templates;
}

function countLessons(content) {
  // Match English "## Lesson N" or translated equivalents (## + any text + digit)
  // All translations use "## <translated-word> N:" pattern
  const englishCount = (content.match(/^## Lesson \d+/gm) || []).length;
  if (englishCount > 0) return englishCount;

  // Count ## headings that contain a number (translated lesson headers)
  // e.g., "## レッスン 1:", "## Lección 1:", "## Lektion 1:", etc.
  const translatedCount = (content.match(/^## .+\d+/gm) || []).length;
  // Subtract subsection headers that happen to contain numbers
  // Lesson headers are always at the ## level and contain ": " after the number
  const lessonHeaders = (content.match(/^## [^\n]+\d+\s*[:：]/gm) || []).length;
  return lessonHeaders > 0 ? lessonHeaders : Math.min(translatedCount, 8);
}

function showStatus() {
  console.log('\nSTREETS Translation Coverage\n');
  console.log('Module'.padEnd(25) + LANGUAGES.map(l => l.code.padStart(6)).join(''));
  console.log('-'.repeat(25 + LANGUAGES.length * 6));

  let totalTranslated = 0;
  let totalPossible = MODULES.length * LANGUAGES.length;

  for (const mod of MODULES) {
    const enPath = path.join(STREETS_DIR, mod.file);
    if (!fs.existsSync(enPath)) {
      console.log(`${mod.id} (${mod.name})`.padEnd(25) + ' MISSING EN SOURCE');
      continue;
    }

    const enContent = fs.readFileSync(enPath, 'utf-8');
    const enLessons = countLessons(enContent);
    let row = `${mod.id} (${enLessons}L)`.padEnd(25);

    for (const lang of LANGUAGES) {
      const langPath = path.join(STREETS_DIR, lang.code, mod.file);
      if (fs.existsSync(langPath)) {
        const langContent = fs.readFileSync(langPath, 'utf-8');
        const langLessons = countLessons(langContent);
        const langLines = langContent.split('\n').length;
        if (langLessons === enLessons && langLines > 100) {
          row += '    OK'.padStart(6);
          totalTranslated++;
        } else {
          row += ` ${langLessons}/${enLessons}`.padStart(6);
        }
      } else {
        row += '     -'.padStart(6);
      }
    }
    console.log(row);
  }

  console.log('-'.repeat(25 + LANGUAGES.length * 6));
  console.log(`\nCoverage: ${totalTranslated}/${totalPossible} (${Math.round(totalTranslated / totalPossible * 100)}%)`);
  console.log(`\nOK = translated with correct lesson count`);
  console.log(`-  = not yet translated`);
  console.log(`N/M = has N lessons but English has M\n`);
}

function validateLanguage(langCode) {
  let errors = 0;
  let warnings = 0;

  console.log(`\nValidating ${langCode} translations:\n`);

  for (const mod of MODULES) {
    const enPath = path.join(STREETS_DIR, mod.file);
    const langPath = path.join(STREETS_DIR, langCode, mod.file);

    if (!fs.existsSync(langPath)) {
      console.log(`  ${mod.id}: SKIP (not translated)`);
      continue;
    }

    const enContent = fs.readFileSync(enPath, 'utf-8');
    const langContent = fs.readFileSync(langPath, 'utf-8');

    const enTemplates = getEnglishTemplates(enContent);
    const langTemplates = getEnglishTemplates(langContent);

    // Check all English templates exist in translation
    const missing = [];
    for (const t of enTemplates) {
      if (!langTemplates.has(t)) {
        missing.push(t);
      }
    }

    // Check lesson count matches
    const enLessons = countLessons(enContent);
    const langLessons = countLessons(langContent);

    // Check code blocks preserved
    const enCodeBlocks = (enContent.match(/```[\s\S]*?```/g) || []).length;
    const langCodeBlocks = (langContent.match(/```[\s\S]*?```/g) || []).length;

    let status = 'OK';
    if (missing.length > 0) {
      status = 'TEMPLATE ERROR';
      errors += missing.length;
    }
    if (enLessons !== langLessons) {
      status = 'LESSON MISMATCH';
      errors++;
    }

    console.log(`  ${mod.id}: ${status}`);
    if (missing.length > 0) {
      for (const t of missing.slice(0, 5)) {
        console.log(`    MISSING: ${t}`);
      }
      if (missing.length > 5) {
        console.log(`    ... and ${missing.length - 5} more`);
      }
    }
    if (enLessons !== langLessons) {
      console.log(`    Lessons: ${langLessons} (expected ${enLessons})`);
    }
    if (Math.abs(enCodeBlocks - langCodeBlocks) > 2) {
      console.log(`    WARNING: Code blocks: ${langCodeBlocks} (English has ${enCodeBlocks})`);
      warnings++;
    }
  }

  console.log(`\n  Errors: ${errors}, Warnings: ${warnings}\n`);
  return errors;
}

// CLI
const args = process.argv.slice(2);

if (args.includes('--validate')) {
  const langIdx = args.indexOf('--lang');
  if (langIdx !== -1 && args[langIdx + 1]) {
    const code = args[langIdx + 1];
    const errors = validateLanguage(code);
    process.exit(errors > 0 ? 1 : 0);
  } else {
    let totalErrors = 0;
    for (const lang of LANGUAGES) {
      totalErrors += validateLanguage(lang.code);
    }
    process.exit(totalErrors > 0 ? 1 : 0);
  }
} else {
  showStatus();
}
