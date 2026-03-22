#!/usr/bin/env node
/**
 * GAME Shader Validator — prevents black-screen bugs from shipping.
 *
 * Extracts WGSL and GLSL shader strings from all Platonic geometry component
 * .js files and validates basic syntax. Catches:
 * - Undefined identifiers (like `func_disabled`)
 * - Use-before-declaration in WGSL
 * - Unbalanced braces/brackets
 * - Missing required function signatures
 * - Invalid WGSL keywords used in GLSL and vice versa
 *
 * Usage: node scripts/validate-shaders.cjs
 * Exit code: 0 = all valid, 1 = errors found
 */

const fs = require('fs');
const path = require('path');

const COMPONENT_DIR = path.join(__dirname, '..', 'src', 'lib', 'game-components');

// Platonic geometry components to validate
const COMPONENTS = [
  'tetrahedron.js',
  'pentachoron.js',
  'icosahedron.js',
  'dodecahedron.js',
  'compound-five-tetrahedra.js',
  'simplex-unfold.js',
];

let errors = 0;
let warnings = 0;

function error(file, msg) {
  console.error(`  \x1b[31mERROR\x1b[0m [${file}] ${msg}`);
  errors++;
}

function warn(file, msg) {
  console.warn(`  \x1b[33mWARN\x1b[0m  [${file}] ${msg}`);
  warnings++;
}

function pass(file, msg) {
  console.log(`  \x1b[32mPASS\x1b[0m  [${file}] ${msg}`);
}

function extractShaderStrings(content) {
  const shaders = {};
  // Extract WGSL_F = `...`;
  const wgslMatch = content.match(/const WGSL_F = `([\s\S]*?)`;/);
  if (wgslMatch) shaders.wgsl = wgslMatch[1];
  // Extract GLSL_F = `...`;
  const glslMatch = content.match(/const GLSL_F = `([\s\S]*?)`;/);
  if (glslMatch) shaders.glsl = glslMatch[1];
  return shaders;
}

function checkBalancedBraces(shader, type) {
  let depth = 0;
  for (const ch of shader) {
    if (ch === '{') depth++;
    if (ch === '}') depth--;
    if (depth < 0) return `Unbalanced braces in ${type} shader (extra closing brace)`;
  }
  if (depth !== 0) return `Unbalanced braces in ${type} shader (${depth} unclosed)`;
  return null;
}

function checkWgslSyntax(wgsl, file) {
  // Check for common WGSL issues
  const issues = [];

  // 1. Check for undefined identifiers (known bad patterns)
  const badIdents = ['func_disabled', 'undefined', 'null', 'NaN'];
  for (const ident of badIdents) {
    if (wgsl.includes(ident)) {
      issues.push(`Contains invalid identifier '${ident}'`);
    }
  }

  // 2. Check for GLSL-only syntax in WGSL
  if (wgsl.includes('void main()')) {
    issues.push('Contains GLSL "void main()" in WGSL shader');
  }
  if (wgsl.match(/\bfragColor\b/) && !wgsl.includes('var fragColor')) {
    issues.push('Uses GLSL "fragColor" without declaration in WGSL');
  }

  // 3. Check balanced braces
  const braceCheck = checkBalancedBraces(wgsl, 'WGSL');
  if (braceCheck) issues.push(braceCheck);

  // 4. Required WGSL structures
  if (!wgsl.includes('@fragment')) {
    issues.push('Missing @fragment annotation');
  }
  if (!wgsl.includes('fn fs_main')) {
    issues.push('Missing fs_main function');
  }
  if (!wgsl.includes('struct Uniforms')) {
    issues.push('Missing Uniforms struct');
  }

  // 5. Check for use-before-declaration patterns
  // Extract let declarations and their usage order
  const letDecls = [];
  const lines = wgsl.split('\n');
  for (let i = 0; i < lines.length; i++) {
    const letMatch = lines[i].match(/\blet\s+(\w+)\s*=/);
    if (letMatch) {
      letDecls.push({ name: letMatch[1], line: i });
    }
  }
  // Check if any identifier is used before its let declaration
  for (const decl of letDecls) {
    for (let i = 0; i < decl.line; i++) {
      // Skip comments and string literals
      const line = lines[i].replace(/\/\/.*$/, '');
      // Check for usage (not in a let declaration or fn parameter)
      const usageRegex = new RegExp(`\\b${decl.name}\\b`);
      if (usageRegex.test(line) && !line.match(/\blet\s+/)) {
        // Check it's not in a function signature
        if (!line.match(/fn\s+\w+/) && !line.includes('struct')) {
          issues.push(`Potential use-before-declaration: '${decl.name}' used at line ${i + 1}, declared at line ${decl.line + 1}`);
        }
      }
    }
  }

  return issues;
}

function checkGlslSyntax(glsl, file) {
  const issues = [];

  // 1. Check version directive
  if (!glsl.includes('#version 300 es')) {
    issues.push('Missing "#version 300 es" directive');
  }

  // 2. Check for WGSL-only syntax in GLSL
  if (glsl.includes('vec2<f32>') || glsl.includes('vec3<f32>') || glsl.includes('vec4<f32>')) {
    issues.push('Contains WGSL type syntax (vec2<f32>) in GLSL shader');
  }
  if (glsl.includes('@fragment') || glsl.includes('@group')) {
    issues.push('Contains WGSL annotations (@fragment/@group) in GLSL shader');
  }

  // 3. Check balanced braces
  const braceCheck = checkBalancedBraces(glsl, 'GLSL');
  if (braceCheck) issues.push(braceCheck);

  // 4. Required GLSL structures
  if (!glsl.includes('void main()')) {
    issues.push('Missing main() function');
  }
  if (!glsl.includes('out vec4 fragColor') && !glsl.includes('out vec4 frag')) {
    issues.push('Missing fragColor output declaration');
  }

  return issues;
}

function checkUniforms(content, file) {
  const issues = [];

  // Extract UNIFORMS array
  const uniformsMatch = content.match(/const UNIFORMS = \[([\s\S]*?)\];/);
  if (!uniformsMatch) {
    issues.push('Missing UNIFORMS array');
    return issues;
  }

  // Count uniforms
  const uniformCount = (uniformsMatch[1].match(/name:/g) || []).length;

  // Check WGSL Uniforms struct has matching fields
  const wgslMatch = content.match(/const WGSL_F = `([\s\S]*?)`;/);
  if (wgslMatch) {
    const structMatch = wgslMatch[1].match(/struct Uniforms \{([\s\S]*?)\};/);
    if (structMatch) {
      // Count fields after mouse (the standard 10 fields)
      const fields = structMatch[1].split('\n').filter(l => l.includes(': f32')).length;
      const standardFields = 8; // time, bass, mid, treble, energy, beat + resolution(vec2) + mouse(vec2) = 6 f32 + 2 vec2 = 10 but vec2 counts as 1 field declaration
      // Actually: time, audio_bass, audio_mid, audio_treble, audio_energy, audio_beat = 6 f32
      // resolution: vec2 = 1 field, mouse: vec2 = 1 field
      // Total struct fields = 8 + custom
      // Custom f32 fields = fields - 6 (the 6 scalar f32 before resolution)
      // This is approximate — just check they exist
      if (uniformCount === 0 && fields > 8) {
        issues.push(`WGSL struct has ${fields} fields but UNIFORMS array is empty`);
      }
    }
  }

  return issues;
}

// Main validation
console.log('\n\x1b[1m=== GAME Shader Validator ===\x1b[0m\n');

for (const filename of COMPONENTS) {
  const filepath = path.join(COMPONENT_DIR, filename);
  if (!fs.existsSync(filepath)) {
    error(filename, `File not found: ${filepath}`);
    continue;
  }

  console.log(`\x1b[1m${filename}\x1b[0m`);
  const content = fs.readFileSync(filepath, 'utf-8');

  // Extract shaders
  const shaders = extractShaderStrings(content);

  // Validate WGSL
  if (shaders.wgsl) {
    const wgslIssues = checkWgslSyntax(shaders.wgsl, filename);
    if (wgslIssues.length === 0) {
      pass(filename, 'WGSL shader syntax OK');
    } else {
      for (const issue of wgslIssues) {
        if (issue.startsWith('Potential')) warn(filename, `WGSL: ${issue}`);
        else error(filename, `WGSL: ${issue}`);
      }
    }
  } else {
    error(filename, 'No WGSL fragment shader found');
  }

  // Validate GLSL
  if (shaders.glsl) {
    const glslIssues = checkGlslSyntax(shaders.glsl, filename);
    if (glslIssues.length === 0) {
      pass(filename, 'GLSL shader syntax OK');
    } else {
      for (const issue of glslIssues) {
        error(filename, `GLSL: ${issue}`);
      }
    }
  } else {
    error(filename, 'No GLSL fragment shader found');
  }

  // Check uniforms consistency
  const uniformIssues = checkUniforms(content, filename);
  for (const issue of uniformIssues) {
    warn(filename, `Uniforms: ${issue}`);
  }

  // Check custom element registration
  if (!content.includes("customElements.define('game-")) {
    error(filename, 'Missing customElements.define() registration');
  } else {
    pass(filename, 'Custom element registered');
  }

  console.log('');
}

// Summary
console.log('\x1b[1m=== Summary ===\x1b[0m');
console.log(`  ${COMPONENTS.length} components checked`);
if (errors > 0) {
  console.log(`  \x1b[31m${errors} error(s)\x1b[0m`);
}
if (warnings > 0) {
  console.log(`  \x1b[33m${warnings} warning(s)\x1b[0m`);
}
if (errors === 0 && warnings === 0) {
  console.log('  \x1b[32mAll shaders valid\x1b[0m');
}

process.exit(errors > 0 ? 1 : 0);
