// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

/**
 * Evidence Tab — Intelligence Reconciliation Phase 12 (2026-04-17).
 *
 * The tab where 4DA proves — with timestamps and real items — that it's
 * working and that it's learning you. Replaces the old Momentum page.
 *
 * Three sections, fixed:
 *   1. This Week — honest one-line claim with real numbers
 *   2. Active Commitments — open Commitment Contracts + status
 *   3. Recent Intelligence — latest EvidenceItems from all lenses
 *
 * Per Intelligence Doctrine rule 3: no vanity metrics. Every number
 * on screen informs an action or proves a claim.
 */

import { memo, useEffect, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';
import type { EvidenceItem } from '../../../src-tauri/bindings/bindings/EvidenceItem';
import type { Urgency } from '../../../src-tauri/bindings/bindings/Urgency';
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

const URGENCY_COLOR: Record<Urgency, string> = {
  critical: 'text-red-400',
  high: 'text-orange-400',
  medium: 'text-yellow-400',
  watch: 'text-blue-400',
};

const KIND_LABEL: Record<string, string> = {
  alert: 'Alert',
  gap: 'Gap',
  missed_signal: 'Missed',
  chain: 'Chain',
  decision: 'Decision',
  retrospective: 'Retro',
  refutation: 'Refutation',
  precedent: 'Precedent',
};

function daysAgo(isoDate: string): string {
  const diff = Date.now() - new Date(isoDate).getTime();
  const d = Math.floor(diff / 86_400_000);
  if (d === 0) return 'today';
  if (d === 1) return '1 day ago';
  return `${d} days ago`;
}

function contractAge(createdAt: string): number {
  return Math.floor((Date.now() - new Date(createdAt).getTime()) / 86_400_000);
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
        {t('evidence.thisWeek', 'This week')}
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
            Refutation watch: <span className="text-text-secondary italic">{contract.refutation_condition}</span>
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

  if (all.length === 0) {
    return (
      <section className="bg-bg-secondary rounded-lg border border-border p-5">
        <h2 className="text-[10px] text-text-muted uppercase tracking-wider mb-3">
          {t('evidence.commitments', 'Commitment Contracts')}
        </h2>
        <p className="text-xs text-text-muted">
          {t('evidence.noCommitments', 'No active commitments. Accept a Decision Brief (⌘.) to set one.')}
        </p>
      </section>
    );
  }

  return (
    <section>
      <div className="flex items-center justify-between mb-3 px-1">
        <h2 className="text-[10px] text-text-muted uppercase tracking-wider">
          {t('evidence.commitments', 'Commitment Contracts')}
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
// Section 3 — Recent Intelligence Items
// ============================================================================

const IntelCard = memo(function IntelCard({ item }: { item: EvidenceItem }) {
  const primaryCite = item.evidence[0];
  const kindLabel = KIND_LABEL[item.kind] ?? item.kind;

  return (
    <div className="px-4 py-3 rounded-lg border border-border bg-bg-tertiary/20 hover:bg-bg-tertiary/40 transition-colors">
      <div className="flex items-start gap-2">
        <span className={`shrink-0 text-[10px] font-medium uppercase ${URGENCY_COLOR[item.urgency]}`}>
          {kindLabel}
        </span>
        <div className="flex-1 min-w-0">
          <p className="text-sm text-white leading-snug">{item.title}</p>
          {item.explanation && (
            <p className="text-xs text-text-secondary mt-1 line-clamp-2">{item.explanation}</p>
          )}
          <div className="flex items-center gap-2 mt-1.5 text-[10px] text-text-muted">
            {primaryCite && <span>{primaryCite.source}</span>}
            {item.confidence.value > 0 && (
              <>
                <span>·</span>
                <span className="tabular-nums">{Math.round(item.confidence.value * 100)}%</span>
              </>
            )}
            {item.precedents.length > 0 && (
              <>
                <span>·</span>
                <span>{item.precedents.length} precedent{item.precedents.length === 1 ? '' : 's'}</span>
              </>
            )}
          </div>
        </div>
      </div>
    </div>
  );
});

const RecentIntelSection = memo(function RecentIntelSection({
  items,
}: {
  items: EvidenceItem[];
}) {
  const { t } = useTranslation();

  if (items.length === 0) {
    return (
      <section className="bg-bg-secondary rounded-lg border border-border p-5">
        <h2 className="text-[10px] text-text-muted uppercase tracking-wider mb-3">
          {t('evidence.recentIntel', 'Recent Intelligence')}
        </h2>
        <p className="text-xs text-text-muted">
          {t('evidence.noIntel', 'No intelligence items yet. Sources are still scanning.')}
        </p>
      </section>
    );
  }

  return (
    <section>
      <h2 className="text-[10px] text-text-muted uppercase tracking-wider mb-3 px-1">
        {t('evidence.recentIntel', 'Recent Intelligence')}
      </h2>
      <div className="space-y-2">
        {items.slice(0, 15).map(it => (
          <IntelCard key={it.id} item={it} />
        ))}
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
  const [preemptionFeed, setPreemptionFeed] = useState<EvidenceFeed | null>(null);
  const [blindSpotFeed, setBlindSpotFeed] = useState<EvidenceFeed | null>(null);
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
    if (results[1]!.status === 'fulfilled') setPreemptionFeed(results[1]!.value as unknown as EvidenceFeed);
    if (results[2]!.status === 'fulfilled') setBlindSpotFeed(results[2]!.value as unknown as EvidenceFeed);

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

  // Merge items from both lenses, sorted by urgency then recency.
  const allItems: EvidenceItem[] = [
    ...(preemptionFeed?.items ?? []),
    ...(blindSpotFeed?.items ?? []),
  ].sort((a, b) => {
    const URGENCY_ORDER: Urgency[] = ['critical', 'high', 'medium', 'watch'];
    const u = URGENCY_ORDER.indexOf(a.urgency) - URGENCY_ORDER.indexOf(b.urgency);
    if (u !== 0) return u;
    return Number(b.created_at) - Number(a.created_at);
  });

  const activeContracts = contracts.filter(c => c.status !== 'dismissed');

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20 text-text-secondary text-sm">
        {t('evidence.loading', 'Loading evidence...')}
      </div>
    );
  }

  return (
    <div className="space-y-5 pb-8" role="tabpanel" id="view-panel-evidence">
      <header>
        <h1 className="text-xl font-semibold text-white tracking-tight">
          {t('evidence.title', 'Evidence')}
        </h1>
        <p className="text-sm text-text-muted mt-1">
          {t('evidence.subtitle', 'Proof that 4DA is working and learning you.')}
        </p>
      </header>

      <ThisWeekSection
        preemptionCount={preemptionFeed?.total ?? 0}
        blindSpotCount={blindSpotFeed?.total ?? 0}
        contractCount={activeContracts.filter(c => c.status === 'active').length}
      />

      <CommitmentsSection
        contracts={activeContracts}
        onDismiss={handleDismiss}
      />

      <RecentIntelSection items={allItems} />
    </div>
  );
});

export default EvidenceView;
