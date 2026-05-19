// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { listen } from '@tauri-apps/api/event';
import { cmd } from '../../lib/commands';

interface DownloadProgress {
  stage: string;
  percent: number;
  bytes_downloaded: number;
  bytes_total: number;
  message: string;
  done: boolean;
}

export function EmbeddingSetupProgress() {
  const { t } = useTranslation();
  const [progress, setProgress] = useState<DownloadProgress | null>(null);
  const [preparing, setPreparing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const unlisten = listen<DownloadProgress>('embedding-setup-progress', (event) => {
      setProgress(event.payload);
      if (event.payload.done && event.payload.stage === 'error') {
        setError(event.payload.message);
        setPreparing(false);
      } else if (event.payload.done) {
        setPreparing(false);
      }
    });
    return () => { void unlisten.then((fn) => fn()); };
  }, []);

  const handlePrepare = useCallback(async () => {
    setPreparing(true);
    setError(null);
    setProgress(null);
    try {
      await cmd('prepare_embedding_engine');
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setPreparing(false);
    }
  }, []);

  if (progress?.done && progress.stage === 'ready') {
    return (
      <div className="flex items-center gap-2 text-xs text-[var(--success)]">
        <span>&#10003;</span>
        <span>{t('settings.ai.embeddingSetup.ready')}</span>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-2">
      {!preparing && !progress && (
        <button
          onClick={handlePrepare}
          className="text-xs text-[var(--text-secondary)] hover:text-[var(--text-primary)] underline cursor-pointer"
        >
          {t('settings.ai.embeddingSetup.prepare')}
        </button>
      )}
      {preparing && progress && (
        <div className="flex flex-col gap-1">
          <div className="text-xs text-[var(--text-secondary)]">
            {progress.message}
          </div>
          <div className="h-1 w-full rounded-full bg-[var(--bg-tertiary)] overflow-hidden">
            <div
              className="h-full rounded-full bg-[var(--accent-primary)] transition-all duration-300"
              style={{ width: `${progress.percent}%` }}
            />
          </div>
        </div>
      )}
      {preparing && !progress && (
        <div className="text-xs text-[var(--text-muted)] animate-pulse">
          {t('settings.ai.embeddingSetup.checking')}
        </div>
      )}
      {error && (
        <div className="text-xs text-[var(--error)]">
          {t('settings.ai.embeddingSetup.error', { error })}
        </div>
      )}
    </div>
  );
}
