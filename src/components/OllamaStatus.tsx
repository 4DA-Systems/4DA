import { useState, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// Types
// ============================================================================

type OllamaConnectionStatus = 'warming' | 'ready' | 'error' | 'offline';

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
  label: string;
  animate: boolean;
}> = {
  warming: {
    dotClass: 'bg-orange-400',
    textClass: 'text-orange-400',
    label: 'Loading model...',
    animate: true,
  },
  ready: {
    dotClass: 'bg-green-400',
    textClass: 'text-green-400',
    label: 'Ollama',
    animate: false,
  },
  error: {
    dotClass: 'bg-red-400',
    textClass: 'text-red-400',
    label: 'Error',
    animate: false,
  },
  offline: {
    dotClass: 'bg-gray-400',
    textClass: 'text-gray-400',
    label: 'Offline',
    animate: false,
  },
};

// ============================================================================
// Component
// ============================================================================

export function OllamaStatus({ provider }: OllamaStatusProps) {
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
  const isClickable = status === 'error' || status === 'offline';

  const handleRetry = async () => {
    try {
      await invoke('check_ollama_status');
      await invoke('warm_ollama_model');
    } catch (err) {
      console.debug('Ollama retry failed:', err);
    }
  };

  return (
    <button
      type="button"
      onClick={isClickable ? handleRetry : undefined}
      disabled={!isClickable}
      className={`
        inline-flex items-center gap-2 px-3 py-1.5 rounded-lg border
        bg-[#141414] border-[#2A2A2A]
        text-xs select-none transition-colors
        ${isClickable ? 'cursor-pointer hover:border-[#3A3A3A]' : 'cursor-default'}
      `}
      title={errorMsg ?? config.label}
    >
      <span
        className={`
          w-2 h-2 rounded-full flex-shrink-0
          ${config.dotClass}
          ${config.animate ? 'animate-pulse' : ''}
        `}
      />
      <span className={config.textClass}>
        {config.label}
      </span>
    </button>
  );
}
