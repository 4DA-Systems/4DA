import { useState, useEffect, memo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { SignalChain } from '../types';

const PRIORITY_COLORS: Record<string, string> = {
  critical: 'border-red-500/30 bg-red-500/10',
  high: 'border-amber-500/30 bg-amber-500/10',
  medium: 'border-yellow-500/30 bg-yellow-500/10',
  low: 'border-gray-500/30 bg-gray-500/10',
};

const PRIORITY_TEXT: Record<string, string> = {
  critical: 'text-red-400',
  high: 'text-amber-400',
  medium: 'text-yellow-400',
  low: 'text-gray-400',
};

export const SignalChainsPanel = memo(function SignalChainsPanel() {
  const [chains, setChains] = useState<SignalChain[]>([]);
  const [expandedChain, setExpandedChain] = useState<string | null>(null);

  useEffect(() => {
    const load = async () => {
      try {
        const c = await invoke<SignalChain[]>('get_signal_chains');
        setChains(c.filter(ch => ch.resolution === 'open'));
      } catch {
        // Signal chains are optional, don't error
      }
    };
    load();
  }, []);

  const resolveChain = async (chainId: string) => {
    try {
      await invoke('resolve_signal_chain', { chainId, resolution: 'resolved' });
      setChains(prev => prev.filter(c => c.id !== chainId));
    } catch (e) {
      console.error('Failed to resolve chain:', e);
    }
  };

  if (chains.length === 0) return null;

  return (
    <div className="mb-6 bg-[#141414] rounded-lg border border-[#2A2A2A] overflow-hidden">
      <div className="px-5 py-4 border-b border-[#2A2A2A] flex items-center gap-3">
        <div className="w-8 h-8 bg-[#1F1F1F] rounded-lg flex items-center justify-center">
          <span className="text-gray-400">🔗</span>
        </div>
        <div>
          <h2 className="font-medium text-white text-sm">Signal Chains</h2>
          <p className="text-xs text-gray-500">{chains.length} active chain{chains.length !== 1 ? 's' : ''}</p>
        </div>
      </div>

      <div className="p-4 space-y-3">
        {chains.map((chain) => {
          const isExpanded = expandedChain === chain.id;
          const colors = PRIORITY_COLORS[chain.overall_priority] || PRIORITY_COLORS.low;
          const textColor = PRIORITY_TEXT[chain.overall_priority] || PRIORITY_TEXT.low;

          return (
            <div key={chain.id} className={`rounded-lg border ${colors} transition-all`}>
              <button
                onClick={() => setExpandedChain(isExpanded ? null : chain.id)}
                className="w-full px-4 py-3 flex items-start gap-3 text-left"
              >
                <div className="flex flex-col items-center gap-1 pt-0.5">
                  {chain.links.map((_, i) => (
                    <div key={i} className="flex flex-col items-center">
                      <div className={`w-2 h-2 rounded-full ${
                        i === 0 ? 'bg-white' : 'bg-gray-500'
                      }`} />
                      {i < chain.links.length - 1 && (
                        <div className="w-px h-3 bg-gray-600" />
                      )}
                    </div>
                  ))}
                </div>
                <div className="flex-1 min-w-0">
                  <p className={`text-sm font-medium ${textColor}`}>{chain.chain_name}</p>
                  <p className="text-xs text-gray-400 mt-0.5">{chain.suggested_action}</p>
                  <div className="flex items-center gap-2 mt-1.5">
                    <span className="text-[10px] text-gray-500">{chain.links.length} signals</span>
                    <span className="text-[10px] text-gray-600">
                      {new Date(chain.created_at).toLocaleDateString()}
                    </span>
                  </div>
                </div>
                <span className="text-gray-500 text-xs">{isExpanded ? '▾' : '▸'}</span>
              </button>

              {isExpanded && (
                <div className="px-4 pb-3 border-t border-[#2A2A2A]/50">
                  <div className="mt-3 space-y-2">
                    {chain.links.map((link, i) => (
                      <div key={i} className="flex items-start gap-2 pl-2">
                        <div className="flex flex-col items-center pt-1">
                          <div className="w-1.5 h-1.5 rounded-full bg-gray-500" />
                          {i < chain.links.length - 1 && <div className="w-px h-full bg-gray-700" />}
                        </div>
                        <div className="flex-1">
                          <p className="text-xs text-gray-300">{link.description}</p>
                          <div className="flex items-center gap-2 mt-0.5">
                            <span className="text-[10px] text-gray-500">{link.signal_type}</span>
                            <span className="text-[10px] text-gray-600">{link.title}</span>
                            <span className="text-[10px] text-gray-700 ml-auto">
                              {new Date(link.timestamp).toLocaleDateString()}
                            </span>
                          </div>
                        </div>
                      </div>
                    ))}
                  </div>
                  <div className="mt-3 flex gap-2">
                    <button
                      onClick={(e) => { e.stopPropagation(); resolveChain(chain.id); }}
                      className="px-3 py-1.5 text-xs bg-green-500/10 text-green-400 border border-green-500/20 rounded hover:bg-green-500/20 transition-colors"
                    >
                      Resolve
                    </button>
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
});
