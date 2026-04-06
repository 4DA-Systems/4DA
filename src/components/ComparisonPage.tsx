// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useState } from 'react';
import { useTranslation } from 'react-i18next';

type Section = 'approach' | 'privacy' | 'scoring' | 'context' | 'compound' | 'tradeoffs';

function Principle({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="bg-bg-tertiary/30 border border-border/50 rounded-lg p-4">
      <p className="text-xs font-medium text-white mb-2">{title}</p>
      <div className="text-xs text-text-muted leading-relaxed">{children}</div>
    </div>
  );
}

function ArchitectureRow({ label, value, detail }: { label: string; value: string; detail?: string }) {
  return (
    <tr className="border-b border-border/30">
      <td className="py-2.5 px-4 text-text-secondary font-medium text-xs">{label}</td>
      <td className="py-2.5 px-4 text-xs text-white">{value}</td>
      {detail !== undefined && <td className="py-2.5 px-4 text-xs text-text-muted">{detail}</td>}
    </tr>
  );
}

function ApproachSection() {
  const { t } = useTranslation();
  return (
    <div className="space-y-4">
      <h3 className="text-lg font-semibold text-white">{t('comp.approach.title')}</h3>
      <p className="text-sm text-text-secondary leading-relaxed">{t('comp.approach.desc')}</p>

      <div className="grid grid-cols-1 gap-3">
        <Principle title={t('comp.approach.proactiveTitle')}>
          <p>{t('comp.approach.proactiveBody')}</p>
        </Principle>
        <Principle title={t('comp.approach.intelTitle')}>
          <p>{t('comp.approach.intelBody')}</p>
        </Principle>
        <Principle title={t('comp.approach.transparentTitle')}>
          <p>{t('comp.approach.transparentBody')}</p>
        </Principle>
      </div>

      <div className="bg-bg-tertiary/50 border border-border rounded-lg p-4">
        <p className="text-xs font-medium text-white mb-2">{t('comp.approach.categoryTitle')}</p>
        <p className="text-xs text-text-muted leading-relaxed">{t('comp.approach.categoryBody')}</p>
      </div>
    </div>
  );
}

function PrivacySection() {
  const { t } = useTranslation();
  return (
    <div className="space-y-4">
      <h3 className="text-lg font-semibold text-white">{t('comp.privacy.title')}</h3>
      <p className="text-sm text-text-secondary leading-relaxed">{t('comp.privacy.desc')}</p>

      <div className="bg-bg-tertiary/50 border border-border rounded-lg overflow-hidden">
        <table className="w-full text-xs">
          <thead>
            <tr className="border-b border-border">
              <th className="text-start py-2.5 px-4 font-medium text-text-muted">{t('comp.privacy.colLayer')}</th>
              <th className="text-start py-2.5 px-4 font-medium text-white">{t('comp.privacy.colHow')}</th>
              <th className="text-start py-2.5 px-4 font-medium text-text-muted">{t('comp.privacy.colVerify')}</th>
            </tr>
          </thead>
          <tbody>
            <ArchitectureRow label={t('comp.privacy.storageLabel')} value={t('comp.privacy.storageValue')} detail={t('comp.privacy.storageDetail')} />
            <ArchitectureRow label={t('comp.privacy.embeddingsLabel')} value={t('comp.privacy.embeddingsValue')} detail={t('comp.privacy.embeddingsDetail')} />
            <ArchitectureRow label={t('comp.privacy.llmLabel')} value={t('comp.privacy.llmValue')} detail={t('comp.privacy.llmDetail')} />
            <ArchitectureRow label={t('comp.privacy.scanLabel')} value={t('comp.privacy.scanValue')} detail={t('comp.privacy.scanDetail')} />
            <ArchitectureRow label={t('comp.privacy.networkLabel')} value={t('comp.privacy.networkValue')} detail={t('comp.privacy.networkDetail')} />
            <ArchitectureRow label={t('comp.privacy.keysLabel')} value={t('comp.privacy.keysValue')} detail={t('comp.privacy.keysDetail')} />
          </tbody>
        </table>
      </div>

      <div className="grid grid-cols-2 gap-3">
        <Principle title={t('comp.privacy.devTitle')}>
          <p>{t('comp.privacy.devBody')}</p>
        </Principle>
        <Principle title={t('comp.privacy.byokTitle')}>
          <p>{t('comp.privacy.byokBody')}</p>
        </Principle>
      </div>
    </div>
  );
}

function ScoringSection() {
  const { t } = useTranslation();
  return (
    <div className="space-y-4">
      <h3 className="text-lg font-semibold text-white">{t('comp.scoring.title')}</h3>
      <p className="text-sm text-text-secondary leading-relaxed">{t('comp.scoring.desc')}</p>

      <div className="bg-bg-tertiary/50 border border-border rounded-lg overflow-hidden">
        <table className="w-full text-xs">
          <thead>
            <tr className="border-b border-border">
              <th className="text-start py-2.5 px-4 font-medium text-text-muted">{t('comp.scoring.colPhase')}</th>
              <th className="text-start py-2.5 px-4 font-medium text-white">{t('comp.scoring.colWhat')}</th>
            </tr>
          </thead>
          <tbody>
            <ArchitectureRow label={t('comp.scoring.p1Label')} value={t('comp.scoring.p1Value')} />
            <ArchitectureRow label={t('comp.scoring.p2Label')} value={t('comp.scoring.p2Value')} />
            <ArchitectureRow label={t('comp.scoring.p3Label')} value={t('comp.scoring.p3Value')} />
            <ArchitectureRow label={t('comp.scoring.p4Label')} value={t('comp.scoring.p4Value')} />
            <ArchitectureRow label={t('comp.scoring.p5Label')} value={t('comp.scoring.p5Value')} />
            <ArchitectureRow label={t('comp.scoring.p6Label')} value={t('comp.scoring.p6Value')} />
            <ArchitectureRow label={t('comp.scoring.p7Label')} value={t('comp.scoring.p7Value')} />
            <ArchitectureRow label={t('comp.scoring.p8Label')} value={t('comp.scoring.p8Value')} />
          </tbody>
        </table>
      </div>

      <div className="grid grid-cols-2 gap-3">
        <Principle title={t('comp.scoring.gateTitle')}>
          <p>{t('comp.scoring.gateBody')}</p>
        </Principle>
        <Principle title={t('comp.scoring.explainTitle')}>
          <p>{t('comp.scoring.explainBody')}</p>
        </Principle>
      </div>
    </div>
  );
}

function ContextSection() {
  const { t } = useTranslation();
  return (
    <div className="space-y-4">
      <h3 className="text-lg font-semibold text-white">{t('comp.context.title')}</h3>
      <p className="text-sm text-text-secondary leading-relaxed">{t('comp.context.desc')}</p>

      <div className="bg-bg-tertiary/50 border border-border rounded-lg overflow-hidden">
        <table className="w-full text-xs">
          <thead>
            <tr className="border-b border-border">
              <th className="text-start py-2.5 px-4 font-medium text-text-muted">{t('comp.context.colSignal')}</th>
              <th className="text-start py-2.5 px-4 font-medium text-white">{t('comp.context.colExtract')}</th>
            </tr>
          </thead>
          <tbody>
            <ArchitectureRow label={t('comp.context.pkgLabel')} value={t('comp.context.pkgValue')} />
            <ArchitectureRow label={t('comp.context.importLabel')} value={t('comp.context.importValue')} />
            <ArchitectureRow label={t('comp.context.configLabel')} value={t('comp.context.configValue')} />
            <ArchitectureRow label={t('comp.context.docsLabel')} value={t('comp.context.docsValue')} />
            <ArchitectureRow label={t('comp.context.gitLabel')} value={t('comp.context.gitValue')} />
          </tbody>
        </table>
      </div>

      <div className="grid grid-cols-2 gap-3">
        <Principle title={t('comp.context.zeroTitle')}>
          <p>{t('comp.context.zeroBody')}</p>
        </Principle>
        <Principle title={t('comp.context.evolveTitle')}>
          <p>{t('comp.context.evolveBody')}</p>
        </Principle>
      </div>
    </div>
  );
}

function CompoundSection() {
  const { t } = useTranslation();
  return (
    <div className="space-y-4">
      <h3 className="text-lg font-semibold text-white">{t('comp.compound.title')}</h3>
      <p className="text-sm text-text-secondary leading-relaxed">{t('comp.compound.desc')}</p>

      <div className="grid grid-cols-1 gap-3">
        <Principle title={t('comp.compound.behaviourTitle')}>
          <p>{t('comp.compound.behaviourBody')}</p>
        </Principle>
        <Principle title={t('comp.compound.perUserTitle')}>
          <p>{t('comp.compound.perUserBody')}</p>
        </Principle>
        <Principle title={t('comp.compound.inspectTitle')}>
          <p>{t('comp.compound.inspectBody')}</p>
        </Principle>
      </div>
    </div>
  );
}

function TradeoffsSection() {
  const { t } = useTranslation();
  return (
    <div className="space-y-4">
      <h3 className="text-lg font-semibold text-white">{t('comp.tradeoffs.title')}</h3>
      <p className="text-sm text-text-secondary leading-relaxed">{t('comp.tradeoffs.desc')}</p>

      <div className="bg-bg-tertiary/50 border border-border rounded-lg overflow-hidden">
        <table className="w-full text-xs">
          <thead>
            <tr className="border-b border-border">
              <th className="text-start py-2.5 px-4 font-medium text-text-muted">{t('comp.tradeoffs.colChose')}</th>
              <th className="text-start py-2.5 px-4 font-medium text-white">{t('comp.tradeoffs.colMeans')}</th>
              <th className="text-start py-2.5 px-4 font-medium text-text-muted">{t('comp.tradeoffs.colCost')}</th>
            </tr>
          </thead>
          <tbody>
            <ArchitectureRow label={t('comp.tradeoffs.desktopLabel')} value={t('comp.tradeoffs.desktopValue')} detail={t('comp.tradeoffs.desktopCost')} />
            <ArchitectureRow label={t('comp.tradeoffs.localLabel')} value={t('comp.tradeoffs.localValue')} detail={t('comp.tradeoffs.localCost')} />
            <ArchitectureRow label={t('comp.tradeoffs.byokLabel')} value={t('comp.tradeoffs.byokValue')} detail={t('comp.tradeoffs.byokCost')} />
            <ArchitectureRow label={t('comp.tradeoffs.devLabel')} value={t('comp.tradeoffs.devValue')} detail={t('comp.tradeoffs.devCost')} />
            <ArchitectureRow label={t('comp.tradeoffs.precisionLabel')} value={t('comp.tradeoffs.precisionValue')} detail={t('comp.tradeoffs.precisionCost')} />
            <ArchitectureRow label={t('comp.tradeoffs.explainLabel')} value={t('comp.tradeoffs.explainValue')} detail={t('comp.tradeoffs.explainCost')} />
          </tbody>
        </table>
      </div>

      <div className="bg-orange-500/5 border border-orange-500/20 rounded-lg p-4">
        <p className="text-xs text-orange-400 font-medium mb-2">{t('comp.tradeoffs.notForYouTitle')}</p>
        <div className="space-y-1.5 text-xs text-text-muted">
          <p>{t('comp.tradeoffs.notForYou1')}</p>
          <p>{t('comp.tradeoffs.notForYou2')}</p>
          <p>{t('comp.tradeoffs.notForYou3')}</p>
          <p>{t('comp.tradeoffs.notForYou4')}</p>
        </div>
      </div>

      <div className="bg-green-500/5 border border-green-500/20 rounded-lg p-4">
        <p className="text-xs text-green-400 font-medium mb-2">{t('comp.tradeoffs.forYouTitle')}</p>
        <div className="space-y-1.5 text-xs text-text-muted">
          <p>{t('comp.tradeoffs.forYou1')}</p>
          <p>{t('comp.tradeoffs.forYou2')}</p>
          <p>{t('comp.tradeoffs.forYou3')}</p>
          <p>{t('comp.tradeoffs.forYou4')}</p>
        </div>
      </div>
    </div>
  );
}

interface ComparisonPageProps {
  onClose: () => void;
}

export const ComparisonPage = memo(function ComparisonPage({ onClose }: ComparisonPageProps) {
  const { t } = useTranslation();

  const SECTIONS: Array<{ id: Section; name: string; desc: string }> = [
    { id: 'approach', name: t('comp.nav.approach'), desc: t('comp.nav.approachDesc') },
    { id: 'privacy', name: t('comp.nav.privacy'), desc: t('comp.nav.privacyDesc') },
    { id: 'scoring', name: t('comp.nav.scoring'), desc: t('comp.nav.scoringDesc') },
    { id: 'context', name: t('comp.nav.context'), desc: t('comp.nav.contextDesc') },
    { id: 'compound', name: t('comp.nav.compound'), desc: t('comp.nav.compoundDesc') },
    { id: 'tradeoffs', name: t('comp.nav.tradeoffs'), desc: t('comp.nav.tradeoffsDesc') },
  ];

  const [activeSection, setActiveSection] = useState<Section>('approach');

  const renderSection = () => {
    switch (activeSection) {
      case 'approach': return <ApproachSection />;
      case 'privacy': return <PrivacySection />;
      case 'scoring': return <ScoringSection />;
      case 'context': return <ContextSection />;
      case 'compound': return <CompoundSection />;
      case 'tradeoffs': return <TradeoffsSection />;
    }
  };

  return (
    <div className="fixed inset-0 z-50 bg-bg-primary/95 backdrop-blur-sm flex flex-col">
      <div className="flex items-center justify-between px-6 py-4 border-b border-border">
        <div>
          <h2 className="text-lg font-semibold text-white">{t('comparison.title')}</h2>
          <p className="text-xs text-text-muted">{t('comparison.subtitle')}</p>
        </div>
        <button
          onClick={onClose}
          className="px-3 py-1.5 text-xs text-text-secondary hover:text-white border border-border rounded-lg hover:bg-bg-tertiary transition-all"
          aria-label={t('action.close')}
        >
          {t('action.close')}
        </button>
      </div>

      <div className="flex-1 flex overflow-hidden">
        <nav className="w-48 border-e border-border p-4 space-y-1 flex-shrink-0" aria-label={t('comp.nav.ariaLabel')}>
          {SECTIONS.map(({ id, name, desc }) => (
            <button
              key={id}
              onClick={() => setActiveSection(id)}
              className={`w-full text-start px-3 py-2 rounded-lg transition-all ${
                activeSection === id
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
