// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { memo, useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { cmd, type EmbeddingEngineStatusDto } from '../../lib/commands';

const MB = 1_048_576;
const POLL_MS = 1_000;
/** The pinned model's size (model_store::FILES), shown before any byte arrives. */
const MODEL_MB = 262;

/** What the row says, from the engine status. Exported for tests. */
export function modelStatusView(s: EmbeddingEngineStatusDto | null): {
  key: string;
  params?: Record<string, number>;
  tone: 'ok' | 'pending' | 'warn' | 'muted';
  action: 'download' | 'retry' | null;
} {
  if (!s) return { key: 'settings.ai.localModel.checking', tone: 'muted', action: null };
  switch (s.state) {
    case 'ready':
      return { key: 'settings.ai.localModel.ready', tone: 'ok', action: null };
    case 'idle':
      return { key: 'settings.ai.localModel.onDisk', tone: 'ok', action: null };
    case 'preparing':
      if (s.bytes_total > 0) {
        return {
          key: 'settings.ai.localModel.downloading',
          params: {
            percent: Math.min(100, Math.floor((s.bytes_downloaded / s.bytes_total) * 100)),
            done: Math.round(s.bytes_downloaded / MB),
            total: Math.round(s.bytes_total / MB),
          },
          tone: 'pending',
          action: null,
        };
      }
      return { key: 'settings.ai.localModel.loading', tone: 'pending', action: null };
    case 'failed':
      return { key: 'settings.ai.localModel.failed', tone: 'warn', action: 'retry' };
    case 'not_downloaded':
      return { key: 'settings.ai.localModel.notDownloaded', params: { size: MODEL_MB }, tone: 'muted', action: 'download' };
    default:
      return { key: 'settings.ai.localModel.unavailable', tone: 'muted', action: null };
  }
}

const DOT: Record<ReturnType<typeof modelStatusView>['tone'], string> = {
  ok: 'bg-green-400',
  pending: 'bg-amber-400',
  warn: 'bg-red-400',
  muted: 'bg-text-muted',
};

/**
 * The local search model (AD-054 decision 7): not in the installer, fetched
 * once when interests or natural-language search need it, or from here.
 * Shows not downloaded / downloading N% / ready, with the download button.
 */
export const LocalSearchModelStatus = memo(function LocalSearchModelStatus() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<EmbeddingEngineStatusDto | null>(null);
  const [requesting, setRequesting] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const alive = useRef(true);

  const poll = useCallback(async () => {
    try {
      const s = await cmd('get_embedding_engine_status');
      if (!alive.current || !s) return;
      setStatus(s);
      if (s.state === 'preparing') timer.current = setTimeout(() => void poll(), POLL_MS);
    } catch {
      // Unknown stays "checking": never a false "ready".
    }
  }, []);

  useEffect(() => {
    alive.current = true;
    void poll();
    return () => {
      alive.current = false;
      if (timer.current) clearTimeout(timer.current);
    };
  }, [poll]);

  const download = useCallback(async () => {
    setRequesting(true);
    try {
      const s = await cmd('download_embedding_model');
      if (alive.current && s) setStatus(s);
    } catch {
      // The status poll below reports what actually happened.
    } finally {
      if (alive.current) setRequesting(false);
      if (timer.current) clearTimeout(timer.current);
      void poll();
    }
  }, [poll]);

  const view = modelStatusView(status);
  return (
    <div className="px-3 py-2 bg-bg-secondary rounded-lg" data-testid="local-search-model">
      <div className="flex items-center justify-between gap-3">
        <span className="text-xs text-text-muted">{t('settings.ai.localModel.label')}</span>
        <span className="flex items-center gap-2">
          <span className={`w-1.5 h-1.5 rounded-full ${DOT[view.tone]}`} aria-hidden="true" />
          <span className="text-xs text-text-secondary" data-testid="local-search-model-state">
            {t(view.key, view.params)}
          </span>
          {view.action && (
            <button
              type="button"
              onClick={() => void download()}
              disabled={requesting}
              className="text-xs px-2 py-0.5 rounded border border-border text-text-primary hover:bg-bg-tertiary disabled:opacity-50"
              data-testid="local-search-model-download"
            >
              {t(view.action === 'retry' ? 'settings.ai.localModel.retry' : 'settings.ai.localModel.download')}
            </button>
          )}
        </span>
      </div>
      {status?.state === 'failed' && status.message && (
        <p className="mt-1 text-xs text-text-muted break-words" data-testid="local-search-model-error">
          {status.message}
        </p>
      )}
      {status?.state === 'not_downloaded' && (
        <p className="mt-1 text-xs text-text-muted leading-relaxed">{t('settings.ai.localModel.why')}</p>
      )}
    </div>
  );
});
