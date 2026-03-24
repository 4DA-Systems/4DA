// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useState } from 'react';
import { useTranslation } from 'react-i18next';

type Competitor = 'overview' | 'feedly' | 'perplexity' | 'hackernews' | 'rss' | 'bigtech';

interface ComparisonRow {
  feature: string;
  fourda: string;
  competitor: string;
  fourdaStatus: 'yes' | 'partial' | 'no';
  competitorStatus: 'yes' | 'partial' | 'no';
}

const STATUS_STYLES = {
  yes: 'text-green-400',
  partial: 'text-amber-400',
  no: 'text-red-400',
};

const STATUS_ICONS = {
  yes: '\u2713',
  partial: '~',
  no: '\u2717',
};

const COMPETITORS: Array<{ id: Competitor; name: string; desc: string }> = [
  { id: 'overview', name: 'Summary', desc: 'All competitors at a glance' },
  { id: 'feedly', name: 'vs Feedly', desc: 'RSS aggregator (15M+ users)' },
  { id: 'perplexity', name: 'vs Perplexity', desc: 'AI search ($20/mo)' },
  { id: 'hackernews', name: 'vs Hacker News', desc: 'Community curation' },
  { id: 'rss', name: 'vs RSS Readers', desc: 'Traditional feed readers' },
  { id: 'bigtech', name: 'vs Big Tech', desc: 'When GitHub/MS ships feeds' },
];

function ComparisonTable({ rows, competitorName, featureLabel, fourdaLabel }: { rows: ComparisonRow[]; competitorName: string; featureLabel: string; fourdaLabel: string }) {
  return (
    <div className="bg-bg-tertiary/50 border border-border rounded-lg overflow-hidden">
      <table className="w-full text-xs">
        <thead>
          <tr className="border-b border-border">
            <th className="text-start py-2.5 px-4 font-medium text-text-muted">{featureLabel}</th>
            <th className="text-start py-2.5 px-4 font-medium text-orange-400 w-2/5">{fourdaLabel}</th>
            <th className="text-start py-2.5 px-4 font-medium text-text-secondary w-2/5">{competitorName}</th>
          </tr>
        </thead>
        <tbody>
          {rows.map(({ feature, fourda, competitor, fourdaStatus, competitorStatus }) => (
            <tr key={feature} className="border-b border-border/30">
              <td className="py-2 px-4 text-text-secondary font-medium">{feature}</td>
              <td className="py-2 px-4">
                <span className={STATUS_STYLES[fourdaStatus]}>
                  {STATUS_ICONS[fourdaStatus]} {fourda}
                </span>
              </td>
              <td className="py-2 px-4">
                <span className={STATUS_STYLES[competitorStatus]}>
                  {STATUS_ICONS[competitorStatus]} {competitor}
                </span>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function OverviewSection() {
  const { t } = useTranslation();
  return (
    <div className="space-y-4">
      <h3 className="text-lg font-semibold text-white">{t('comparison.competitivePosition')}</h3>
      <p className="text-sm text-text-secondary">
        4DA occupies a unique position: more focused than general aggregators, more proactive than
        search tools, more transparent than algorithmic feeds, and more private than any cloud service.
      </p>

      <div className="bg-bg-tertiary/50 border border-border rounded-lg overflow-hidden">
        <table className="w-full text-xs">
          <thead>
            <tr className="border-b border-border">
              <th className="text-start py-2.5 px-3 font-medium text-text-muted">{t('comparison.feature')}</th>
              <th className="text-center py-2.5 px-2 font-medium text-orange-400">4DA</th>
              <th className="text-center py-2.5 px-2 font-medium text-text-muted">{t('comparison.feedly')}</th>
              <th className="text-center py-2.5 px-2 font-medium text-text-muted">{t('comparison.perplexity')}</th>
              <th className="text-center py-2.5 px-2 font-medium text-text-muted">{t('comparison.hn')}</th>
            </tr>
          </thead>
          <tbody>
            {[
              { feature: 'Auto context discovery', scores: ['yes', 'no', 'no', 'no'] },
              { feature: 'Semantic relevance', scores: ['yes', 'partial', 'partial', 'no'] },
              { feature: 'Privacy (BYOK)', scores: ['yes', 'no', 'no', 'yes'] },
              { feature: 'Explainable scoring', scores: ['yes', 'no', 'no', 'partial'] },
              { feature: 'Multi-source', scores: ['yes', 'yes', 'partial', 'no'] },
              { feature: 'Behaviour learning', scores: ['yes', 'partial', 'partial', 'no'] },
              { feature: 'Developer-focused', scores: ['yes', 'no', 'no', 'partial'] },
              { feature: 'Ambient monitoring', scores: ['yes', 'partial', 'no', 'no'] },
              { feature: 'Desktop app', scores: ['yes', 'no', 'no', 'no'] },
            ].map(({ feature, scores }) => (
              <tr key={feature} className="border-b border-border/30">
                <td className="py-2 px-3 text-text-secondary">{feature}</td>
                {scores.map((s, i) => (
                  <td key={i} className={`py-2 px-2 text-center ${STATUS_STYLES[s as keyof typeof STATUS_STYLES]}`}>
                    {STATUS_ICONS[s as keyof typeof STATUS_ICONS]}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="grid grid-cols-2 gap-3">
        <div className="bg-bg-tertiary/30 border border-border/50 rounded-lg p-4">
          <p className="text-xs font-medium text-white mb-2">{t('comparison.fourdaMoat')}</p>
          <div className="space-y-1.5 text-xs text-text-muted">
            <p>1. Automatic context discovery (zero-config)</p>
            <p>2. 5-axis scoring with confirmation gate</p>
            <p>3. Full score transparency and explainability</p>
            <p>4. Privacy by architecture, not policy</p>
            <p>5. Compound knowledge (improves per-user over time)</p>
          </div>
        </div>
        <div className="bg-bg-tertiary/30 border border-border/50 rounded-lg p-4">
          <p className="text-xs font-medium text-white mb-2">{t('comparison.strategicPosition')}</p>
          <div className="space-y-1.5 text-xs text-text-muted">
            <p>More <span className="text-white">focused</span> than Feedly</p>
            <p>More <span className="text-white">proactive</span> than Hacker News</p>
            <p>More <span className="text-white">transparent</span> than Perplexity</p>
            <p>More <span className="text-white">private</span> than any cloud service</p>
            <p>More <span className="text-white">depth</span> than general AI assistants</p>
          </div>
        </div>
      </div>
    </div>
  );
}

function FeedlySection() {
  const { t } = useTranslation();
  const rows: ComparisonRow[] = [
    { feature: 'Context discovery', fourda: 'Codebase scanning', competitor: 'Manual keywords', fourdaStatus: 'yes', competitorStatus: 'no' },
    { feature: 'Relevance scoring', fourda: 'KNN + LLM + behaviour', competitor: 'Keywords only', fourdaStatus: 'yes', competitorStatus: 'partial' },
    { feature: 'Privacy', fourda: 'Local + BYOK', competitor: 'Cloud service', fourdaStatus: 'yes', competitorStatus: 'no' },
    { feature: 'Source count', fourda: 'HN/arXiv/Reddit/RSS/GitHub', competitor: '1000+ sources', fourdaStatus: 'partial', competitorStatus: 'yes' },
    { feature: 'Team features', fourda: 'Encrypted relay (planned)', competitor: 'Shared boards', fourdaStatus: 'partial', competitorStatus: 'yes' },
    { feature: 'Mobile app', fourda: 'Desktop only', competitor: 'iOS/Android', fourdaStatus: 'no', competitorStatus: 'yes' },
    { feature: 'Score explainability', fourda: 'Full breakdown', competitor: 'Black box', fourdaStatus: 'yes', competitorStatus: 'no' },
    { feature: 'Cost', fourda: '~$0.50/day (BYOK)', competitor: '$6-12/mo', fourdaStatus: 'yes', competitorStatus: 'partial' },
  ];
  return (
    <div className="space-y-4">
      <h3 className="text-lg font-semibold text-white">{t('comparison.vsFeedly')}</h3>
      <p className="text-sm text-text-secondary">
        Feedly is the incumbent RSS reader with 15M+ users. It excels at source breadth and team features.
        4DA excels at relevance depth and privacy.
      </p>
      <ComparisonTable rows={rows} competitorName="Feedly" featureLabel={t('comparison.feature')} fourdaLabel="4DA" />
      <div className="bg-amber-500/5 border border-amber-500/20 rounded-lg p-4">
        <p className="text-xs text-amber-400 font-medium mb-1">{t('comparison.whenChooseFeedly')}</p>
        <p className="text-xs text-text-muted">You need team collaboration, polished mobile apps, or 1000+ source integrations.</p>
      </div>
      <div className="bg-green-500/5 border border-green-500/20 rounded-lg p-4">
        <p className="text-xs text-green-400 font-medium mb-1">{t('comparison.whenChoose4DA')}</p>
        <p className="text-xs text-text-muted">You're a developer who values privacy, explainable scoring, and automatic context discovery from your codebase.</p>
      </div>
    </div>
  );
}

function PerplexitySection() {
  const { t } = useTranslation();
  const rows: ComparisonRow[] = [
    { feature: 'Interaction model', fourda: 'Proactive monitoring', competitor: 'Search-initiated', fourdaStatus: 'yes', competitorStatus: 'partial' },
    { feature: 'Context awareness', fourda: 'Codebase-aware', competitor: 'Query-based', fourdaStatus: 'yes', competitorStatus: 'no' },
    { feature: 'Privacy', fourda: 'Local + BYOK', competitor: 'Cloud service', fourdaStatus: 'yes', competitorStatus: 'no' },
    { feature: 'Real-time search', fourda: 'Scheduled + ambient', competitor: 'Always up-to-date', fourdaStatus: 'partial', competitorStatus: 'yes' },
    { feature: 'Conversational', fourda: 'Structured UI', competitor: 'Natural language', fourdaStatus: 'partial', competitorStatus: 'yes' },
    { feature: 'Explainability', fourda: 'Full score breakdown', competitor: 'Citation links', fourdaStatus: 'yes', competitorStatus: 'partial' },
    { feature: 'Cost', fourda: '~$0.50/day (BYOK)', competitor: '$20/mo', fourdaStatus: 'yes', competitorStatus: 'no' },
  ];
  return (
    <div className="space-y-4">
      <h3 className="text-lg font-semibold text-white">{t('comparison.vsPerplexity')}</h3>
      <p className="text-sm text-text-secondary">
        Perplexity is an AI search engine. It answers questions. 4DA surfaces what you didn't know to ask about.
      </p>
      <ComparisonTable rows={rows} competitorName="Perplexity" featureLabel={t('comparison.feature')} fourdaLabel="4DA" />
    </div>
  );
}

function HackerNewsSection() {
  const { t } = useTranslation();
  const rows: ComparisonRow[] = [
    { feature: 'Personalisation', fourda: 'Learns your interests', competitor: 'Same for everyone', fourdaStatus: 'yes', competitorStatus: 'no' },
    { feature: 'Sources', fourda: 'HN + arXiv + Reddit + RSS', competitor: 'HN only', fourdaStatus: 'yes', competitorStatus: 'no' },
    { feature: 'Filtering', fourda: '99%+ rejected (PASIFA)', competitor: 'Votes + chronological', fourdaStatus: 'yes', competitorStatus: 'partial' },
    { feature: 'Ambient monitoring', fourda: 'System tray + digest', competitor: 'Manual refresh', fourdaStatus: 'yes', competitorStatus: 'no' },
    { feature: 'Community', fourda: 'Solo tool', competitor: 'World-class discussions', fourdaStatus: 'no', competitorStatus: 'yes' },
    { feature: 'Cost', fourda: 'Free tier + BYOK', competitor: 'Free', fourdaStatus: 'yes', competitorStatus: 'yes' },
    { feature: 'Discovery serendipity', fourda: 'Focused (by design)', competitor: 'High (browsing)', fourdaStatus: 'partial', competitorStatus: 'yes' },
  ];
  return (
    <div className="space-y-4">
      <h3 className="text-lg font-semibold text-white">{t('comparison.vsHackerNews')}</h3>
      <p className="text-sm text-text-secondary">
        HN is the canonical tech aggregator. 4DA uses HN as a source — but filters it through your context
        so you see only what's relevant to YOUR work, not what 1M other developers voted on.
      </p>
      <ComparisonTable rows={rows} competitorName="Hacker News" featureLabel={t('comparison.feature')} fourdaLabel="4DA" />
    </div>
  );
}

function RssSection() {
  const { t } = useTranslation();
  const rows: ComparisonRow[] = [
    { feature: 'Setup', fourda: 'Zero-config (scans codebase)', competitor: 'Manual feed curation', fourdaStatus: 'yes', competitorStatus: 'no' },
    { feature: 'Relevance', fourda: 'Semantic 5-axis scoring', competitor: '"New items in feed"', fourdaStatus: 'yes', competitorStatus: 'no' },
    { feature: 'Learning', fourda: 'Improves from feedback', competitor: 'Static', fourdaStatus: 'yes', competitorStatus: 'no' },
    { feature: 'Simplicity', fourda: 'More complex', competitor: 'Subscribe + read', fourdaStatus: 'partial', competitorStatus: 'yes' },
    { feature: 'Cross-platform', fourda: 'Desktop only', competitor: 'Web/iOS/Android', fourdaStatus: 'no', competitorStatus: 'yes' },
    { feature: 'Cost', fourda: 'Free + BYOK', competitor: '$3-5/mo', fourdaStatus: 'yes', competitorStatus: 'yes' },
  ];
  return (
    <div className="space-y-4">
      <h3 className="text-lg font-semibold text-white">{t('comparison.vsRssReaders')}</h3>
      <p className="text-sm text-text-secondary">
        RSS readers show you everything in your feeds. 4DA shows you only what's relevant to your work.
        If you already have curated feeds and enjoy manual browsing, RSS is simpler. If you want
        intelligence, 4DA is deeper.
      </p>
      <ComparisonTable rows={rows} competitorName="RSS Readers" featureLabel={t('comparison.feature')} fourdaLabel="4DA" />
    </div>
  );
}

function BigTechSection() {
  const { t } = useTranslation();
  return (
    <div className="space-y-4">
      <h3 className="text-lg font-semibold text-white">{t('comparison.vsBigTech')}</h3>
      <p className="text-sm text-text-secondary">
        The highest-probability competitive threat: GitHub adds "Trending for You," Copilot adds
        relevant content, or JetBrains ships a developer feed plugin.
      </p>

      <div className="bg-bg-tertiary/50 border border-border rounded-lg p-4 space-y-3">
        <p className="text-xs font-medium text-white">Why this validates 4DA, not threatens it:</p>
        <div className="space-y-2 text-xs text-text-muted">
          <p>1. <span className="text-white">They prove the category exists.</span> If GitHub builds
          developer feeds, it proves the problem is real and worth solving.</p>
          <p>2. <span className="text-white">They can't offer privacy.</span> Their business model
          requires your data. 4DA's doesn't. This is structural, not fixable.</p>
          <p>3. <span className="text-white">They'll optimise for engagement.</span> Their metrics
          are time-on-platform. 4DA's metric is time saved. Different incentives, different products.</p>
          <p>4. <span className="text-white">"Good enough" serves the majority.</span> 4DA serves
          the 20% who care about precision and privacy. That 20% is the highest-value segment.</p>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-3">
        <div className="bg-red-500/5 border border-red-500/20 rounded-lg p-4">
          <p className="text-xs text-red-400 font-medium mb-2">{t('comparison.theirApproach')}</p>
          <div className="space-y-1 text-xs text-text-muted">
            <p>Cloud-based, data feeds models</p>
            <p>Optimised for engagement</p>
            <p>Aggregated signals (average user)</p>
            <p>"Good enough" for 100M users</p>
            <p>Privacy as policy (terms of service)</p>
          </div>
        </div>
        <div className="bg-green-500/5 border border-green-500/20 rounded-lg p-4">
          <p className="text-xs text-green-400 font-medium mb-2">{t('comparison.fourdaApproach')}</p>
          <div className="space-y-1 text-xs text-text-muted">
            <p>Local-first, data stays on machine</p>
            <p>Optimised for precision</p>
            <p>Individual signals (compound per-user)</p>
            <p>Excellent for developers who care</p>
            <p>Privacy as architecture (CSP, keychain)</p>
          </div>
        </div>
      </div>

      <div className="bg-orange-500/5 border border-orange-500/20 rounded-lg p-4">
        <p className="text-xs text-orange-400 font-medium mb-1">{t('comparison.frameworkDefence')}</p>
        <p className="text-xs text-text-muted">
          4DA is not just a product — it's a framework. PASIFA, the Authority Stack, and AOS are
          published openly. If a competitor adopts these concepts, they validate the approach.
          The reference implementation is at 4da.ai/framework.
        </p>
      </div>
    </div>
  );
}

interface ComparisonPageProps {
  onClose: () => void;
}

export const ComparisonPage = memo(function ComparisonPage({ onClose }: ComparisonPageProps) {
  const { t } = useTranslation();
  const [activeCompetitor, setActiveCompetitor] = useState<Competitor>('overview');

  const renderSection = () => {
    switch (activeCompetitor) {
      case 'overview': return <OverviewSection />;
      case 'feedly': return <FeedlySection />;
      case 'perplexity': return <PerplexitySection />;
      case 'hackernews': return <HackerNewsSection />;
      case 'rss': return <RssSection />;
      case 'bigtech': return <BigTechSection />;
    }
  };

  return (
    <div className="fixed inset-0 z-50 bg-bg-primary/95 backdrop-blur-sm flex flex-col">
      <div className="flex items-center justify-between px-6 py-4 border-b border-border">
        <div>
          <h2 className="text-lg font-semibold text-white">
            {t('comparison.title', { defaultValue: 'Competitive Comparison' })}
          </h2>
          <p className="text-xs text-text-muted">
            {t('comparison.subtitle', { defaultValue: 'How 4DA compares to existing tools' })}
          </p>
        </div>
        <button
          onClick={onClose}
          className="px-3 py-1.5 text-xs text-text-secondary hover:text-white border border-border rounded-lg hover:bg-bg-tertiary transition-all"
          aria-label={t('action.close', { defaultValue: 'Close' })}
        >
          {t('action.close', { defaultValue: 'Close' })}
        </button>
      </div>

      <div className="flex-1 flex overflow-hidden">
        <nav className="w-48 border-e border-border p-4 space-y-1 flex-shrink-0" aria-label="Competitors">
          {COMPETITORS.map(({ id, name, desc }) => (
            <button
              key={id}
              onClick={() => setActiveCompetitor(id)}
              className={`w-full text-start px-3 py-2 rounded-lg transition-all ${
                activeCompetitor === id
                  ? 'bg-orange-500/15 text-orange-400 font-medium'
                  : 'text-text-muted hover:text-text-secondary hover:bg-bg-tertiary/50'
              }`}
            >
              <span className="text-sm block">{name}</span>
              <span className="text-[10px] text-text-muted block">{desc}</span>
            </button>
          ))}
        </nav>

        <div className="flex-1 overflow-y-auto p-6 max-w-3xl">
          {renderSection()}
        </div>
      </div>
    </div>
  );
});

export default ComparisonPage;
