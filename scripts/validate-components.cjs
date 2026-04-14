#!/usr/bin/env node
/**
 * Validates 4DA Web Component .js files (src/lib/fourda-components/) for
 * syntax correctness.
 *
 * HISTORY: These components were originally produced by the GAME compiler
 * (Generative Animation Matrix Engine). That compiler has been retired —
 * GAME evolved into Glyph, then into Airlock (agent safety middleware),
 * which is unrelated to visual rendering. The components remain as
 * standalone pre-compiled Web Components with no build-time dependency.
 *
 * This validator is a safety net for hand-edits to shader code
 * (e.g., pentachoron visual tuning). It catches syntax errors and
 * orphaned methods outside class bodies.
 *
 * Also run as part of: pnpm run validate
 */

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const COMPONENTS_DIR = path.join(__dirname, '..', 'src', 'lib', 'fourda-components');

if (!fs.existsSync(COMPONENTS_DIR)) {
  console.log('No fourda-components directory found, skipping.');
  process.exit(0);
}

const files = fs.readdirSync(COMPONENTS_DIR).filter(f => f.endsWith('.js'));

if (files.length === 0) {
  console.log('No component files found.');
  process.exit(0);
}

let failed = 0;
let passed = 0;

for (const file of files) {
  const filePath = path.join(COMPONENTS_DIR, file);
  try {
    // Syntax check via Node.js --check
    execSync(`node -c "${filePath}"`, { stdio: 'pipe' });

    // Structural check: verify no methods leak outside class bodies
    const content = fs.readFileSync(filePath, 'utf-8');
    const lines = content.split('\n');
    let braceDepth = 0;
    let inIIFE = false;
    const orphanedMethods = [];

    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];
      // Track IIFE wrapper
      if (line.includes('(function(){') || line.includes('(function ()')) inIIFE = true;

      // Count braces (rough — good enough for generated code with consistent formatting)
      for (const ch of line) {
        if (ch === '{') braceDepth++;
        if (ch === '}') braceDepth--;
      }

      // If we're at IIFE level (depth 1) or below, and see an indented method pattern,
      // it's likely an orphaned method
      if (braceDepth <= 1 && inIIFE) {
        const methodMatch = line.match(/^\s{2,}(_?\w+)\s*\([^)]*\)\s*\{/);
        if (methodMatch && !line.includes('class ') && !line.includes('function ')) {
          orphanedMethods.push({ line: i + 1, method: methodMatch[1] });
        }
      }
    }

    if (orphanedMethods.length > 0) {
      console.error(`FAIL  ${file} — ${orphanedMethods.length} orphaned method(s) outside class body:`);
      for (const m of orphanedMethods) {
        console.error(`        line ${m.line}: ${m.method}()`);
      }
      failed++;
    } else {
      console.log(`PASS  ${file}`);
      passed++;
    }
  } catch (err) {
    const stderr = err.stderr ? err.stderr.toString().trim() : err.message;
    console.error(`FAIL  ${file} — syntax error:`);
    console.error(`        ${stderr.split('\n')[0]}`);
    failed++;
  }
}

console.log(`\n${passed + failed} file(s) checked: ${passed} passed, ${failed} failed.`);

if (failed > 0) {
  console.error('\nComponent validation failed. Fix syntax before committing.');
  process.exit(1);
}
