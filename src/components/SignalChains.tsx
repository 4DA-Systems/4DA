import { useState, useEffect, memo } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../lib/commands';
import { ProGate } from './ProGate';
import type { SignalChainWithPrediction, ChainPhase } from '../types';

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
  low: 'text-text-secondary',
};

const PHASE_CONFIG: Record<ChainPhase, { label: string; color: string; bg: string }> = {
  nascent: { label: 'Nascent', color: 'text-gray-400', bg: 'bg-gray-500/20' },
  active: { label: 'Active', color: 'text-blue-400', bg: 'bg-blue-500/20' },
  escalating: { label: 'Escalating', color: 'text-orange-400', bg: 'bg-orange-500/20' },
  peak: { label: 'Peak', color: 'text-red-400', bg: 'bg-red-500/20' },
  resolving: { label: 'Resolving', color: 'text-green-400', bg: 'bg-green-500/20' },
};

export const SignalChainsPanel = memo(function SignalChainsPanel() {
  const { t } = useTranslation();
  const [chains, setChains] = useState<SignalChainWithPrediction[]>([]);
  const [expandedChain, setExpandedChain] = useState<string | null>(null);

  useEffect(() => {
    const load = async () => {
      try {
        const c = await cmd('get_signal_chains_predicted');
        setChains(c.filter(ch => ch.resolution === 'open'));
      } catch {
        // Fall back to non-predicted chains
        try {
          const fallback = await cmd('get_signal_chains');
          const open = fallback.filter(ch => ch.resolution === 'open');
          setChains(open.map(ch => ({
            ...ch,
            prediction: {
              phase: 'active' as ChainPhase,
              intervals_hours: [],
              acceleration: 0,
              predicted_next_hours: null,
              confidence: 0,
              forecast: '',
            },
          })));
        } catch {
          // Signal chains are optional
        }
      }
    };
    load();
  }, []);

  const resolveChain = async (chainId: string) => {
    try {
      await cmd('resolve_signal_chain', { chainId, resolution: 'resolved' });
      setChains(prev => prev.filter(c => c.id !== chainId));
    } catch (e) {
      console.error('Failed to resolve chain:', e);
    }
  };

  return (
    <ProGate feature={t('signals.feature')}>
    <div className="mb-6 bg-bg-secondary rounded-lg border border-border overflow-hidden">
      {chains.length === 0 ? (
        <div className="px-5 py-4 flex items-center gap-3">
          <div className="w-8 h-8 bg-bg-tertiary rounded-lg flex items-center justify-center">
            <span className="text-text-secondary">&#x1F517;</span>
          </div>
          <div>
            <h2 className="font-medium text-white text-sm">{t('signals.title')}</h2>
            <p className="text-xs text-text-muted">{t('signals.noChains', 'No active chains — monitoring for emerging patterns')}</p>
          </div>
        </div>
      ) : (
      <>
      <div className="px-5 py-4 border-b border-border flex items-center gap-3">
        <div className="w-8 h-8 bg-bg-tertiary rounded-lg flex items-center justify-center">
          <span className="text-text-secondary">&#x1F517;</span>
        </div>
        <div>
          <h2 className="font-medium text-white text-sm">{t('signals.title')}</h2>
          <p className="text-xs text-text-muted">{t('signals.activeChains', { count: chains.length })}</p>
        </div>
      </div>

      <div className="p-4 space-y-3">
        {chains.map((chain) => {
          const isExpanded = expandedChain === chain.id;
          const colors = PRIORITY_COLORS[chain.overall_priority] || PRIORITY_COLORS.low;
          const textColor = PRIORITY_TEXT[chain.overall_priority] || PRIORITY_TEXT.low;
          const phase = PHASE_CONFIG[chain.prediction.phase] || PHASE_CONFIG.active;

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
                  <div className="flex items-center gap-2">
                    <p className={`text-sm font-medium ${textColor}`}>{chain.chain_name}</p>
                    <span className={`text-[10px] px-1.5 py-0.5 rounded ${phase.bg} ${phase.color}`}>
                      {phase.label}
                    </span>
                  </div>
                  <p className="text-xs text-text-secondary mt-0.5">{chain.suggested_action}</p>
                  {chain.prediction.forecast && (
                    <p className="text-xs text-text-muted mt-1 italic">{chain.prediction.forecast}</p>
                  )}
                  <div className="flex items-center gap-2 mt-1.5">
                    <span className="text-[10px] text-text-muted">{t('signals.signalCount', { count: chain.links.length })}</span>
                    <span className="text-[10px] text-text-muted">
                      {new Date(chain.created_at).toLocaleDateString()}
                    </span>
                    {chain.prediction.acceleration < -2 && (
                      <span className="text-[10px] text-orange-400">&#x26A1; accelerating</span>
                    )}
                  </div>
                </div>
                <span className="text-text-muted text-xs">{isExpanded ? '\u25BE' : '\u25B8'}</span>
              </button>

              {isExpanded && (
                <div className="px-4 pb-3 border-t border-border/50">
                  {/* Prediction summary */}
                  {chain.prediction.confidence > 0.1 && (
                    <div className="mt-3 p-2.5 bg-bg-tertiary/50 rounded-lg border border-border/50">
                      <div className="flex items-center gap-4 text-xs">
                        <div>
                          <span className="text-text-muted">Phase: </span>
                          <span className={phase.color}>{phase.label}</span>
                        </div>
                        {chain.prediction.predicted_next_hours != null && (
                          <div>
                            <span className="text-text-muted">Next signal: </span>
                            <span className="text-white">
                              {chain.prediction.predicted_next_hours < 24
                                ? `~${Math.round(chain.prediction.predicted_next_hours)}h`
                                : `~${Math.round(chain.prediction.predicted_next_hours / 24)}d`}
                            </span>
                          </div>
                        )}
                        <div>
                          <span className="text-text-muted">Confidence: </span>
                          <span className="text-white">{Math.round(chain.prediction.confidence * 100)}%</span>
                        </div>
                        {chain.prediction.acceleration !== 0 && (
                          <div>
                            <span className="text-text-muted">Trend: </span>
                            <span className={chain.prediction.acceleration < 0 ? 'text-orange-400' : 'text-green-400'}>
                              {chain.prediction.acceleration < 0 ? 'speeding up' : 'slowing down'}
                            </span>
                          </div>
                        )}
                      </div>
                    </div>
                  )}

                  <div className="mt-3 space-y-2">
                    {chain.links.map((link, i) => (
                      <div key={i} className="flex items-start gap-2 pl-2">
                        <div className="flex flex-col items-center pt-1">
                          <div className="w-1.5 h-1.5 rounded-full bg-gray-500" />
                          {i < chain.links.length - 1 && <div className="w-px h-full bg-gray-700" />}
                        </div>
                        <div className="flex-1">
                          <p className="text-xs text-text-secondary">{link.description}</p>
                          <div className="flex items-center gap-2 mt-0.5">
                            <span className="text-[10px] text-text-muted">{link.signal_type}</span>
                            <span className="text-[10px] text-text-muted">{link.title}</span>
                            <span className="text-[10px] text-text-muted ml-auto">
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
                      {t('signals.resolve')}
                    </button>
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </div>
      </>
      )}
    </div>
    </ProGate>
  );
});
