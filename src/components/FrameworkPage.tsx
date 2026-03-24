// Copyright (c) 2025-2026 4DA Systems Pty Ltd (ACN 696 078 841). All rights reserved.
// Licensed under the Functional Source License 1.1 (FSL-1.1-Apache-2.0). See LICENSE file.

import { memo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  OverviewSection,
  PasifaSection,
  AuthoritySection,
  AosSection,
  PrivacySection,
  CompoundSection,
} from './framework/FrameworkSections';
import { GeometryShowcase } from './geometry/GeometryShowcase';

type Section = 'overview' | 'pasifa' | 'authority' | 'aos' | 'privacy' | 'compound' | 'geometry';

const SECTIONS: Array<{ id: Section; labelKey: string }> = [
  { id: 'overview', labelKey: 'Overview' },
  { id: 'pasifa', labelKey: 'PASIFA Scoring' },
  { id: 'authority', labelKey: 'Authority Stack' },
  { id: 'aos', labelKey: 'Operations (AOS)' },
  { id: 'privacy', labelKey: 'Privacy Architecture' },
  { id: 'compound', labelKey: 'Compound Knowledge' },
  { id: 'geometry', labelKey: 'Platonic Architecture' },
];

interface FrameworkPageProps {
  onClose: () => void;
}

export const FrameworkPage = memo(function FrameworkPage({ onClose }: FrameworkPageProps) {
  const { t } = useTranslation();
  const [activeSection, setActiveSection] = useState<Section>('overview');

  const renderSection = () => {
    switch (activeSection) {
      case 'overview': return <OverviewSection />;
      case 'pasifa': return <PasifaSection />;
      case 'authority': return <AuthoritySection />;
      case 'aos': return <AosSection />;
      case 'privacy': return <PrivacySection />;
      case 'compound': return <CompoundSection />;
      case 'geometry': return <GeometryShowcase />;
    }
  };

  return (
    <div className="fixed inset-0 z-50 bg-bg-primary/95 backdrop-blur-sm flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between px-6 py-4 border-b border-border">
        <div>
          <h2 className="text-lg font-semibold text-white">The 4DA Framework</h2>
          <p className="text-xs text-text-muted">
            {t('about.frameworkSubtitle', { defaultValue: 'A Philosophy for Private Developer Intelligence' })}
          </p>
        </div>
        <div className="flex items-center gap-3">
          <button
            onClick={() => import('@tauri-apps/plugin-opener').then(({ openUrl }) => openUrl('https://4da.ai/framework'))}
            className="px-3 py-1.5 text-xs text-text-secondary hover:text-white border border-border rounded-lg hover:border-orange-500/30 transition-all"
          >
            Open on 4da.ai
          </button>
          <button
            onClick={onClose}
            className="px-3 py-1.5 text-xs text-text-secondary hover:text-white border border-border rounded-lg hover:bg-bg-tertiary transition-all"
            aria-label={t('action.close', { defaultValue: 'Close' })}
          >
            Close
          </button>
        </div>
      </div>

      {/* Navigation + Content */}
      <div className="flex-1 flex overflow-hidden">
        {/* Sidebar Navigation */}
        <nav className="w-48 border-e border-border p-4 space-y-1 flex-shrink-0" aria-label="Framework sections">
          {SECTIONS.map(({ id, labelKey }) => (
            <button
              key={id}
              onClick={() => setActiveSection(id)}
              className={`w-full text-start px-3 py-2 text-sm rounded-lg transition-all ${
                activeSection === id
                  ? 'bg-orange-500/15 text-orange-400 font-medium'
                  : 'text-text-muted hover:text-text-secondary hover:bg-bg-tertiary/50'
              }`}
            >
              {labelKey}
            </button>
          ))}
          <div className="pt-4 border-t border-border/50 mt-4">
            <p className="text-[10px] text-text-muted px-3 leading-relaxed">
              Published by 4DA Systems Pty Ltd. These ideas are published openly. The strongest
              competitive position is to become the standard.
            </p>
          </div>
        </nav>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-6 max-w-3xl">
          {renderSection()}
        </div>
      </div>
    </div>
  );
});

export default FrameworkPage;
