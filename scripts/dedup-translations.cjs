/**
 * Translation Deduplicator — ensures consistent translations for duplicate English values.
 *
 * Problem: Many English values (e.g., "Saved") appear under multiple keys. Across 13
 * languages, translators may have used different words for the same English term, causing
 * inconsistency. This script picks a canonical key per duplicate group and propagates
 * its translation to all other keys sharing the same English value.
 *
 * This does NOT remove or rename keys — it only harmonizes translated values.
 *
 * Usage:
 *   node scripts/dedup-translations.cjs              # apply changes
 *   node scripts/dedup-translations.cjs --dry-run    # report only, no file writes
 */

'use strict';

const fs = require('fs');
const path = require('path');

// ============================================================================
// Configuration
// ============================================================================

const LOCALES_DIR = path.join(__dirname, '..', 'src', 'locales');
const SOURCE_LANG = 'en';
const TARGET_FILE = 'ui.json';

// Minimum number of keys sharing the same value to be considered a duplicate group
const MIN_DUPLICATE_COUNT = 3;

// Minimum value length to consider (skip very short values like "ON", "OFF")
const MIN_VALUE_LENGTH = 4;

// Brand terms that should NOT be deduplicated (they may intentionally vary by context)
const BRAND_TERMS = new Set([
  '4DA',
  'STREETS',
  'PASIFA',
  'Ollama',
]);

// ============================================================================
// Helpers
// ============================================================================

function readJsonFile(filePath) {
  const raw = fs.readFileSync(filePath, 'utf8');
  return JSON.parse(raw);
}

function writeJsonFile(filePath, data) {
  // Preserve formatting: 2-space indent, trailing newline
  const json = JSON.stringify(data, null, 2) + '\n';
  fs.writeFileSync(filePath, json, 'utf8');
}

function discoverLanguages() {
  return fs.readdirSync(LOCALES_DIR).filter((entry) => {
    const fullPath = path.join(LOCALES_DIR, entry);
    return (
      fs.statSync(fullPath).isDirectory() &&
      fs.existsSync(path.join(fullPath, TARGET_FILE))
    );
  });
}

/**
 * Pick the canonical key from a set of keys sharing the same English value.
 *
 * Strategy: prefer the key that has the MOST actual (non-English) translations
 * across all languages. This avoids picking a key like "tier.team" that keeps
 * the English "Team" everywhere, when "settings.tabs.team" has proper localized
 * translations like "Equipe", "Equipo", etc.
 *
 * Tiebreakers: (1) most localized translations, (2) shortest key, (3) alphabetical.
 */
function pickCanonicalKey(keys, englishValue, allLangData) {
  // Count how many non-English languages have a localized (non-English) translation
  const nonEnLangs = Object.keys(allLangData).filter((l) => l !== SOURCE_LANG);

  const scored = keys.map((key) => {
    let localizedCount = 0;
    for (const lang of nonEnLangs) {
      const langData = allLangData[lang];
      const val = langData?.[key];
      // Count as localized if the value exists and differs from the English value
      if (val && val !== englishValue) {
        localizedCount++;
      }
    }
    return { key, localizedCount };
  });

  scored.sort((a, b) => {
    // Most localized translations first
    if (b.localizedCount !== a.localizedCount) return b.localizedCount - a.localizedCount;
    // Shorter key name (more likely to be the "primary" definition)
    if (a.key.length !== b.key.length) return a.key.length - b.key.length;
    // Alphabetical tiebreaker
    return a.key.localeCompare(b.key);
  });

  return scored[0].key;
}

// ============================================================================
// Core Logic
// ============================================================================

function findDuplicateGroups(enData, allLangData) {
  // Map: English value -> array of keys
  const valueToKeys = {};

  for (const [key, value] of Object.entries(enData)) {
    if (typeof value !== 'string') continue;
    if (value.length < MIN_VALUE_LENGTH) continue;
    if (BRAND_TERMS.has(value)) continue;

    if (!valueToKeys[value]) {
      valueToKeys[value] = [];
    }
    valueToKeys[value].push(key);
  }

  // Filter to groups with MIN_DUPLICATE_COUNT or more keys
  const groups = [];
  for (const [value, keys] of Object.entries(valueToKeys)) {
    if (keys.length >= MIN_DUPLICATE_COUNT) {
      const canonical = pickCanonicalKey(keys, value, allLangData);
      const others = keys.filter((k) => k !== canonical);
      groups.push({ value, canonical, others, totalKeys: keys.length });
    }
  }

  // Sort by key count descending, then by value alphabetically
  groups.sort((a, b) => {
    if (b.totalKeys !== a.totalKeys) return b.totalKeys - a.totalKeys;
    return a.value.localeCompare(b.value);
  });

  return groups;
}

function deduplicateLanguage(langCode, langData, groups) {
  const changes = [];

  for (const group of groups) {
    const canonicalValue = langData[group.canonical];

    // Skip if canonical key has no translation in this language
    if (canonicalValue === undefined || canonicalValue === null) continue;

    for (const otherKey of group.others) {
      const currentValue = langData[otherKey];

      // Skip if key does not exist in this language
      if (currentValue === undefined || currentValue === null) continue;

      // Skip if already matching
      if (currentValue === canonicalValue) continue;

      changes.push({
        lang: langCode,
        key: otherKey,
        canonicalKey: group.canonical,
        englishValue: group.value,
        oldValue: currentValue,
        newValue: canonicalValue,
      });

      langData[otherKey] = canonicalValue;
    }
  }

  return changes;
}

// ============================================================================
// Main
// ============================================================================

function main() {
  const dryRun = process.argv.includes('--dry-run');

  console.log('Translation Deduplicator');
  console.log('========================');
  console.log(`Mode: ${dryRun ? 'DRY RUN (no files will be modified)' : 'LIVE (files will be updated)'}`);
  console.log();

  // Step 1: Read English source
  const enPath = path.join(LOCALES_DIR, SOURCE_LANG, TARGET_FILE);
  if (!fs.existsSync(enPath)) {
    console.error(`Error: English source file not found at ${enPath}`);
    process.exit(1);
  }
  const enData = readJsonFile(enPath);
  const totalKeys = Object.keys(enData).length;
  console.log(`English source: ${totalKeys} keys in ${TARGET_FILE}`);

  // Step 2: Discover languages and load all data upfront (needed for smart canonical selection)
  const languages = discoverLanguages();
  console.log(`Languages: ${languages.join(', ')} (${languages.length} total)`);

  const allLangData = {};
  for (const lang of languages) {
    const langPath = path.join(LOCALES_DIR, lang, TARGET_FILE);
    allLangData[lang] = readJsonFile(langPath);
  }

  // Step 3: Find duplicate groups (uses all language data for smart canonical key selection)
  const groups = findDuplicateGroups(enData, allLangData);
  if (groups.length === 0) {
    console.log('No duplicate groups found. Nothing to do.');
    process.exit(0);
  }

  console.log(`Found ${groups.length} duplicate groups (values appearing ${MIN_DUPLICATE_COUNT}+ times, length > ${MIN_VALUE_LENGTH - 1}):`);
  console.log();

  const totalRedundantKeys = groups.reduce((sum, g) => sum + g.others.length, 0);

  for (const group of groups) {
    console.log(`  "${group.value}" (${group.totalKeys} keys)`);
    console.log(`    canonical: ${group.canonical}`);
    for (const other of group.others) {
      console.log(`    duplicate: ${other}`);
    }
    console.log();
  }

  console.log(`Summary: ${groups.length} groups, ${totalRedundantKeys} redundant keys to harmonize`);
  console.log();

  // Step 4: Process each language
  let totalChanges = 0;
  const changesByLang = {};

  for (const lang of languages) {
    const langData = allLangData[lang];

    const changes = deduplicateLanguage(lang, langData, groups);
    changesByLang[lang] = changes;
    totalChanges += changes.length;

    if (changes.length > 0) {
      if (!dryRun) {
        const langPath = path.join(LOCALES_DIR, lang, TARGET_FILE);
        writeJsonFile(langPath, langData);
      }

      console.log(`${lang}: ${changes.length} value(s) harmonized${dryRun ? ' (would be)' : ''}`);
      for (const change of changes) {
        console.log(`  ${change.key} (en: "${change.englishValue}")`);
        console.log(`    was: "${change.oldValue}"`);
        console.log(`    now: "${change.newValue}" (from ${change.canonicalKey})`);
      }
      console.log();
    } else {
      console.log(`${lang}: already consistent`);
    }
  }

  // Step 5: Final report
  console.log();
  console.log('========================================');
  console.log('FINAL REPORT');
  console.log('========================================');
  console.log(`Duplicate groups consolidated: ${groups.length}`);
  console.log(`Redundant entries across groups: ${totalRedundantKeys}`);
  console.log(`Total translations harmonized: ${totalChanges} across ${languages.length} languages`);
  console.log(`Potential max changes: ${totalRedundantKeys} keys x ${languages.length} langs = ${totalRedundantKeys * languages.length}`);
  console.log(`Already consistent: ${totalRedundantKeys * languages.length - totalChanges} entries`);

  if (dryRun) {
    console.log();
    console.log('DRY RUN complete. No files were modified.');
    console.log('Run without --dry-run to apply changes.');
  } else {
    console.log();
    console.log('All files updated successfully.');
  }
}

main();
