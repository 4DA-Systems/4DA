import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { ProGate } from './ProGate';

interface ContextHandoffProps {
  onStatus: (msg: string) => void;
}

export function ContextHandoff({ onStatus }: ContextHandoffProps) {
  const { t } = useTranslation();
  const [exporting, setExporting] = useState(false);

  const exportContext = async () => {
    setExporting(true);
    try {
      const packet = await invoke<object>('generate_context_packet');
      const json = JSON.stringify(packet, null, 2);
      await navigator.clipboard.writeText(json);  
      onStatus(t('context.packetCopied'));
    } catch (e) {
      onStatus(t('context.exportFailed', { error: e }));
    } finally {
      setExporting(false);
    }
  };

  return (
    <ProGate feature={t('context.handoffFeature')}>
    <button
      onClick={exportContext}
      disabled={exporting}
      aria-label={exporting ? t('context.exporting') : t('context.exportToClipboard')}
      aria-busy={exporting}
      className="w-10 h-10 rounded-lg flex items-center justify-center bg-bg-tertiary text-gray-500 border border-border hover:text-gray-300 transition-all disabled:opacity-30"
      title={t('context.exportToClipboard')}
    >
      {exporting ? (
        <div className="w-4 h-4 border-2 border-gray-500 border-t-transparent rounded-full animate-spin" />
      ) : (
        '📋'
      )}
    </button>
    </ProGate>
  );
}
