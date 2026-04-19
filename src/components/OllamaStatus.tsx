// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { listen } from '@tauri-apps/api/event';
import { cmd } from '../lib/commands';
import { registerFourdaComponent } from '../lib/fourda-components';

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
    textClass: 'text-text-muted',
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
  const [showHint, setShowHint] = useState(false);
  const hintRef = useRef<HTMLDivElement>(null);

  useEffect(() => { registerFourdaComponent('fourda-status-orb'); }, []);

  // Close hint on outside click
  useEffect(() => {
    if (!showHint) return;
    const handler = (e: MouseEvent) => {
      if (hintRef.current && !hintRef.current.contains(e.target as globalThis.Node)) {
        setShowHint(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [showHint]);

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
      await cmd('check_ollama_status', { baseUrl: null });
      // The backend ensure_models_available will handle pulling + warming
      // via ollama-status events, so we just need to trigger a re-check
    } catch {
      /* retry failed — status events will update UI */
    }
  };

  return (
    <div className="relative inline-flex items-center gap-1" ref={hintRef}>
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
        {config.animate ? (
          <fourda-status-orb
            style={{ width: '10px', height: '10px', flexShrink: 0 }}
            aria-hidden="true"
            ref={(el: HTMLElement | null) => {
              if (el && 'health' in el) (el as HTMLElement & { health: number }).health = 1.0;
            }}
          />
        ) : (
          <span
            className={`w-2 h-2 rounded-full flex-shrink-0 ${config.dotClass}`}
          />
        )}
        <span className={config.textClass}>
          {label}
        </span>
      </button>
      {(status === 'offline' || status === 'error') && (
        <button
          type="button"
          onClick={() => setShowHint(!showHint)}
          className="w-4 h-4 rounded-full bg-bg-tertiary text-text-muted text-[10px] flex items-center justify-center hover:text-text-secondary transition-colors"
          aria-label={t('ollama.setupHelp')}
        >
          ?
        </button>
      )}
      {showHint && (
        <div className="absolute top-full mt-2 start-0 w-64 bg-bg-secondary border border-border rounded-lg p-3 shadow-lg z-50">
          <p className="text-[11px] text-text-secondary mb-2">
            {t('ollama.hintFreeLocal')}
          </p>
          <div className="space-y-1.5 text-[10px] text-text-muted">
            <p>1. {t('ollama.hintInstall')}</p>
            <p>2. {t('ollama.hintServe')}</p>
            <p>3. {t('ollama.hintRetry')}</p>
          </div>
        </div>
      )}
    </div>
  );
}
