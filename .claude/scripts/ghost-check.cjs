const fs = require('fs');

// Extract frontend CommandMap keys
const tsContent = fs.readFileSync('src/lib/commands.ts', 'utf8');
const mapMatch = tsContent.match(/interface CommandMap \{([\s\S]*?)\n\}/);
const frontendCmds = new Set();
if (mapMatch) {
  mapMatch[1].split('\n').forEach(line => {
    const m = line.match(/^\s+(\w+):/);
    if (m) frontendCmds.add(m[1]);
  });
}

// Extract Rust invoke_handler commands
const rsContent = fs.readFileSync('src-tauri/src/lib.rs', 'utf8');
const handlerMatch = rsContent.match(/invoke_handler\(tauri::generate_handler!\[([\s\S]*?)\]\)/);
const rustCmds = new Set();
if (handlerMatch) {
  handlerMatch[1].split('\n').forEach(line => {
    const trimmed = line.trim();
    if (trimmed.startsWith('//') || trimmed.indexOf('::') === -1) return;
    const parts = trimmed.replace(/,$/, '').trim().split('::');
    const cmd = parts[parts.length - 1].trim();
    if (cmd) rustCmds.add(cmd);
  });
}

console.log('Frontend commands:', frontendCmds.size);
console.log('Rust handlers:', rustCmds.size);

// Find ghosts (in frontend but not in Rust)
const ghosts = [...frontendCmds].filter(c => !rustCmds.has(c)).sort();
if (ghosts.length > 0) {
  console.log('\nGHOST COMMANDS (in frontend, no Rust handler):');
  ghosts.forEach(g => console.log('  -', g));
} else {
  console.log('\nNo ghost commands found.');
}

// Find orphans (in Rust but not in frontend)
const orphans = [...rustCmds].filter(c => !frontendCmds.has(c)).sort();
if (orphans.length > 0) {
  console.log('\nORPHAN HANDLERS (in Rust, not in frontend CommandMap):');
  orphans.forEach(o => console.log('  -', o));
}
