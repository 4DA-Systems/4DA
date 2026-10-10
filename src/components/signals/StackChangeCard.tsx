// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//
// One stack change: what changed (badge + version strip), why it concerns
// the user (explanation, projects) and what to run. Commands are copied, never
// executed — 4DA names the command, the user runs it.

import { memo, useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';
import { formatProjectNames } from '../../utils/project-path';
import { useTranslatedContent } from '../ContentTranslationProvider';
import { commandsOf, primaryLink, stackChangeOf, versionLine, type StackChange } from './stack-change';

const BADGE: Record<StackChange, { key: string; cls: string }> = {
  security: { key: 'signals.stack.change.security', cls: 'text-red-400 bg-red-500/10 border-red-500/25' },
  yanked: { key: 'signals.stack.change.yanked', cls: 'text-orange-400 bg-orange-500/10 border-orange-500/25' },
  major: { key: 'signals.stack.change.major', cls: 'text-amber-400 bg-amber-500/10 border-amber-500/25' },
  breaking: { key: 'signals.stack.change.breaking', cls: 'text-amber-400 bg-amber-500/10 border-amber-500/25' },
  minor: { key: 'signals.stack.change.minor', cls: 'text-blue-400 bg-blue-500/10 border-blue-500/20' },
};

const EXPLANATION_CLAMP = 260;

function openExternal(url: string) {
  import('@tauri-apps/plugin-opener')
    .then(({ openUrl }) => openUrl(url))
    .catch(() => window.open(url, '_blank', 'noopener,noreferrer'));
}

const CommandRow = memo(function CommandRow({ command, where }: { command: string; where: string }) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);
  const copy = useCallback(() => {
    void navigator.clipboard.writeText(command).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    }).catch(() => { /* clipboard unavailable: the command stays selectable */ });
  }, [command]);
  return (
    <div className="flex items-center gap-2 mt-2">
      <code className="flex-1 min-w-0 truncate px-2 py-1 rounded bg-black/30 border border-border text-[11px] font-mono text-text-primary select-all" title={command}>
        {command}
      </code>
      <span className="hidden sm:inline shrink-0 text-[10px] text-text-muted">{t('signals.stack.runIn', { where })}</span>
      <button
        type="button"
        onClick={copy}
        aria-label={t('signals.stack.copyAria', { command })}
        className="shrink-0 px-2 py-1 text-[10px] rounded border border-border text-text-secondary hover:text-text-primary hover:bg-bg-tertiary transition-colors"
      >
        {copied ? t('signals.stack.copied') : t('signals.stack.copy')}
      </button>
    </div>
  );
});

export const StackChangeCard = memo(function StackChangeCard({ item }: { item: EvidenceItem }) {
  const { t } = useTranslation();
  const { getTranslated, requestTranslation } = useTranslatedContent();
  const [expanded, setExpanded] = useState(false);
  const change = stackChangeOf(item);
  const badge = change ? BADGE[change] : null;
  const version = versionLine(item);
  const commands = commandsOf(item);
  const link = primaryLink(item);
  const projects = formatProjectNames(item.affected_projects);

  useEffect(() => {
    requestTranslation([
      { id: item.id, text: item.title },
      { id: `${item.id}:expl`, text: item.explanation },
    ]);
  }, [item.id, item.title, item.explanation, requestTranslation]);

  const title = getTranslated(item.id, item.title);
  const explanation = getTranslated(`${item.id}:expl`, item.explanation);
  const clamped = explanation.length > EXPLANATION_CLAMP && !expanded;

  return (
    <article className="rounded-lg border border-border bg-bg-primary/40 px-4 py-3" data-stack-change={change ?? undefined}>
      <header className="flex items-start gap-2">
        {badge && (
          <span className={`shrink-0 text-[10px] font-semibold uppercase tracking-wider px-1.5 py-0.5 rounded border ${badge.cls}`}>
            {t(badge.key)}
          </span>
        )}
        <h4 className="flex-1 min-w-0 text-[13px] font-medium text-text-primary leading-snug">{title}</h4>
        {link && (
          <button
            type="button"
            onClick={() => openExternal(link)}
            className="shrink-0 text-[10px] text-text-muted hover:text-text-secondary underline-offset-2 hover:underline"
          >
            {t('signals.stack.open')}
          </button>
        )}
      </header>
      {version && (
        <div className="mt-2 inline-flex items-center gap-2 px-2 py-1 rounded bg-black/20 border border-border text-[11px] font-mono text-text-secondary">
          {version.ecosystem && <span className="text-text-muted">{version.ecosystem}</span>}
          <span>{version.span}</span>
        </div>
      )}
      {explanation && (
        <p className="mt-2 text-xs text-text-secondary leading-relaxed">
          {clamped ? `${explanation.slice(0, EXPLANATION_CLAMP).trimEnd()}…` : explanation}
          {explanation.length > EXPLANATION_CLAMP && (
            <button
              type="button"
              onClick={() => setExpanded(!expanded)}
              aria-expanded={expanded}
              className="ms-1 text-text-muted hover:text-text-secondary underline-offset-2 hover:underline"
            >
              {expanded ? t('preemption.explanation.collapse', 'less') : t('preemption.explanation.expand', 'more')}
            </button>
          )}
        </p>
      )}
      {projects.length > 0 && (
        <div className="mt-2 flex items-baseline gap-2 flex-wrap text-[10px]">
          <span className="font-medium text-text-muted uppercase tracking-wider">{t('signals.stack.projects')}</span>
          {projects.slice(0, 4).map((name) => (
            <span key={name} className="px-1.5 py-0.5 rounded font-mono bg-bg-tertiary text-text-secondary border border-border">{name}</span>
          ))}
          {projects.length > 4 && <span className="text-text-muted">+{projects.length - 4}</span>}
        </div>
      )}
      {commands.map((c) => <CommandRow key={c.command} command={c.command} where={c.where} />)}
    </article>
  );
});
