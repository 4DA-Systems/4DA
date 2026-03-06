import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';
import { useShallow } from 'zustand/react/shallow';
import { renderMarkdown } from '../../utils/playbook-markdown';
import { ProvenanceTooltip } from './ProvenanceTooltip';
import { ChannelChangelog } from './ChannelChangelog';
import type { RenderProvenance } from '../../types/channels';

export function ChannelContent() {
  const { t } = useTranslation();
  const {
    activeRender,
    activeProvenance,
    activeChangelog,
    renderLoading,
    renderError,
    activeChannelId,
    channels,
  } = useAppStore(
    useShallow((s) => ({
      activeRender: s.activeRender,
      activeProvenance: s.activeProvenance,
      activeChangelog: s.activeChangelog,
      renderLoading: s.renderLoading,
      renderError: s.renderError,
      activeChannelId: s.activeChannelId,
      channels: s.channels,
    })),
  );
  const renderChannel = useAppStore((s) => s.renderChannel);

  const activeChannel = channels.find((c) => c.id === activeChannelId);

  // Empty state: no channel selected
  if (!activeChannelId) {
    return (
      <div className="flex items-center justify-center h-64 text-text-muted text-sm">
        {t('channels.selectChannel')}
      </div>
    );
  }

  // Loading skeleton
  if (renderLoading) {
    return (
      <div className="space-y-4 animate-pulse">
        <div className="h-8 bg-bg-tertiary rounded w-1/3" />
        <div className="h-4 bg-bg-tertiary rounded w-full" />
        <div className="h-4 bg-bg-tertiary rounded w-5/6" />
        <div className="h-4 bg-bg-tertiary rounded w-4/6" />
        <div className="h-4 bg-bg-tertiary rounded w-full" />
        <div className="h-4 bg-bg-tertiary rounded w-3/4" />
      </div>
    );
  }

  // Error state
  if (renderError) {
    return (
      <div className="p-4 bg-red-500/10 border border-red-500/30 rounded-lg">
        <p className="text-sm text-red-400">{renderError}</p>
      </div>
    );
  }

  // Never-rendered state with CTA
  if (!activeRender) {
    return (
      <div className="text-center py-12">
        <p className="text-text-muted text-sm mb-4">
          {t('channels.neverRendered')}
        </p>
        <button
          onClick={() => activeChannelId && renderChannel(activeChannelId)}
          className="px-4 py-2 text-sm bg-cyan-500/20 text-cyan-400 border border-cyan-500/30 rounded-lg hover:bg-cyan-500/30 transition-colors"
        >
          {t('channels.renderNow')}
        </button>
      </div>
    );
  }

  return (
    <div>
      {/* Header with title, version badge, model, and re-render button */}
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-3">
          <h2 className="text-lg font-semibold text-white">
            {activeChannel?.title}
          </h2>
          <span className="px-2 py-0.5 text-[10px] font-mono text-cyan-400 bg-cyan-500/10 rounded">
            v{activeRender.version}
          </span>
          {activeRender.model && (
            <span className="text-[10px] text-text-muted">
              {activeRender.model}
            </span>
          )}
          {!activeRender.model && (
            <span className="text-xs px-2 py-0.5 bg-amber-500/10 text-amber-400 rounded">
              {t('channels.basicMode')} — {t('channels.configureOllama')}
            </span>
          )}
        </div>
        <button
          onClick={() => activeChannelId && renderChannel(activeChannelId)}
          disabled={renderLoading}
          className="px-3 py-1.5 text-xs bg-cyan-500/20 text-cyan-400 border border-cyan-500/30 rounded-lg hover:bg-cyan-500/30 transition-colors disabled:opacity-50"
        >
          {renderLoading ? t('channels.rendering') : t('channels.rerender')}
        </button>
      </div>

      {/* Render timestamp */}
      <p className="text-[10px] text-text-muted mb-4">
        {t('channels.lastRendered')}: {formatDateTime(activeRender.rendered_at)}
      </p>

      {/* Content with provenance badges */}
      <ChannelContentBody
        markdown={activeRender.content_markdown}
        provenance={activeProvenance}
      />

      {/* Render stats footer */}
      <div className="mt-6 pt-4 border-t border-border flex items-center gap-4 text-[10px] text-text-muted">
        <span>
          {activeRender.source_item_ids.length} {t('channels.sources')}
        </span>
        {activeRender.tokens_used && (
          <span>{activeRender.tokens_used} tokens</span>
        )}
        {activeRender.latency_ms && (
          <span>{activeRender.latency_ms}ms</span>
        )}
      </div>

      {/* Changelog diff between versions */}
      {activeChangelog && <ChannelChangelog changelog={activeChangelog} />}
    </div>
  );
}

/**
 * Sub-component: renders markdown content and injects provenance tooltip
 * badges for [S1], [S2], etc. source markers embedded in the prose.
 */
function ChannelContentBody({
  markdown,
  provenance,
}: {
  markdown: string;
  provenance: RenderProvenance[];
}) {
  const provenanceMap = useMemo(() => {
    const map = new Map<number, RenderProvenance>();
    for (const p of provenance) {
      if (!map.has(p.claim_index)) {
        map.set(p.claim_index, p);
      }
    }
    return map;
  }, [provenance]);

  // Check if the markdown contains any source markers
  const hasMarkers = /\[S\d+\]/.test(markdown);

  // If there are provenance entries and markers, render with tooltips
  const processedContent = useMemo(() => {
    if (!hasMarkers || provenance.length === 0) return null;

    const markerRegex = /\[S(\d+)\]/g;

    // Split the markdown by source markers and interleave tooltips
    const segments: React.ReactNode[] = [];
    let lastIndex = 0;
    let key = 0;
    let match: RegExpExecArray | null;

    while ((match = markerRegex.exec(markdown)) !== null) {
      // Render the markdown chunk before this marker
      const before = markdown.slice(lastIndex, match.index);
      if (before) {
        const rendered = renderMarkdown(before);
        segments.push(
          <span key={`md-${key++}`}>{rendered}</span>,
        );
      }

      // Render the provenance badge
      const sourceNum = parseInt(match[1], 10);
      const prov = provenanceMap.get(sourceNum);

      if (prov) {
        segments.push(
          <ProvenanceTooltip key={`prov-${key++}`} provenance={prov}>
            S{sourceNum}
          </ProvenanceTooltip>,
        );
      } else {
        segments.push(
          <span
            key={`badge-${key++}`}
            className="bg-cyan-500/20 text-cyan-400 text-xs rounded px-1 font-mono"
          >
            S{sourceNum}
          </span>,
        );
      }

      lastIndex = match.index + match[0].length;
    }

    // Render remaining markdown after the last marker
    const remaining = markdown.slice(lastIndex);
    if (remaining) {
      const rendered = renderMarkdown(remaining);
      segments.push(
        <span key={`md-${key++}`}>{rendered}</span>,
      );
    }

    return segments.length > 0 ? segments : null;
  }, [markdown, provenance, provenanceMap, hasMarkers]);

  // Fallback: render plain markdown without provenance injection
  const plainRendered = useMemo(() => {
    if (processedContent) return null;
    return renderMarkdown(markdown);
  }, [markdown, processedContent]);

  return (
    <div
      className="prose prose-invert prose-sm max-w-none
        prose-headings:text-white prose-headings:font-semibold
        prose-h2:text-base prose-h2:mt-6 prose-h2:mb-3
        prose-p:text-text-secondary prose-p:leading-relaxed
        prose-li:text-text-secondary
        prose-strong:text-white
        prose-a:text-cyan-400 prose-a:no-underline hover:prose-a:underline"
    >
      {processedContent || plainRendered}
    </div>
  );
}

function formatDateTime(dateStr: string): string {
  const date = new Date(dateStr + 'Z');
  return date.toLocaleString(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}
