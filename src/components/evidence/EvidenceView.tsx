// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * Evidence Tab — Intelligence Reconciliation Phase 12+13 (2026-04-17).
 *
 * The tab where 4DA proves it's working and that it's learning you.
 * Three sections when populated; a focused CTA when empty.
 *
 * Per Intelligence Doctrine rule 3: no vanity metrics. Every number
 * on screen informs an action or proves a claim.
 */

import { memo, useEffect, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';
import type { EvidenceFeed } from '../../../src-tauri/bindings/bindings/EvidenceFeed';

// ============================================================================
// Types
// ============================================================================

interface CommitmentContract {
  id: number;
  decision_statement: string;
  refutation_condition: string;
  subject: string;
  status: 'active' | 'triggered' | 'dismissed';
  created_at: string;
  triggered_at: string | null;
}

// ============================================================================
// Helpers
// ============================================================================

function contractAge(createdAt: string): number {
  return Math.floor((Date.now() - new Date(createdAt).getTime()) / 86_400_000);
}

function daysAgo(isoDate: string): string {
  const diff = Date.now() - new Date(isoDate).getTime();
  const d = Math.floor(diff / 86_400_000);
  if (d === 0) return 'today';
  if (d === 1) return '1 day ago';
  return `${d} days ago`;
}

// ============================================================================
// Section 1 — This Week
// ============================================================================

const ThisWeekSection = memo(function ThisWeekSection({
  preemptionCount,
  blindSpotCount,
  contractCount,
}: {
  preemptionCount: number;
  blindSpotCount: number;
  contractCount: number;
}) {
  const { t } = useTranslation();
  const total = preemptionCount + blindSpotCount;

  return (
    <section className="bg-bg-secondary rounded-lg border border-border p-5">
      <h2 className="text-[10px] text-text-muted uppercase tracking-wider mb-3">
        {t('evidence.thisWeek')}
      </h2>
      {total === 0 && contractCount === 0 ? (
        <p className="text-sm text-text-secondary">
          {t('evidence.thisWeekEmpty')}
        </p>
      ) : (
        <p className="text-sm text-text-secondary leading-relaxed">
          {t('evidence.thisWeekSummary', {
            preemption: preemptionCount,
            blindSpots: blindSpotCount,
            contracts: contractCount,
            contractPlural: contractCount === 1 ? '' : 's',
          })}
        </p>
      )}
    </section>
  );
});

// ============================================================================
// Section 2 — Active Commitments
// ============================================================================

const CommitmentCard = memo(function CommitmentCard({
  contract,
  onDismiss,
}: {
  contract: CommitmentContract;
  onDismiss: (id: number) => void;
}) {
  const age = contractAge(contract.created_at);

  return (
    <div
      className={`rounded-lg border p-4 ${
        contract.status === 'triggered'
          ? 'border-red-500/30 bg-red-500/[0.06]'
          : 'border-border bg-bg-tertiary/30'
      }`}
    >
      <div className="flex items-start gap-3">
        <span
          className={`shrink-0 text-xs mt-0.5 ${
            contract.status === 'triggered' ? 'text-red-400' : 'text-accent-gold'
          }`}
          aria-hidden="true"
        >
          {contract.status === 'triggered' ? '!' : '◇'}
        </span>
        <div className="flex-1 min-w-0">
          <p className="text-sm text-white">{contract.decision_statement}</p>
          <p className="text-xs text-text-muted mt-1">
            Watching for: <span className="text-text-secondary italic">{contract.refutation_condition}</span>
          </p>
          <div className="flex items-center gap-3 mt-2 text-[10px] text-text-muted">
            <span>{age === 0 ? 'today' : `${age}d ago`}</span>
            {contract.status === 'triggered' && contract.triggered_at && (
              <span className="text-red-400">triggered {daysAgo(contract.triggered_at)}</span>
            )}
            {contract.status === 'active' && (
              <span className="text-green-400/60">watching</span>
            )}
          </div>
        </div>
        {contract.status === 'triggered' && (
          <button
            type="button"
            onClick={() => onDismiss(contract.id)}
            className="shrink-0 text-[10px] text-text-muted hover:text-white transition-colors px-2 py-1 rounded border border-border"
          >
            Dismiss
          </button>
        )}
      </div>
    </div>
  );
});

const CommitmentsSection = memo(function CommitmentsSection({
  contracts,
  onDismiss,
}: {
  contracts: CommitmentContract[];
  onDismiss: (id: number) => void;
}) {
  const { t } = useTranslation();
  const active = contracts.filter(c => c.status === 'active');
  const triggered = contracts.filter(c => c.status === 'triggered');
  const all = [...triggered, ...active];

  if (all.length === 0) return null;

  return (
    <section>
      <div className="flex items-center justify-between mb-3 px-1">
        <h2 className="text-[10px] text-text-muted uppercase tracking-wider">
          {t('evidence.commitments')}
        </h2>
        <span className="text-[10px] text-text-muted tabular-nums">
          {active.length} watching{triggered.length > 0 && ` · ${triggered.length} triggered`}
        </span>
      </div>
      <div className="space-y-2">
        {all.slice(0, 10).map(c => (
          <CommitmentCard key={c.id} contract={c} onDismiss={onDismiss} />
        ))}
      </div>
    </section>
  );
});

// ============================================================================
// Hero CTA — the main content when the page is empty
// ============================================================================

const WisdomCta = memo(function WisdomCta() {
  const { t } = useTranslation();
  return (
    <section className="bg-bg-secondary rounded-lg border border-accent-gold/20 p-8 text-center space-y-4">
      <div className="w-14 h-14 rounded-full bg-accent-gold/10 border border-accent-gold/30 flex items-center justify-center mx-auto">
        <span className="text-accent-gold text-xl" aria-hidden="true">◇</span>
      </div>
      <div>
        <h3 className="text-base font-medium text-white">
          {t('evidence.ctaTitle')}
        </h3>
        <p className="text-sm text-text-secondary mt-2 max-w-md mx-auto leading-relaxed">
          Press <kbd className="px-1.5 py-0.5 rounded bg-bg-tertiary border border-border text-xs font-mono text-accent-gold">⌘.</kbd> anywhere
          to consult the Wisdom engine on a decision you are weighing. Your decisions, commitments, and
          their outcomes will appear here as proof that 4DA is compounding your judgment.
        </p>
      </div>
      <div className="flex justify-center gap-6 pt-2 text-[10px] text-text-muted">
        <div className="text-center">
          <div className="text-lg font-semibold text-white tabular-nums">0</div>
          <div>decisions</div>
        </div>
        <div className="text-center">
          <div className="text-lg font-semibold text-white tabular-nums">0</div>
          <div>commitments</div>
        </div>
        <div className="text-center">
          <div className="text-lg font-semibold text-white tabular-nums">0</div>
          <div>outcomes</div>
        </div>
      </div>
    </section>
  );
});

// ============================================================================
// Main View
// ============================================================================

const EvidenceView = memo(function EvidenceView() {
  const { t } = useTranslation();
  const [contracts, setContracts] = useState<CommitmentContract[]>([]);
  const [preemptionCount, setPreemptionCount] = useState(0);
  const [blindSpotCount, setBlindSpotCount] = useState(0);
  const [loading, setLoading] = useState(true);

  const loadAll = useCallback(async () => {
    setLoading(true);
    const results = await Promise.allSettled([
      cmd('get_commitment_contracts'),
      cmd('get_preemption_alerts'),
      cmd('get_blind_spots'),
      cmd('check_refutations', { hours: 168 }),
    ]);

    if (results[0]!.status === 'fulfilled') {
      try { setContracts(JSON.parse(results[0]!.value as string) as CommitmentContract[]); }
      catch { setContracts([]); }
    }
    if (results[1]!.status === 'fulfilled') {
      const feed = results[1]!.value as unknown as EvidenceFeed;
      setPreemptionCount(feed.total);
    }
    if (results[2]!.status === 'fulfilled') {
      const feed = results[2]!.value as unknown as EvidenceFeed;
      setBlindSpotCount(feed.total);
    }

    setLoading(false);
  }, []);

  useEffect(() => { void loadAll(); }, [loadAll]);

  const handleDismiss = useCallback(async (contractId: number) => {
    try {
      await cmd('dismiss_commitment_contract', { contractId });
      setContracts(prev => prev.map(c =>
        c.id === contractId ? { ...c, status: 'dismissed' as const } : c,
      ));
    } catch { /* non-fatal */ }
  }, []);

  const activeContracts = contracts.filter(c => c.status !== 'dismissed');
  const hasContracts = activeContracts.length > 0;

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20 text-text-secondary text-sm">
        {t('evidence.loading')}
      </div>
    );
  }

  return (
    <div className="space-y-5 pb-8" role="tabpanel" id="view-panel-evidence">
      <header>
        <h1 className="text-xl font-semibold text-white tracking-tight">
          {t('evidence.title')}
        </h1>
        <p className="text-sm text-text-muted mt-1">
          {t('evidence.subtitle')}
        </p>
      </header>

      <ThisWeekSection
        preemptionCount={preemptionCount}
        blindSpotCount={blindSpotCount}
        contractCount={activeContracts.filter(c => c.status === 'active').length}
      />

      {hasContracts && (
        <CommitmentsSection
          contracts={activeContracts}
          onDismiss={handleDismiss}
        />
      )}

      {/* Hero CTA — the page's purpose when no decisions/contracts yet */}
      <WisdomCta />
    </div>
  );
});

export default EvidenceView;
