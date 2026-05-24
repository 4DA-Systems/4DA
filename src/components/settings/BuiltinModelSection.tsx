// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';

interface ModelInfo {
  id: string;
  display_name: string;
  family: string;
  size_bytes: number;
  size_gb: number;
  min_ram_gb: number;
  quantization: string;
  downloaded: boolean;
  path: string | null;
  fits_ram: boolean;
}

interface DownloadProgress {
  model_id: string;
  downloaded_bytes: number;
  total_bytes: number;
  percent: number;
  status: 'downloading' | 'verifying' | 'complete' | 'failed' | 'cancelled';
}

interface SidecarStatus {
  status: 'stopped' | 'starting' | 'ready' | 'error';
  port: number | null;
}

export function BuiltinModelSection() {
  const { t } = useTranslation();
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [recommendedId, setRecommendedId] = useState<string | null>(null);
  const [ramTotal, setRamTotal] = useState(0);
  const [sidecar, setSidecar] = useState<SidecarStatus>({ status: 'stopped', port: null });
  const [downloading, setDownloading] = useState<string | null>(null);
  const [progress, setProgress] = useState<DownloadProgress | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadModels = useCallback(async () => {
    try {
      const result = await cmd('list_builtin_models');
      setModels(result.models);
      setRecommendedId(result.recommended_id);
      setRamTotal(result.ram_total_gb);
    } catch {
      // silently handle — models list will remain empty
    }
  }, []);

  const loadSidecarStatus = useCallback(async () => {
    try {
      const result = await cmd('get_builtin_llm_status');
      setSidecar(result as SidecarStatus);
    } catch {
      // silently handle
    }
  }, []);

  useEffect(() => {
    void loadModels();
    void loadSidecarStatus();
  }, [loadModels, loadSidecarStatus]);

  // Listen for download progress events
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void import('@tauri-apps/api/event').then(({ listen }) => {
      void listen<DownloadProgress>('model-download-progress', (event) => {
        setProgress(event.payload);
        if (event.payload.status === 'complete') {
          setDownloading(null);
          void loadModels();
          void loadSidecarStatus();
        } else if (event.payload.status === 'failed' || event.payload.status === 'cancelled') {
          setDownloading(null);
          if (event.payload.status === 'failed') {
            setError('Download failed. Check your network connection and try again.');
          }
        }
      }).then(fn => { unlisten = fn; });
    });
    return () => unlisten?.();
  }, [loadModels, loadSidecarStatus]);

  const handleDownload = async (modelId: string) => {
    setError(null);
    setDownloading(modelId);
    setProgress(null);
    try {
      await cmd('download_builtin_model', { model_id: modelId });
    } catch (e) {
      setDownloading(null);
      setError(String(e));
    }
  };

  const handleCancel = async () => {
    try {
      await cmd('cancel_builtin_model_download');
    } catch {
      // ignore
    }
  };

  const handleDelete = async (modelId: string) => {
    try {
      await cmd('delete_builtin_model', { model_id: modelId });
      void loadModels();
    } catch (e) {
      setError(String(e));
    }
  };

  const handleStartSidecar = async (modelPath: string) => {
    setError(null);
    setSidecar({ status: 'starting', port: null });
    try {
      const result = await cmd('start_builtin_llm', { model_path: modelPath });
      setSidecar({ status: result.status as SidecarStatus['status'], port: result.port });
    } catch (e) {
      setSidecar({ status: 'error', port: null });
      setError(String(e));
    }
  };

  const handleStopSidecar = async () => {
    try {
      await cmd('stop_builtin_llm');
      setSidecar({ status: 'stopped', port: null });
    } catch (e) {
      setError(String(e));
    }
  };

  const statusColor = {
    stopped: 'text-text-muted',
    starting: 'text-amber-400',
    ready: 'text-green-400',
    error: 'text-red-400',
  }[sidecar.status];

  const statusLabel = {
    stopped: t('settings.ai.sidecarStopped', 'Stopped'),
    starting: t('settings.ai.sidecarStarting', 'Starting...'),
    ready: t('settings.ai.sidecarReady', 'Running'),
    error: t('settings.ai.sidecarError', 'Error'),
  }[sidecar.status];

  return (
    <div className="space-y-3">
      {/* Sidecar status bar */}
      <div className="flex items-center justify-between bg-bg-secondary rounded-lg p-3 border border-border">
        <div className="flex items-center gap-2">
          <span className={`w-2 h-2 rounded-full ${sidecar.status === 'ready' ? 'bg-green-400' : sidecar.status === 'starting' ? 'bg-amber-400 animate-pulse' : sidecar.status === 'error' ? 'bg-red-400' : 'bg-text-muted'}`} />
          <span className={`text-xs font-medium ${statusColor}`}>{statusLabel}</span>
          {sidecar.port && (
            <span className="text-xs text-text-muted">:{sidecar.port}</span>
          )}
        </div>
        {sidecar.status === 'ready' && (
          <button
            type="button"
            onClick={() => void handleStopSidecar()}
            className="text-xs px-2.5 py-1 bg-red-500/10 text-red-400 rounded hover:bg-red-500/20 transition-colors"
          >
            {t('settings.ai.stopLLM', 'Stop')}
          </button>
        )}
      </div>

      {/* Error banner */}
      {error && (
        <div className="p-2.5 bg-red-900/15 border border-red-500/30 rounded-lg">
          <p className="text-xs text-red-400">{error}</p>
          <button
            type="button"
            onClick={() => setError(null)}
            className="text-xs text-red-300 underline mt-1"
          >
            {t('action.dismiss', 'Dismiss')}
          </button>
        </div>
      )}

      {/* RAM info */}
      {ramTotal > 0 && (
        <p className="text-xs text-text-muted">
          {t('settings.ai.systemRam', 'System RAM')}: {ramTotal} GB
        </p>
      )}

      {/* Model catalog */}
      <div className="space-y-2">
        {models.map((model) => {
          const isDownloading = downloading === model.id;
          const isRecommended = model.id === recommendedId;
          const canStart = model.downloaded && sidecar.status !== 'ready' && sidecar.status !== 'starting';

          return (
            <div
              key={model.id}
              className={`rounded-lg p-3 border transition-colors ${
                isRecommended
                  ? 'bg-green-900/10 border-green-500/30'
                  : model.fits_ram
                    ? 'bg-bg-secondary border-border'
                    : 'bg-bg-secondary border-border opacity-60'
              }`}
            >
              <div className="flex items-start justify-between gap-2">
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="text-sm text-white font-medium">{model.display_name}</span>
                    {isRecommended && (
                      <span className="text-[10px] px-1.5 py-0.5 bg-green-500/20 text-green-400 rounded font-medium">
                        {t('settings.ai.recommended', 'Recommended')}
                      </span>
                    )}
                  </div>
                  <p className="text-xs text-text-muted mt-0.5">
                    {model.size_gb} GB &middot; {model.quantization} &middot; {t('settings.ai.minRam', 'Min RAM')}: {model.min_ram_gb} GB
                  </p>
                </div>

                <div className="flex items-center gap-1.5 flex-shrink-0">
                  {model.downloaded && canStart && (
                    <button
                      type="button"
                      onClick={() => model.path && void handleStartSidecar(model.path)}
                      className="text-xs px-2.5 py-1 bg-green-500/20 text-green-300 rounded hover:bg-green-500/30 transition-colors font-medium"
                    >
                      {t('settings.ai.startLLM', 'Start')}
                    </button>
                  )}
                  {model.downloaded && !canStart && sidecar.status === 'ready' && (
                    <span className="text-xs text-green-400">{t('settings.ai.active', 'Active')}</span>
                  )}
                  {!model.downloaded && !isDownloading && model.fits_ram && (
                    <button
                      type="button"
                      onClick={() => void handleDownload(model.id)}
                      disabled={downloading !== null}
                      className="text-xs px-2.5 py-1 bg-white/10 text-white rounded hover:bg-white/15 transition-colors font-medium disabled:opacity-40"
                    >
                      {t('settings.ai.download', 'Download')}
                    </button>
                  )}
                  {!model.fits_ram && !model.downloaded && (
                    <span className="text-xs text-text-muted">{t('settings.ai.needsMoreRam', 'Needs more RAM')}</span>
                  )}
                  {model.downloaded && (
                    <button
                      type="button"
                      onClick={() => void handleDelete(model.id)}
                      className="text-xs px-1.5 py-1 text-text-muted hover:text-red-400 transition-colors"
                      title={t('action.delete', 'Delete')}
                    >
                      &times;
                    </button>
                  )}
                </div>
              </div>

              {/* Download progress */}
              {isDownloading && progress && (
                <div className="mt-2">
                  <div className="flex items-center justify-between text-xs text-text-muted mb-1">
                    <span>
                      {progress.status === 'verifying'
                        ? t('settings.ai.verifying', 'Verifying...')
                        : `${Math.round(progress.percent)}%`}
                    </span>
                    <button
                      type="button"
                      onClick={() => void handleCancel()}
                      className="text-red-400 hover:text-red-300"
                    >
                      {t('action.cancel', 'Cancel')}
                    </button>
                  </div>
                  <div className="w-full h-1.5 bg-bg-tertiary rounded-full overflow-hidden">
                    <div
                      className="h-full bg-white/70 rounded-full transition-all duration-300"
                      style={{ width: `${progress.percent}%` }}
                    />
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
