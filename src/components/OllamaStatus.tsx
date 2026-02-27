import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// Types
// ============================================================================

type OllamaConnectionStatus = 'pulling' | 'warming' | 'ready' | 'error' | 'offline';

interface OllamaStatusPayload {
  phase: string;
  model: string;
  error?: string;
}

interface OllamaStatusProps {
  provider: string;
}

// ============================================================================
// Phase-to-status mapping
// ============================================================================

function mapPhaseToStatus(phase: string): OllamaConnectionStatus {
  switch (phase) {
    case 'pulling':
      return 'pulling';
    case 'warming':
      return 'warming';
    case 'ready':
      return 'ready';
    case 'error':
      return 'error';
    default:
      return 'offline';
  }
}

// ============================================================================
// Status display config
// ============================================================================

const STATUS_CONFIG: Record<OllamaConnectionStatus, {
  dotClass: string;
  textClass: string;
  labelKey: string;
  animate: boolean;
}> = {
  pulling: {
    dotClass: 'bg-blue-400',
    textClass: 'text-blue-400',
    labelKey: 'ollama.pulling',
    animate: true,
  },
  warming: {
    dotClass: 'bg-orange-400',
    textClass: 'text-orange-400',
    labelKey: 'ollama.loading',
    animate: true,
  },
  ready: {
    dotClass: 'bg-green-400',
    textClass: 'text-green-400',
    labelKey: 'ollama.ready',
    animate: false,
  },
  error: {
    dotClass: 'bg-red-400',
    textClass: 'text-red-400',
    labelKey: 'ollama.error',
    animate: false,
  },
  offline: {
    dotClass: 'bg-gray-500',
    textClass: 'text-gray-500',
    labelKey: 'ollama.basicMode',
    animate: false,
  },
};

// ============================================================================
// Component
// ============================================================================

export function OllamaStatus({ provider }: OllamaStatusProps) {
  const { t } = useTranslation();
  const [status, setStatus] = useState<OllamaConnectionStatus>('offline');
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  useEffect(() => {
    if (provider !== 'ollama') return;

    const unlisten = listen<OllamaStatusPayload>('ollama-status', (event) => {
      const { phase, error } = event.payload;
      setStatus(mapPhaseToStatus(phase));
      setErrorMsg(error ?? null);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [provider]);

  if (provider !== 'ollama') return null;

  const config = STATUS_CONFIG[status];
  const label = t(config.labelKey);
  const isClickable = status === 'error' || status === 'offline';

  const handleRetry = async () => {
    setStatus('warming');
    setErrorMsg(null);
    try {
      await invoke('check_ollama_status', { baseUrl: null });
      // The backend ensure_models_available will handle pulling + warming
      // via ollama-status events, so we just need to trigger a re-check
    } catch (err) {
      console.debug('Ollama retry failed:', err);
    }
  };

  return (
    <button
      type="button"
      onClick={isClickable ? handleRetry : undefined}
      disabled={!isClickable}
      aria-label={`${t('ollama.status')}: ${label}${isClickable ? `. ${t('ollama.clickRetry')}` : ''}`}
      className={`
        inline-flex items-center gap-2 px-3 py-1.5 rounded-lg border
        bg-bg-secondary border-border
        text-xs select-none transition-colors
        ${isClickable ? 'cursor-pointer hover:border-[#3A3A3A]' : 'cursor-default'}
      `}
      title={errorMsg ?? label}
    >
      <span
        className={`
          w-2 h-2 rounded-full flex-shrink-0
          ${config.dotClass}
          ${config.animate ? 'animate-pulse' : ''}
        `}
      />
      <span className={config.textClass}>
        {label}
      </span>
    </button>
  );
}
