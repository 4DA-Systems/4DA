import { useState, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import type { ParsedCommand, CommandExecutionResult, OsTarget, RiskLevel } from '../../types/streets';

interface StreetsCodeBlockProps {
  code: string;
  language: string;
  moduleId: string;
  lessonIdx: number;
  blockIndex: number;
}

// Detect current OS for auto-selecting the right tab
function detectCurrentOs(): OsTarget {
  const platform = navigator.platform?.toLowerCase() ?? '';
  if (platform.includes('win')) return 'windows';
  if (platform.includes('mac')) return 'mac_os';
  return 'linux';
}

const RISK_COLORS: Record<RiskLevel, { dot: string; label: string }> = {
  safe: { dot: 'bg-[#22C55E]', label: 'Safe' },
  moderate: { dot: 'bg-[#D4AF37]', label: 'Moderate' },
  elevated: { dot: 'bg-[#EF4444]', label: 'Elevated' },
};

const OS_LABELS: Record<OsTarget, string> = {
  linux: 'Linux/Mac',
  mac_os: 'macOS',
  windows: 'Windows',
  universal: 'Universal',
};

export function StreetsCodeBlock({ code, language, moduleId, lessonIdx, blockIndex }: StreetsCodeBlockProps) {
  const { t } = useTranslation();
  const [commands, setCommands] = useState<ParsedCommand[]>([]);
  const [parsed, setParsed] = useState(false);
  const [results, setResults] = useState<Map<string, CommandExecutionResult>>(new Map());
  const [running, setRunning] = useState<Set<string>>(new Set());
  const [expandedResults, setExpandedResults] = useState<Set<string>>(new Set());
  const [activeOs, setActiveOs] = useState<OsTarget>(detectCurrentOs());
  const [parseError, setParseError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  // Parse commands on first interaction
  const ensureParsed = useCallback(async () => {
    if (parsed) return commands;
    try {
      const result = await invoke<ParsedCommand[]>('parse_lesson_commands', {
        moduleId,
        lessonIdx,
      });
      // Filter to only commands from this block index
      const blockCommands = result.filter((cmd) => cmd.id.includes(`-B${blockIndex}-`));
      setCommands(blockCommands);
      setParsed(true);
      setParseError(null);
      return blockCommands;
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setParseError(msg);
      return [];
    }
  }, [parsed, commands, moduleId, lessonIdx, blockIndex]);

  // Available OS tabs from code content
  const osTabs = useMemo(() => {
    const tabs = new Set<OsTarget>();
    if (!parsed) {
      // Pre-parse detection from raw code
      const lines = code.split('\n');
      for (const line of lines) {
        const trimmed = line.trim().toLowerCase();
        if (trimmed.startsWith('#')) {
          if (trimmed.includes('windows') || trimmed.includes('powershell')) tabs.add('windows');
          else if (trimmed.includes('macos')) tabs.add('mac_os');
          else if (trimmed.includes('linux') || trimmed.includes('linux/mac')) tabs.add('linux');
        }
      }
      if (tabs.size === 0) tabs.add('universal');
    } else {
      for (const cmd of commands) {
        tabs.add(cmd.os_target);
      }
      if (tabs.size === 0) tabs.add('universal');
    }
    return Array.from(tabs);
  }, [code, commands, parsed]);

  // Filtered commands for active OS
  const filteredCommands = useMemo(() => {
    return commands.filter(
      (cmd) => cmd.os_target === activeOs || cmd.os_target === 'universal',
    );
  }, [commands, activeOs]);

  const executeCommand = useCallback(async (cmd: ParsedCommand) => {
    setRunning((prev) => new Set(prev).add(cmd.id));
    setExpandedResults((prev) => new Set(prev).add(cmd.id));
    try {
      const result = await invoke<CommandExecutionResult>('execute_streets_command', {
        commandId: cmd.id,
        command: cmd.command,
        riskLevel: cmd.risk_level,
      });
      setResults((prev) => new Map(prev).set(cmd.id, result));
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setResults((prev) => new Map(prev).set(cmd.id, {
        command_id: cmd.id,
        success: false,
        stdout: '',
        stderr: msg,
        exit_code: -1,
        duration_ms: 0,
        executed_at: new Date().toISOString(),
      }));
    } finally {
      setRunning((prev) => {
        const next = new Set(prev);
        next.delete(cmd.id);
        return next;
      });
    }
  }, []);

  const runAllSafe = useCallback(async () => {
    const cmds = parsed ? filteredCommands : await ensureParsed();
    const safeCmds = (Array.isArray(cmds) ? cmds : filteredCommands).filter(
      (c) => c.risk_level === 'safe',
    );
    for (const cmd of safeCmds) {
      await executeCommand(cmd);
      // Check if last one failed
      const lastResult = results.get(cmd.id);
      if (lastResult && !lastResult.success) break;
    }
  }, [parsed, filteredCommands, ensureParsed, executeCommand, results]);

  const handleRunClick = useCallback(async (cmd: ParsedCommand) => {
    await ensureParsed();
    await executeCommand(cmd);
  }, [ensureParsed, executeCommand]);

  const toggleResult = useCallback((id: string) => {
    setExpandedResults((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const copyBlock = useCallback(() => {
    navigator.clipboard.writeText(code).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  }, [code]);

  // Count safe commands for the "Run All Safe" button
  const safeCount = parsed
    ? filteredCommands.filter((c) => c.risk_level === 'safe').length
    : 0;

  const runningAny = running.size > 0;

  return (
    <div className="bg-bg-primary border border-border rounded-lg p-4 my-3">
      {/* Header row: OS tabs + actions */}
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-1.5">
          {/* Language label */}
          <span className="text-[10px] text-[#666] mr-2 font-mono">{language}</span>
          {/* OS tabs */}
          {osTabs.length > 1 && osTabs.map((os) => (
            <button
              key={os}
              onClick={() => { setActiveOs(os); ensureParsed(); }}
              aria-label={`Select OS: ${OS_LABELS[os]}`}
              className={`px-2 py-0.5 text-[10px] rounded font-medium transition-colors ${
                activeOs === os
                  ? 'bg-[#D4AF37]/20 text-[#D4AF37] border border-[#D4AF37]/30'
                  : 'text-[#666] hover:text-text-secondary border border-transparent'
              }`}
            >
              {OS_LABELS[os]}
            </button>
          ))}
        </div>
        <div className="flex items-center gap-2">
          {parsed && safeCount > 0 && (
            <button
              onClick={runAllSafe}
              disabled={runningAny}
              aria-label={t('playbook.code.runAllSafe', { count: safeCount })}
              className="flex items-center gap-1 px-2 py-1 text-[10px] font-medium text-[#22C55E] bg-[#22C55E]/10 border border-[#22C55E]/20 rounded hover:bg-[#22C55E]/20 transition-colors disabled:opacity-50"
            >
              {runningAny && <span className="w-3 h-3 border border-[#22C55E] border-t-transparent rounded-full animate-spin" aria-hidden="true" />}
              {t('playbook.code.runAllSafe', { count: safeCount })}
            </button>
          )}
          <button
            onClick={copyBlock}
            aria-label={copied ? t('action.copied') : t('action.copy')}
            className="px-2 py-1 text-[10px] text-[#666] hover:text-text-secondary border border-border rounded transition-colors"
          >
            {copied ? t('action.copied') : t('action.copy')}
          </button>
        </div>
      </div>

      {parseError && (
        <div className="mb-2 px-3 py-2 text-[10px] text-[#EF4444] bg-[#EF4444]/10 rounded">
          {parseError}
        </div>
      )}

      {/* Code lines */}
      <div className="space-y-0.5">
        {code.split('\n').map((line, lineIdx) => {
          const trimmed = line.trim();
          const isComment = trimmed.startsWith('#');
          const isEmpty = trimmed === '';

          // Find matching parsed command for this line
          const matchedCmd = parsed
            ? filteredCommands.find((c) => c.command === trimmed)
            : undefined;
          const result = matchedCmd ? results.get(matchedCmd.id) : undefined;
          const isRunning = matchedCmd ? running.has(matchedCmd.id) : false;
          const isExpanded = matchedCmd ? expandedResults.has(matchedCmd.id) : false;

          return (
            <div key={lineIdx}>
              <div className="flex items-center gap-2 group">
                {/* Line content */}
                <code
                  className={`flex-1 text-xs font-mono leading-relaxed px-1 ${
                    isComment
                      ? 'text-[#666] italic'
                      : isEmpty
                        ? ''
                        : 'text-text-secondary'
                  }`}
                >
                  {line || '\u00A0'}
                </code>

                {/* Run button + risk badge (only for actual commands) */}
                {!isComment && !isEmpty && (
                  <div className="flex items-center gap-1.5 opacity-0 group-hover:opacity-100 transition-opacity flex-shrink-0">
                    {matchedCmd && (
                      <>
                        <span
                          className={`w-1.5 h-1.5 rounded-full ${RISK_COLORS[matchedCmd.risk_level].dot}`}
                          title={RISK_COLORS[matchedCmd.risk_level].label}
                        />
                        <button
                          onClick={() => handleRunClick(matchedCmd)}
                          disabled={isRunning}
                          className="flex items-center justify-center w-5 h-5 text-[#22C55E] hover:bg-[#22C55E]/10 rounded transition-colors disabled:opacity-50"
                          title={t('playbook.code.runCommand')}
                          aria-label={t('playbook.code.runCommand')}
                        >
                          {isRunning ? (
                            <span className="w-3 h-3 border border-[#22C55E] border-t-transparent rounded-full animate-spin" aria-hidden="true" />
                          ) : (
                            <svg width="10" height="10" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                              <polygon points="5,3 19,12 5,21" />
                            </svg>
                          )}
                        </button>
                      </>
                    )}
                    {!matchedCmd && !parsed && (
                      <button
                        onClick={ensureParsed}
                        className="flex items-center justify-center w-5 h-5 text-[#666] hover:text-text-secondary hover:bg-bg-tertiary rounded transition-colors"
                        title={t('playbook.code.parseCommands')}
                        aria-label={t('playbook.code.parseCommands')}
                      >
                        <svg width="10" height="10" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                          <polygon points="5,3 19,12 5,21" />
                        </svg>
                      </button>
                    )}
                  </div>
                )}
              </div>

              {/* Execution result panel */}
              {result && isExpanded && (
                <div
                  className={`mt-1 mb-2 ml-1 p-3 rounded text-xs font-mono border-l-2 bg-bg-primary cursor-pointer ${
                    result.success ? 'border-[#22C55E]' : 'border-[#EF4444]'
                  }`}
                  onClick={() => matchedCmd && toggleResult(matchedCmd.id)}
                >
                  <div className="flex items-center justify-between mb-1.5">
                    <span className={`text-[10px] font-medium ${result.success ? 'text-[#22C55E]' : 'text-[#EF4444]'}`}>
                      {result.success ? t('playbook.code.success') : t('playbook.code.failed', { code: result.exit_code })}
                    </span>
                    <span className="text-[10px] text-[#666]">{result.duration_ms}ms</span>
                  </div>
                  {result.stdout && (
                    <pre className="text-text-secondary whitespace-pre-wrap break-all max-h-40 overflow-y-auto">
                      {result.stdout}
                    </pre>
                  )}
                  {result.stderr && (
                    <pre className="text-[#EF4444]/80 whitespace-pre-wrap break-all mt-1 max-h-20 overflow-y-auto">
                      {result.stderr}
                    </pre>
                  )}
                </div>
              )}

              {/* Collapsed result indicator */}
              {result && !isExpanded && matchedCmd && (
                <button
                  onClick={() => toggleResult(matchedCmd.id)}
                  aria-label={result.success ? t('playbook.code.showOutput') : t('playbook.code.showError')}
                  className={`ml-1 mt-0.5 mb-1 px-2 py-0.5 text-[10px] rounded ${
                    result.success
                      ? 'text-[#22C55E] bg-[#22C55E]/5 hover:bg-[#22C55E]/10'
                      : 'text-[#EF4444] bg-[#EF4444]/5 hover:bg-[#EF4444]/10'
                  } transition-colors`}
                >
                  {result.success ? t('playbook.code.showOutput') : t('playbook.code.showError')} ({result.duration_ms}ms)
                </button>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
