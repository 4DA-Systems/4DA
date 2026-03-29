import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import type { SignalChainWithPrediction } from '../../types/innovation';
import type { CompoundAdvantageScore } from '../../types/autophagy';

// ---------------------------------------------------------------------------
// Phase badge
// ---------------------------------------------------------------------------

const PHASE_STYLE: Record<string, { color: string; bg: string }> = {
  nascent:    { color: 'text-blue-400',   bg: 'bg-blue-500/10' },
  active:     { color: 'text-green-400',  bg: 'bg-green-500/10' },
  escalating: { color: 'text-amber-400',  bg: 'bg-amber-500/10' },
  peak:       { color: 'text-red-400',    bg: 'bg-red-500/10' },
  resolving:  { color: 'text-text-muted', bg: 'bg-bg-tertiary' },
};

const PRIORITY_STYLE: Record<string, string> = {
  critical: 'text-red-400',
  alert:    'text-amber-400',
  advisory: 'text-blue-400',
  watch:    'text-text-muted',
};

// ---------------------------------------------------------------------------
// Signal Chain Card
// ---------------------------------------------------------------------------

function ChainCard({ chain }: { chain: SignalChainWithPrediction }) {
  const { t } = useTranslation();
  const phase = PHASE_STYLE[chain.prediction.phase] ?? PHASE_STYLE.nascent!;
  const priorityColor = PRIORITY_STYLE[chain.overall_priority] ?? 'text-text-muted';

  return (
    <div className="px-3 py-2.5 rounded-lg bg-[#0F0F0F] border border-border/50">
      <div className="flex items-center gap-2 mb-1">
        <span className={`text-xs font-medium ${priorityColor} truncate flex-1`}>
          {chain.chain_name}
        </span>
        <span className={`text-[9px] px-1.5 py-0.5 rounded font-medium ${phase.color} ${phase.bg}`}>
          {t(`momentum.chainPhase.${chain.prediction.phase}`, chain.prediction.phase)}
        </span>
      </div>
      {chain.prediction.forecast !== '' && (
        <p className="text-[11px] text-text-muted leading-relaxed line-clamp-2">
          {chain.prediction.forecast}
        </p>
      )}
      {chain.suggested_action !== '' && chain.prediction.forecast === '' && (
        <p className="text-[11px] text-text-muted leading-relaxed line-clamp-2">
          {chain.suggested_action}
        </p>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// AWE Wisdom Display
// ---------------------------------------------------------------------------

interface AweData {
  principles?: string[];
  decisions_count?: number;
  domain?: string;
}

function WisdomCard({ aweData, advantage }: { aweData: AweData | null; advantage: CompoundAdvantageScore | null }) {
  const { t } = useTranslation();

  // If AWE has principles, show them
  if (aweData !== null && aweData.principles !== undefined && aweData.principles.length > 0) {
    return (
      <div className="bg-bg-secondary rounded-lg border border-border p-4">
        <h4 className="text-[10px] text-accent-gold uppercase tracking-wider font-medium mb-2.5">
          {t('momentum.learnedPrinciples')}
        </h4>
        <ul className="space-y-2">
          {aweData.principles.slice(0, 3).map((p, i) => (
            <li key={i} className="text-xs text-text-secondary leading-relaxed flex items-start gap-2">
              <span className="text-accent-gold/50 mt-0.5 flex-shrink-0">{'\u25C6'}</span>
              <span>{p}</span>
            </li>
          ))}
        </ul>
      </div>
    );
  }

  // Fallback: show compound advantage breakdown
  if (advantage !== null) {
    const acted = Number(advantage.windows_acted);
    const opened = Number(advantage.windows_opened);
    const responseRate = opened > 0 ? Math.round((acted / opened) * 100) : 0;
    const leadTime = Math.round(advantage.avg_lead_time_hours);

    return (
      <div className="bg-bg-secondary rounded-lg border border-border p-4">
        <h4 className="text-[10px] text-text-muted uppercase tracking-wider font-medium mb-2.5">
          {t('momentum.positioning')}
        </h4>
        <div className="grid grid-cols-2 gap-3">
          <div>
            <div className="text-lg font-semibold text-white tabular-nums">{responseRate}%</div>
            <div className="text-[10px] text-text-muted">{t('momentum.responseRate')}</div>
          </div>
          <div>
            <div className="text-lg font-semibold text-white tabular-nums">{leadTime > 0 ? `${leadTime}h` : '--'}</div>
            <div className="text-[10px] text-text-muted">{t('momentum.leadTime')}</div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="bg-bg-secondary rounded-lg border border-border p-4">
      <p className="text-xs text-text-muted">{t('momentum.noPrinciples')}</p>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main Component
// ---------------------------------------------------------------------------

export interface PositioningSectionProps {
  chains: SignalChainWithPrediction[];
  aweData: AweData | null;
  advantage: CompoundAdvantageScore | null;
}

export const PositioningSection = memo(function PositioningSection({
  chains,
  aweData,
  advantage,
}: PositioningSectionProps) {
  const { t } = useTranslation();
  const openChains = chains.filter(c => c.resolution === 'open').slice(0, 3);

  return (
    <section aria-label={t('momentum.positioning')}>
      <h3 className="text-[10px] text-text-muted uppercase tracking-wider font-medium mb-3 px-1">
        {t('momentum.positioning')}
      </h3>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
        {/* Signal Intelligence */}
        <div className="bg-bg-secondary rounded-lg border border-border p-4">
          <h4 className="text-[10px] text-text-muted uppercase tracking-wider font-medium mb-2.5">
            {t('momentum.signalIntel')}
          </h4>
          {openChains.length > 0 ? (
            <div className="space-y-2">
              {openChains.map(chain => (
                <ChainCard key={chain.id} chain={chain} />
              ))}
            </div>
          ) : (
            <p className="text-xs text-text-muted py-2">{t('momentum.noChains')}</p>
          )}
        </div>

        {/* Wisdom / Positioning */}
        <WisdomCard aweData={aweData} advantage={advantage} />
      </div>
    </section>
  );
});
