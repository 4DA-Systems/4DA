// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { parseBriefingContent, parseInline, type ParsedSection } from '../../utils/briefing-parser';
import { openExternalUrl } from '../../lib/open-url';
import { RelativeTimestamp } from './BriefingHelpers';

interface NarratedBriefProps {
  content: string;
  model: string | null;
  lastGenerated: Date | null;
  loading: boolean;
  onRegenerate: () => void;
}

/** Section accent: Act now is the only red on the page. */
const SECTION_STYLE: Record<ParsedSection['type'], { border: string; title: string }> = {
  action: { border: 'border-red-500/40', title: 'text-red-400' },
  upgrades: { border: 'border-amber-500/30', title: 'text-amber-400' },
  worth_knowing: { border: 'border-border', title: 'text-text-primary' },
  still_open: { border: 'border-border', title: 'text-text-muted' },
  filtered: { border: 'border-border', title: 'text-text-muted' },
  general: { border: 'border-border', title: 'text-text-secondary' },
};

function InlineText({ line }: { line: string }) {
  return (
    <>
      {parseInline(line).map((seg, i) => {
        switch (seg.kind) {
          case 'bold':
            return <strong key={i} className="font-semibold text-text-primary">{seg.text}</strong>;
          case 'code':
            return <code key={i} className="font-mono text-[0.85em] px-1 rounded bg-bg-tertiary">{seg.text}</code>;
          case 'link':
            return (
              <a
                key={i}
                href={seg.url}
                onClick={(e) => { e.preventDefault(); openExternalUrl(seg.url); }}
                className="text-text-primary underline decoration-border underline-offset-2 hover:decoration-text-secondary"
              >
                {seg.text}
              </a>
            );
          default:
            return <span key={i}>{seg.text}</span>;
        }
      })}
    </>
  );
}

/** Render one section's lines: bullets (with one nesting level) and prose. */
function SectionBody({ lines }: { lines: string[] }) {
  const blocks = lines
    .map(l => l.replace(/\s+$/, ''))
    .filter(l => l.trim() !== '' && l.trim() !== '---');
  return (
    <ul className="space-y-1.5 text-sm leading-relaxed text-text-secondary">
      {blocks.map((raw, i) => {
        const nested = /^\s{2,}[-*]\s/.test(raw);
        const bullet = /^\s*[-*]\s/.test(raw);
        const text = raw.replace(/^\s*[-*]\s/, '');
        const footnote = /^_.*_$/.test(text.trim());
        if (footnote) {
          return (
            <li key={i} className="text-xs text-text-muted list-none pt-1">
              {text.trim().slice(1, -1)}
            </li>
          );
        }
        return (
          <li
            key={i}
            className={
              nested
                ? 'ms-5 list-[circle] text-text-muted'
                : bullet
                  ? 'ms-4 list-disc'
                  : 'list-none'
            }
          >
            <InlineText line={text} />
          </li>
        );
      })}
    </ul>
  );
}

/**
 * The narrated brief — the Brief tab's hero (Decision 2, 2026-10-02).
 *
 * Until now the brief Sonnet wrote was generated ~28 times a day and shown
 * nowhere on this tab (the March "3-zone" redesign cut the parsed sections);
 * the tab showed keyword-classified cards instead, including a false
 * "Critical" for an already-fixed hono. The brief is now built from computed
 * facts (`brief_facts`) and rendered here as written.
 */
export const NarratedBrief = memo(function NarratedBrief({
  content,
  model,
  lastGenerated,
  loading,
  onRegenerate,
}: NarratedBriefProps) {
  const { t } = useTranslation();
  const sections = useMemo(() => parseBriefingContent(content), [content]);
  const computed = model === 'deterministic';

  return (
    <article
      aria-label={t('briefing.narrated.ariaLabel')}
      className="bg-bg-secondary border border-border rounded-lg"
      data-testid="narrated-brief"
    >
      <header className="flex items-center gap-3 px-5 pt-4 pb-2">
        <h3 className="text-sm font-semibold text-text-primary">{t('briefing.narrated.title')}</h3>
        <span className="text-xs text-text-muted">
          {computed
            ? t('briefing.narrated.computed')
            : model
              ? t('briefing.viaModel', { model })
              : null}
        </span>
        <div className="ms-auto flex items-center gap-3">
          {lastGenerated && <RelativeTimestamp date={lastGenerated} />}
          <button
            type="button"
            onClick={onRegenerate}
            disabled={loading}
            className="text-xs text-text-muted hover:text-text-primary disabled:opacity-50 transition-colors"
            aria-label={t('briefing.refreshTooltip')}
          >
            {loading ? t('briefing.narrated.regenerating') : t('briefing.narrated.regenerate')}
          </button>
        </div>
      </header>
      <div className="px-5 pb-5 space-y-4">
        {sections.map((section, i) => {
          const style = SECTION_STYLE[section.type];
          const untitled = section.title === 'Overview';
          return (
            <section key={`${section.title}-${i}`} className={untitled ? '' : `border-s-2 ps-3 ${style.border}`}>
              {!untitled && (
                <h4 className={`text-xs font-semibold uppercase tracking-wider mb-1.5 ${style.title}`}>
                  {section.title}
                </h4>
              )}
              <SectionBody lines={section.lines} />
            </section>
          );
        })}
      </div>
    </article>
  );
});
