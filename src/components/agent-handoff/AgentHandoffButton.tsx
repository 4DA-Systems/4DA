// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { AgentHandoffTargets } from '../../../src-tauri/bindings/bindings/AgentHandoffTargets';
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';
import type { HandoffTarget } from '../../../src-tauri/bindings/bindings/HandoffTarget';
import { buildBriefPrompt, buildFixPrompt, canBuildFixPrompt, extractFixFacts } from '../../lib/agent-handoff';
import { cmd } from '../../lib/commands';

const NONE: AgentHandoffTargets = {
  claude_code: 'missing',
  claude_code_route: 'prefill',
  vscode_claude: 'missing',
  cursor: 'missing',
};
const CACHE_MS = 60_000;
let cached: { at: number; value: Promise<AgentHandoffTargets> } | null = null;

/** Detection is cheap but every card asks; share one answer for a minute. */
function loadTargets(): Promise<AgentHandoffTargets> {
  const now = Date.now();
  if (!cached || now - cached.at > CACHE_MS) {
    cached = {
      at: now,
      value: cmd('agent_handoff_targets')
        .then(t => t ?? NONE)
        .catch(() => NONE),
    };
  }
  return cached.value;
}

/** Test hook: forget the shared detection answer. */
export function resetAgentHandoffCache(): void {
  cached = null;
}

const ORDER: HandoffTarget[] = ['claude_code', 'vscode_claude', 'cursor'];

/** Backend errors arrive as a UserError object ({ code, title, detail, ... }). */
function errorDetail(e: unknown): string | null {
  const detail = typeof e === 'object' && e !== null && 'detail' in e ? e.detail : e;
  return typeof detail === 'string' && detail ? detail : null;
}

type Props = { mode: 'fix'; item: EvidenceItem } | { mode: 'brief'; item?: undefined };

/**
 * Hand the finding (or the Brief) to the user's own coding agent: opens Claude
 * Code, VS Code or Cursor with a prompt pre-filled, which runs only when the user
 * presses Enter there, on their own plan. "Copy prompt" covers every other agent.
 * On Windows without Windows Terminal, Claude Code's link handler cannot open a
 * window, so the prompt is copied and an empty Claude Code opens to paste into.
 */
export const AgentHandoffButton = memo(function AgentHandoffButton({ mode, item }: Props) {
  const { t } = useTranslation();
  const [targets, setTargets] = useState<AgentHandoffTargets>(NONE);
  const [menuOpen, setMenuOpen] = useState(false);
  const [copied, setCopied] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    void loadTargets().then(v => { if (live) setTargets(v); });
    return () => { live = false; };
  }, []);

  const eligible = mode === 'brief' || canBuildFixPrompt(item);
  const prompt = useMemo(
    () => (!eligible ? '' : mode === 'brief' ? buildBriefPrompt() : buildFixPrompt(item)),
    [eligible, mode, item],
  );
  const cwd = mode === 'fix' ? extractFixFacts(item).project : null;
  const ready = ORDER.filter(target => targets[target] === 'ready');
  const primary = ready[0] ?? null;

  const open = useCallback(async (target: HandoffTarget) => {
    setMenuOpen(false);
    setError(null);
    setNotice(null);
    if (target === 'claude_code' && targets.claude_code_route === 'paste') {
      // Copy while this window still has focus: the console that opens next takes it.
      try {
        await navigator.clipboard.writeText(prompt);
      } catch {
        setError(t('agentHandoff.copyFailed'));
        return;
      }
    }
    try {
      const outcome = await cmd('open_agent_handoff', {
        target,
        prompt,
        cwd: target === 'claude_code' ? cwd : null,
      });
      if (outcome === 'paste') setNotice(t('agentHandoff.pasteHint'));
    } catch (e: unknown) {
      setError(errorDetail(e) ?? t('agentHandoff.openFailed'));
    }
  }, [prompt, cwd, targets.claude_code_route, t]);

  const copy = useCallback(() => {
    setMenuOpen(false);
    setError(null);
    void navigator.clipboard.writeText(prompt).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    }).catch(() => setError(t('agentHandoff.copyFailed')));
  }, [prompt, t]);

  if (!eligible) return null;

  const label = (target: HandoffTarget) => t(`agentHandoff.${mode}.${target}`);
  const buttonClass =
    'px-3 py-1.5 text-[11px] rounded-md border border-border bg-bg-tertiary/60 text-text-secondary hover:text-text-primary hover:bg-bg-tertiary hover:border-text-primary/20 transition-colors';
  const itemClass =
    'block w-full text-start px-3 py-1.5 text-[11px] text-text-secondary hover:text-text-primary hover:bg-bg-tertiary';

  return (
    <div className="relative inline-flex flex-col items-start gap-1" data-testid="agent-handoff">
      <div className="inline-flex items-center gap-1">
        {primary ? (
          <>
            <button
              type="button"
              className={buttonClass}
              onClick={() => { void open(primary); }}
              title={t('agentHandoff.tooltip')}
            >
              {label(primary)}
            </button>
            <button
              type="button"
              className={buttonClass}
              aria-haspopup="menu"
              aria-expanded={menuOpen}
              aria-label={t('agentHandoff.more')}
              onClick={() => setMenuOpen(o => !o)}
            >
              {copied ? t('agentHandoff.copied') : '\u2026'}
            </button>
          </>
        ) : (
          <button type="button" className={buttonClass} onClick={copy} title={t('agentHandoff.copyTooltip')}>
            {copied ? t('agentHandoff.copied') : t('agentHandoff.copy')}
          </button>
        )}
      </div>
      {menuOpen && primary && (
        <div
          role="menu"
          className="absolute top-full start-0 mt-1 z-10 min-w-[12rem] rounded-md border border-border bg-bg-secondary shadow-lg py-1"
        >
          {ready.slice(1).map(target => (
            <button key={target} type="button" role="menuitem" className={itemClass} onClick={() => { void open(target); }}>
              {label(target)}
            </button>
          ))}
          <button type="button" role="menuitem" className={itemClass} onClick={copy} title={t('agentHandoff.copyTooltip')}>
            {t('agentHandoff.copy')}
          </button>
        </div>
      )}
      {targets.claude_code === 'needs_first_run' && (
        <p className="text-[10px] text-text-muted">{t('agentHandoff.needsFirstRun')}</p>
      )}
      {notice && <p role="status" className="text-[10px] text-text-secondary">{notice}</p>}
      {error && <p role="alert" className="text-[10px] text-error">{error}</p>}
    </div>
  );
});
