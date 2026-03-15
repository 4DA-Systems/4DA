#!/usr/bin/env node
/**
 * Wisdom Gate — PreToolUse hook
 *
 * Injects wisdom gate reminders when modifying critical files.
 * Never blocks. Only advises. Respects W-6 (paralysis is not wisdom).
 *
 * Triggers:
 * - Write/Edit to .ai/ files → Gate 1 (Architecture)
 * - Write creating large new abstractions → Gate 4 (Complexity)
 * - Bash with destructive commands → Gate 2 (Irreversibility)
 */

let input = '';
process.stdin.setEncoding('utf8');
process.stdin.on('readable', () => {
  let chunk;
  while ((chunk = process.stdin.read()) !== null) input += chunk;
});

process.stdin.on('end', () => {
  try {
    const hookData = JSON.parse(input);
    const toolName = hookData.tool_name || '';
    const toolInput = hookData.tool_input || {};

    // --- GATE 1: Architecture file modifications ---
    if (toolName === 'Write' || toolName === 'Edit') {
      const filePath = normalize(toolInput.file_path || '');

      // .ai/ canonical docs (excluding WISDOM.md which is self-referential)
      if (filePath.includes('.ai/') && !filePath.includes('WISDOM.md')) {
        const fileName = filePath.split('/').pop();
        if (['INVARIANTS.md', 'DECISIONS.md', 'ARCHITECTURE.md', 'FAILURE_MODES.md'].some(f => fileName === f)) {
          return emit({
            status: 'success',
            message: `WISDOM GATE 1 — Architecture Modification\n` +
              `Modifying ${fileName}. Autonomous checks:\n` +
              `• Was DECISIONS.md consulted for prior art?\n` +
              `• Were INVARIANTS.md constraints verified?\n` +
              `• Was MCP memory checked for relevant history?\n` +
              `Proceed if checks are satisfied. This gate advises, never blocks.`
          });
        }
      }

      // Gate 4: New file with significant abstractions
      if (toolName === 'Write') {
        const content = toolInput.content || '';
        const lines = content.split('\n').length;
        const hasAbstractions = /^(export (default )?(function|class|const|interface|type)|pub (fn|struct|enum|trait|mod))/m.test(content);

        if (hasAbstractions && lines > 100) {
          return emit({
            status: 'success',
            message: `WISDOM GATE 4 — Complexity Check (${lines} lines, new abstractions)\n` +
              `• Could this live in an existing file?\n` +
              `• Is this the minimum needed? (W-7)\n` +
              `Proceed if the complexity is justified.`
          });
        }
      }
    }

    // --- GATE 2: Destructive bash commands ---
    if (toolName === 'Bash') {
      const cmd = toolInput.command || '';
      const destructive = [
        /git\s+push\s+--force/,
        /git\s+reset\s+--hard/,
        /git\s+branch\s+-D/,
        /rm\s+-rf?\s/,
        /drop\s+table/i,
        /truncate\s+table/i,
        /git\s+clean\s+-f/,
      ];

      if (destructive.some(p => p.test(cmd))) {
        return emit({
          status: 'success',
          message: `WISDOM GATE 2 — Irreversible Action Detected\n` +
            `Command: ${cmd.substring(0, 80)}\n` +
            `• Was this confirmed with the human? (W-5)\n` +
            `• Is there a rollback path?\n` +
            `• Consider: awe_consequence_scan before proceeding\n` +
            `This gate advises, never blocks.`
        });
      }
    }

    // No gate triggered — pass through
    emit({ status: 'success' });
  } catch (e) {
    emit({ status: 'success' });
  }
});

function normalize(p) {
  return (p || '').replace(/\\/g, '/');
}

function emit(result) {
  console.log(JSON.stringify(result));
}
