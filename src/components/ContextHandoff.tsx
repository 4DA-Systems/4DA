import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface ContextHandoffProps {
  onStatus: (msg: string) => void;
}

export function ContextHandoff({ onStatus }: ContextHandoffProps) {
  const [exporting, setExporting] = useState(false);

  const exportContext = async () => {
    setExporting(true);
    try {
      const packet = await invoke<object>('generate_context_packet');
      const json = JSON.stringify(packet, null, 2);
      await navigator.clipboard.writeText(json); // eslint-disable-line no-undef
      onStatus('Context packet copied to clipboard');
    } catch (e) {
      onStatus(`Export failed: ${e}`);
    } finally {
      setExporting(false);
    }
  };

  return (
    <button
      onClick={exportContext}
      disabled={exporting}
      className="w-10 h-10 rounded-lg flex items-center justify-center bg-[#1F1F1F] text-gray-500 border border-[#2A2A2A] hover:text-gray-300 transition-all disabled:opacity-30"
      title="Export context packet (clipboard)"
    >
      {exporting ? (
        <div className="w-4 h-4 border-2 border-gray-500 border-t-transparent rounded-full animate-spin" />
      ) : (
        '📋'
      )}
    </button>
  );
}
