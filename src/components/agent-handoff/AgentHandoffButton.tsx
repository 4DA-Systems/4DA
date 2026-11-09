// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
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

const MENU_WIDTH_PX = 224;
/** Three items; only used to decide whether the menu fits below its button. */
const MENU_HEIGHT_EST_PX = 110;
const MENU_GAP_PX = 4;
const EDGE_PX = 8;

type MenuPos = { left: number; top?: number; bottom?: number };

/** Below the button when it fits, else above; never past either side of the window. */
function placeMenu(r: DOMRect): MenuPos {
  const rtl = document.documentElement.dir === 'rtl';
  const anchor = rtl ? r.right - MENU_WIDTH_PX : r.left;
  const left = Math.max(EDGE_PX, Math.min(anchor, window.innerWidth - MENU_WIDTH_PX - EDGE_PX));
  return r.bottom + MENU_GAP_PX + MENU_HEIGHT_EST_PX <= window.innerHeight
    ? { left, top: r.bottom + MENU_GAP_PX }
    : { left, bottom: window.innerHeight - r.top + MENU_GAP_PX };
}

/** Backend errors arrive as a UserError object ({ code, title, detail, ... }). */
function errorDetail(e: unknown): string | null {
  const detail = typeof e === 'object' && e !== null && 'detail' in e ? e.detail : e;
  return typeof detail === 'string' && detail ? detail : null;
}

type Props =
  | {
      mode: 'fix';
      item: EvidenceItem;
      /** The full finding: list items arrive with their evidence trimmed, which would drop versions and advisories. */
      resolveItem?: () => Promise<EvidenceItem>;
    }
  | { mode: 'brief'; item?: undefined; resolveItem?: undefined };

/**
 * Hand the finding (or the Brief) to the user's own coding agent: opens Claude
 * Code, VS Code or Cursor with a prompt pre-filled, which runs only when the user
 * presses Enter there, on their own plan. "Copy prompt" covers every other agent.
 * On Windows without Windows Terminal, Claude Code's link handler cannot open a
 * window, so the prompt is copied and an empty Claude Code opens to paste into.
 */
export const AgentHandoffButton = memo(function AgentHandoffButton({ mode, item, resolveItem }: Props) {
  const { t } = useTranslation();
  const [targets, setTargets] = useState<AgentHandoffTargets>(NONE);
  const [menuPos, setMenuPos] = useState<MenuPos | null>(null);
  const [copied, setCopied] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const moreRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const menuOpen = menuPos !== null;

  useEffect(() => {
    let live = true;
    void loadTargets().then(v => { if (live) setTargets(v); });
    return () => { live = false; };
  }, []);

  const toggleMenu = useCallback(() => {
    const r = moreRef.current?.getBoundingClientRect();
    setMenuPos(pos => (pos || !r ? null : placeMenu(r)));
  }, []);

  // A fixed menu cannot follow its button: close it on scroll, resize, a press outside, or Escape.
  useEffect(() => {
    if (!menuOpen) return;
    const close = () => setMenuPos(null);
    const onPress = (e: MouseEvent) => {
      const target = e.target as Node | null;
      if (target && (menuRef.current?.contains(target) || moreRef.current?.contains(target))) return;
      close();
    };
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') close(); };
    window.addEventListener('scroll', close, true);
    window.addEventListener('resize', close);
    document.addEventListener('mousedown', onPress);
    document.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('scroll', close, true);
      window.removeEventListener('resize', close);
      document.removeEventListener('mousedown', onPress);
      document.removeEventListener('keydown', onKey);
    };
  }, [menuOpen]);

  const eligible = mode === 'brief' || canBuildFixPrompt(item);
  const ready = ORDER.filter(target => targets[target] === 'ready');
  const primary = ready[0] ?? null;

  /** Prompt and folder from the full finding (falls back to the list copy if the fetch fails). */
  const resolve = useCallback(async (): Promise<{ prompt: string; cwd: string | null }> => {
    if (mode === 'brief') return { prompt: buildBriefPrompt(), cwd: null };
    const full = resolveItem ? await resolveItem().catch(() => item) : item;
    return { prompt: buildFixPrompt(full), cwd: extractFixFacts(full).project };
  }, [mode, item, resolveItem]);

  const open = useCallback(async (target: HandoffTarget) => {
    setMenuPos(null);
    setError(null);
    setNotice(null);
    const { prompt, cwd } = await resolve();
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
  }, [resolve, targets.claude_code_route, t]);

  const copy = useCallback(() => {
    setMenuPos(null);
    setError(null);
    void resolve()
      .then(({ prompt }) => navigator.clipboard.writeText(prompt))
      .then(() => {
        setCopied(true);
        setTimeout(() => setCopied(false), 1500);
      })
      .catch(() => setError(t('agentHandoff.copyFailed')));
  }, [resolve, t]);

  if (!eligible) return null;

  const label = (target: HandoffTarget) => t(`agentHandoff.${mode}.${target}`);
  const buttonClass =
    'px-3 py-1.5 text-[11px] rounded-md border border-border bg-bg-tertiary/60 text-text-secondary hover:text-text-primary hover:bg-bg-tertiary hover:border-text-primary/20 transition-colors';
  const itemClass =
    'block w-full text-start px-3 py-1.5 text-[11px] text-text-secondary hover:text-text-primary hover:bg-bg-tertiary';

  return (
    <div className="inline-flex flex-col items-start gap-1" data-testid="agent-handoff">
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
              ref={moreRef}
              type="button"
              className={buttonClass}
              aria-haspopup="menu"
              aria-expanded={menuOpen}
              aria-label={t('agentHandoff.more')}
              onClick={toggleMenu}
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
      {/* Portalled with fixed position: Preemption cards clip their overflow, which hid an absolutely positioned menu. */}
      {menuPos && primary && createPortal(
        <div
          ref={menuRef}
          role="menu"
          style={{ position: 'fixed', ...menuPos, width: MENU_WIDTH_PX }}
          className="z-50 rounded-md border border-border bg-bg-secondary shadow-lg py-1"
        >
          {ready.slice(1).map(target => (
            <button key={target} type="button" role="menuitem" className={itemClass} onClick={() => { void open(target); }}>
              {label(target)}
            </button>
          ))}
          <button type="button" role="menuitem" className={itemClass} onClick={copy} title={t('agentHandoff.copyTooltip')}>
            {t('agentHandoff.copy')}
          </button>
        </div>,
        document.body,
      )}
      {/* w-0 min-w-full: messages wrap to the buttons' width instead of widening the group and shoving the card's other actions aside. */}
      {targets.claude_code === 'needs_first_run' && (
        <p className="w-0 min-w-full text-[10px] text-text-muted">{t('agentHandoff.needsFirstRun')}</p>
      )}
      {notice && <p role="status" className="w-0 min-w-full text-[10px] text-text-secondary">{notice}</p>}
      {error && <p role="alert" className="w-0 min-w-full text-[10px] text-error">{error}</p>}
    </div>
  );
});
