// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';

/** `get_embedding_engine_status` (mirrors `EmbeddingEngineStatus` in Rust). */
export interface EngineStatus {
  state: 'idle' | 'preparing' | 'ready' | 'failed' | 'unavailable';
  message: string | null;
  bytes_downloaded: number;
  bytes_total: number;
}
type EmbeddingMode = 'semantic' | 'keyword-only' | null;

export interface SemanticSummary {
  tone: 'ok' | 'pending' | 'warn';
  key: string;
  params?: Record<string, number>;
}

const MB = 1_048_576;
const POLL_MS = 2_000;

/**
 * What the onboarding summary may say about semantic search. "Active" only
 * when it is: the in-process engine is ready, or an embedding call already
 * came back semantic (Ollama serves embeddings before the in-process engine).
 * Fresh-profile E2E 2026-10-09: the line said "active ✓" while the model was
 * still downloading.
 */
export function semanticSummary(engine: EngineStatus | null, mode: EmbeddingMode): SemanticSummary {
  if (mode === 'keyword-only') return { tone: 'ok', key: 'calibration.onboarding.summaryKeyword' };
  if (mode === 'semantic' || engine?.state === 'ready') {
    return { tone: 'ok', key: 'calibration.onboarding.summaryAI' };
  }
  if (engine?.state === 'failed') return { tone: 'warn', key: 'calibration.onboarding.summaryFailed' };
  if (engine?.state === 'preparing' && engine.bytes_total > 0) {
    return {
      tone: 'pending',
      key: 'calibration.onboarding.summaryDownloading',
      params: {
        done: Math.round(engine.bytes_downloaded / MB),
        total: Math.round(engine.bytes_total / MB),
      },
    };
  }
  return { tone: 'pending', key: 'calibration.onboarding.summaryStarting' };
}

/** Poll the engine's status until it settles (ready, failed or absent). */
function useEngineStatus(): EngineStatus | null {
  const [status, setStatus] = useState<EngineStatus | null>(null);
  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const poll = async () => {
      try {
        const s = await cmd('get_embedding_engine_status');
        if (cancelled || !s) return;
        setStatus(s);
        if (s.state === 'idle' || s.state === 'preparing') timer = setTimeout(() => void poll(), POLL_MS);
      } catch {
        // Unknown stays "starting": never a false "active".
      }
    };
    void poll();
    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
    };
  }, []);
  return status;
}

const ICON: Record<SemanticSummary['tone'], { glyph: string; color: string }> = {
  ok: { glyph: '✓', color: 'var(--color-success)' },
  pending: { glyph: '…', color: 'var(--color-text-muted)' },
  warn: { glyph: '!', color: 'var(--color-accent-gold)' },
};

export const SemanticSearchSummaryLine = memo(function SemanticSearchSummaryLine({
  embeddingMode,
}: {
  embeddingMode: EmbeddingMode;
}) {
  const { t } = useTranslation();
  const engine = useEngineStatus();
  const summary = semanticSummary(engine, embeddingMode);
  const icon = ICON[summary.tone];
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 6 }} data-testid="semantic-search-summary">
      <span style={{ color: icon.color, fontSize: 12 }} aria-hidden="true">
        {icon.glyph}
      </span>
      <span style={{ fontSize: 11, color: 'var(--color-text-secondary)' }}>{t(summary.key, summary.params)}</span>
    </div>
  );
});
