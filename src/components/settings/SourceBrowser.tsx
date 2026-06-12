// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useEffect, useMemo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';
import type { CuratedFeedInfo } from '../../lib/commands';

const DOMAIN_ORDER = [
  'rust', 'typescript', 'javascript', 'python', 'go',
  'security', 'ai-ml', 'infrastructure', 'systems',
  'web-platform', 'databases', 'devops', 'open-source',
];

function frequencyLabel(days: number): string {
  if (days <= 1) return 'Daily';
  if (days <= 3) return 'Every few days';
  if (days <= 7) return 'Weekly';
  if (days <= 14) return 'Biweekly';
  return 'Monthly';
}

function editorialLabel(model: string): string {
  switch (model) {
    case 'official': return 'Official';
    case 'single-expert': return 'Single Author';
    case 'editorial-team': return 'Editorial Team';
    case 'community': return 'Community';
    default: return model;
  }
}

export function SourceBrowser() {
  const { t } = useTranslation();
  const [feeds, setFeeds] = useState<CuratedFeedInfo[]>([]);
  const [domains, setDomains] = useState<string[]>([]);
  const [activeDomain, setActiveDomain] = useState<string>('all');
  const [searchQuery, setSearchQuery] = useState('');
  const [loading, setLoading] = useState(true);
  const [hoveredId, setHoveredId] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    cmd('get_curated_feeds')
      .then((res) => {
        if (cancelled) return;
        setFeeds(res?.feeds ?? []);
        setDomains(res?.domains ?? []);
        setLoading(false);
      })
      .catch(() => {
        if (!cancelled) setLoading(false);
      });
    return () => { cancelled = true; };
  }, []);

  const handleDomainChange = useCallback(async (domain: string) => {
    setActiveDomain(domain);
    setLoading(true);
    try {
      if (domain === 'all') {
        const res = await cmd('get_curated_feeds');
        setFeeds(res?.feeds ?? []);
      } else if (domain === 'suggested') {
        const res = await cmd('get_suggested_curated_feeds');
        setFeeds(res?.feeds ?? []);
      } else {
        const res = await cmd('get_curated_feeds_by_domain', { domain });
        setFeeds(res?.feeds ?? []);
      }
    } catch { /* keep existing feeds on error */ }
    setLoading(false);
  }, []);

  const handleToggle = useCallback(async (url: string, currentEnabled: boolean) => {
    const nextEnabled = !currentEnabled;
    // Optimistic update
    setFeeds((prev) => prev.map((f) => f.url === url ? { ...f, enabled: nextEnabled } : f));
    try {
      const res = await cmd('toggle_curated_feed', { url, enabled: nextEnabled });
      if (!res.success) {
        setFeeds((prev) => prev.map((f) => f.url === url ? { ...f, enabled: currentEnabled } : f));
      }
    } catch {
      setFeeds((prev) => prev.map((f) => f.url === url ? { ...f, enabled: currentEnabled } : f));
    }
  }, []);

  const filtered = useMemo(() => {
    if (!searchQuery.trim()) return feeds;
    const q = searchQuery.toLowerCase();
    return feeds.filter(
      (f) => f.name.toLowerCase().includes(q) || f.description.toLowerCase().includes(q)
    );
  }, [feeds, searchQuery]);

  const enabledCount = useMemo(() => feeds.filter((f) => f.enabled).length, [feeds]);

  const tierBorderClass = (tier: string) => {
    switch (tier) {
      case 'core': return 'border-accent-gold/60';
      case 'ecosystem': return 'border-text-primary/30';
      default: return 'border-border';
    }
  };

  const tierLabel = (tier: string) => {
    switch (tier) {
      case 'core': return 'Core';
      case 'ecosystem': return 'Ecosystem';
      default: return 'Peripheral';
    }
  };

  if (loading && feeds.length === 0) {
    return (
      <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
        <div className="flex items-center gap-2">
          <div className="w-8 h-8 bg-accent-gold/20 rounded-lg flex items-center justify-center flex-shrink-0">
            {/* eslint-disable-next-line i18next/no-literal-string */}
            <span className="text-accent-gold text-sm">&#x2731;</span>
          </div>
          <div>
            <h3 className="text-text-primary font-medium text-sm">
              {t('settings.sourceBrowser.title', 'Curated Sources')}
            </h3>
            <p className="text-text-muted text-xs animate-pulse">
              {t('settings.sourceBrowser.loading', 'Loading catalog...')}
            </p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
      {/* Header */}
      <div className="flex items-start gap-3 mb-3">
        <div className="w-8 h-8 bg-accent-gold/20 rounded-lg flex items-center justify-center flex-shrink-0">
          {/* eslint-disable-next-line i18next/no-literal-string */}
          <span className="text-accent-gold text-sm">&#x2731;</span>
        </div>
        <div className="flex-1 min-w-0">
          <h3 className="text-text-primary font-medium text-sm">
            {t('settings.sourceBrowser.title', 'Curated Sources')}
          </h3>
          <p className="text-text-muted text-xs mt-0.5">
            {t('settings.sourceBrowser.subtitle', '{{enabled}} of {{total}} sources enabled', {
              enabled: enabledCount,
              total: feeds.length,
            })}
          </p>
        </div>
      </div>

      {/* Search */}
      <div className="mb-3">
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          placeholder={t('settings.sourceBrowser.search', 'Search sources...')}
          className="w-full px-3 py-1.5 text-xs bg-bg-primary border border-border rounded-lg text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent-gold/50 transition-colors"
        />
      </div>

      {/* Domain filter bar */}
      <div className="mb-3 flex gap-1.5 overflow-x-auto pb-1 scrollbar-thin scrollbar-thumb-[var(--color-border)]">
        <DomainPill
          label={t('settings.sourceBrowser.all', 'All')}
          active={activeDomain === 'all'}
          onClick={() => void handleDomainChange('all')}
        />
        <DomainPill
          label={t('settings.sourceBrowser.suggested', 'Suggested')}
          active={activeDomain === 'suggested'}
          onClick={() => void handleDomainChange('suggested')}
          accent
        />
        {(domains.length > 0 ? domains : DOMAIN_ORDER).map((d) => (
          <DomainPill
            key={d}
            label={d}
            active={activeDomain === d}
            onClick={() => void handleDomainChange(d)}
          />
        ))}
      </div>

      {/* Feed grid */}
      {filtered.length === 0 ? (
        <p className="text-center text-text-muted text-xs py-6">
          {t('settings.sourceBrowser.noResults', 'No sources match your search.')}
        </p>
      ) : (
        <div className="grid grid-cols-1 xl:grid-cols-2 gap-2 max-h-[420px] overflow-y-auto pr-0.5">
          {filtered.map((feed) => (
            <FeedCard
              key={feed.id}
              feed={feed}
              hovered={hoveredId === feed.id}
              onHover={setHoveredId}
              onToggle={handleToggle}
              tierBorderClass={tierBorderClass}
              tierLabel={tierLabel}
            />
          ))}
        </div>
      )}
    </div>
  );
}

// -- Sub-components --

function DomainPill({
  label,
  active,
  onClick,
  accent,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
  accent?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      className={`px-2.5 py-1 text-[11px] rounded-full whitespace-nowrap transition-colors flex-shrink-0 font-medium ${
        active
          ? accent
            ? 'bg-accent-gold/20 text-accent-gold border border-accent-gold/40'
            : 'bg-text-primary/10 text-text-primary border border-text-primary/20'
          : 'bg-bg-secondary text-text-secondary border border-border hover:text-text-primary hover:border-text-primary/20'
      }`}
    >
      {label}
    </button>
  );
}

function FeedCard({
  feed,
  hovered,
  onHover,
  onToggle,
  tierBorderClass,
  tierLabel,
}: {
  feed: CuratedFeedInfo;
  hovered: boolean;
  onHover: (id: string | null) => void;
  onToggle: (url: string, enabled: boolean) => Promise<void>;
  tierBorderClass: (tier: string) => string;
  tierLabel: (tier: string) => string;
}) {
  return (
    <div
      className={`relative p-3 rounded-lg border bg-bg-secondary transition-colors ${tierBorderClass(feed.tier)} hover:bg-bg-tertiary`}
      onMouseEnter={() => onHover(feed.id)}
      onMouseLeave={() => onHover(null)}
    >
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 mb-1">
            <span className="text-text-primary text-xs font-semibold truncate">{feed.name}</span>
            <span
              className={`px-1.5 py-0.5 text-[9px] rounded font-medium border ${
                feed.tier === 'core'
                  ? 'text-accent-gold border-accent-gold/40 bg-accent-gold/10'
                  : feed.tier === 'ecosystem'
                    ? 'text-text-primary/80 border-text-primary/20 bg-text-primary/5'
                    : 'text-text-muted border-border bg-bg-primary'
              }`}
            >
              {tierLabel(feed.tier)}
            </span>
          </div>
          <p className="text-text-muted text-[11px] leading-tight line-clamp-2 mb-1.5">
            {feed.description}
          </p>
          <div className="flex flex-wrap gap-1">
            {feed.domains.map((d) => (
              <span
                key={d}
                className="px-1.5 py-0.5 text-[9px] rounded bg-bg-primary text-text-secondary border border-border"
              >
                {d}
              </span>
            ))}
          </div>
        </div>
        {/* Toggle */}
        <button
          onClick={() => void onToggle(feed.url, feed.enabled)}
          className={`relative w-9 h-[18px] rounded-full transition-colors flex-shrink-0 mt-0.5 ${
            feed.enabled ? 'bg-success/40' : 'bg-bg-tertiary'
          }`}
          aria-label={feed.enabled ? `Disable ${feed.name}` : `Enable ${feed.name}`}
        >
          <span
            className={`absolute top-[3px] w-3 h-3 rounded-full bg-white transition-transform ${
              feed.enabled ? 'translate-x-[18px]' : 'translate-x-[3px]'
            }`}
          />
        </button>
      </div>
      {/* Hover tooltip with editorial + frequency */}
      {hovered && (
        <div className="absolute bottom-full left-3 mb-1 px-2 py-1 rounded bg-bg-primary border border-border shadow-lg z-10 whitespace-nowrap">
          <span className="text-[10px] text-text-secondary">
            {editorialLabel(feed.editorial_model)} &middot; {frequencyLabel(feed.expected_frequency_days ?? 7)}
          </span>
        </div>
      )}
    </div>
  );
}
