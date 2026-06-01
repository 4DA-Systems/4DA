// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { cmd } from '../../lib/commands';

interface PersonaWeight {
  name: string;
  weight: number;
}

interface TasteProfileSummary {
  dominantPersonaName: string;
  dominantPersonaDescription: string;
  confidence: number;
  itemsShown: number;
  personaWeights: PersonaWeight[];
  topInterests: string[];
}

interface CalibrationSummaryProps {
  summary: TasteProfileSummary;
  onContinue: () => void;
}

export function CalibrationSummary({ summary, onContinue }: CalibrationSummaryProps) {
  const { t } = useTranslation();
  const confidencePct = Math.round(summary.confidence * 100);

  // Detected interests are editable: the taste test is a guess, and users
  // should be able to correct what was surfaced before it shapes their feed.
  // Each change persists immediately via add_interest/remove_interest.
  const [interests, setInterests] = useState<string[]>(summary.topInterests);
  const [draft, setDraft] = useState('');

  const removeInterest = (topic: string) => {
    setInterests((prev) => prev.filter((i) => i !== topic));
    void cmd('remove_interest', { topic }).catch(() => {});
  };

  const addInterest = () => {
    const topic = draft.trim();
    setDraft('');
    if (!topic || interests.some((i) => i.toLowerCase() === topic.toLowerCase())) return;
    setInterests((prev) => [...prev, topic]);
    void cmd('add_interest', { topic }).catch(() => {});
  };

  return (
    <div className="space-y-6 animate-in fade-in duration-300">
      {/* Header */}
      <div className="text-center">
        <h2 className="text-xl font-semibold text-white mb-2">{t('onboarding.calib.title')}</h2>
        <p className="text-text-secondary text-sm">
          {t('onboarding.calib.basedOn', { count: summary.itemsShown, confidence: confidencePct })}
        </p>
      </div>

      {/* Dominant persona */}
      <div className="bg-bg-secondary border border-border rounded-lg p-5">
        <div className="text-xs text-text-muted uppercase tracking-wider mb-2">
          {t('onboarding.calib.developerProfile')}
        </div>
        <h3 className="text-white font-medium text-lg mb-1">{summary.dominantPersonaName}</h3>
        <p className="text-text-secondary text-sm">{summary.dominantPersonaDescription}</p>
      </div>

      {/* Persona blend bar chart */}
      {summary.personaWeights.length > 1 && (
        <div className="bg-bg-secondary border border-border rounded-lg p-5">
          <div className="text-xs text-text-muted uppercase tracking-wider mb-3">{t('onboarding.calib.personaBlend')}</div>
          <div className="space-y-2">
            {summary.personaWeights
              .sort((a, b) => b.weight - a.weight)
              .map((pw) => (
                <div key={pw.name} className="flex items-center gap-3">
                  <span className="text-xs text-text-secondary w-40 truncate">{pw.name}</span>
                  <div className="flex-1 bg-bg-tertiary rounded-full h-2 overflow-hidden">
                    <div
                      className="bg-white h-full rounded-full transition-all duration-500"
                      style={{ width: `${Math.round(pw.weight * 100)}%` }}
                    />
                  </div>
                  <span className="text-xs text-text-muted w-10 text-end">
                    {Math.round(pw.weight * 100)}%
                  </span>
                </div>
              ))}
          </div>
        </div>
      )}

      {/* Detected interests — editable (remove what doesn't fit, add your own) */}
      <div className="bg-bg-secondary border border-border rounded-lg p-5">
        <div className="flex items-center justify-between mb-1">
          <div className="text-xs text-text-muted uppercase tracking-wider">{t('onboarding.calib.detectedInterests')}</div>
          <span className="text-[10px] text-text-muted/70">{t('onboarding.calib.makeItYours')}</span>
        </div>
        <p className="text-[11px] text-text-muted mb-3">
          {t('onboarding.calib.interestsHint')}
        </p>
        <div className="flex flex-wrap gap-2 mb-3">
          {interests.length === 0 && (
            <span className="text-xs text-text-muted">{t('onboarding.calib.noInterests')}</span>
          )}
          {interests.map((interest) => (
            <span
              key={interest}
              className="inline-flex items-center gap-1.5 text-xs bg-bg-tertiary text-text-secondary px-2.5 py-1 rounded-md"
            >
              {interest}
              <button
                onClick={() => removeInterest(interest)}
                aria-label={t('onboarding.calib.removeAria', { interest })}
                className="text-text-muted hover:text-error transition-colors leading-none text-sm"
              >
                <span aria-hidden="true">{'✕'}</span>
              </button>
            </span>
          ))}
        </div>
        <div className="flex gap-2">
          <input
            type="text"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault();
                addInterest();
              }
            }}
            placeholder={t('onboarding.calib.addPlaceholder')}
            className="flex-1 bg-bg-primary border border-border rounded-md px-2.5 py-1.5 text-xs text-white placeholder-text-muted focus:border-orange-500 focus:outline-none"
          />
          <button
            onClick={addInterest}
            disabled={!draft.trim()}
            className="px-3 py-1.5 text-xs font-medium bg-bg-tertiary text-text-secondary border border-border rounded-md hover:text-white hover:border-gray-500 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          >
            {t('onboarding.calib.add')}
          </button>
        </div>
      </div>

      {/* Continue button */}
      <button
        onClick={onContinue}
        className="w-full bg-orange-500 hover:bg-orange-600 text-white font-medium py-3 rounded-lg transition-colors"
      >
        {t('onboarding.nav.continue')}
      </button>
    </div>
  );
}
