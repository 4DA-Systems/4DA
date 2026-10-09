// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { cmd } from '../../lib/commands';
import type { TasteProfileSummary } from '../../types/calibration';

interface CalibrationSummaryProps {
  summary: TasteProfileSummary;
  onContinue: () => void;
}

export function CalibrationSummary({ summary, onContinue }: CalibrationSummaryProps) {
  const { t } = useTranslation();
  const confidencePct = Math.round(summary.confidence * 100);

  // Two kinds of interest, said honestly: topics of the cards the user LIKED
  // (their own answers, saved as their choice) and GUESSES from the closest
  // persona (saved as inferred at reduced weight). Fresh-profile E2E
  // 2026-10-09: one undivided list headed "These came from your responses"
  // held five ML guesses for a user who had skipped the ML card.
  // Each change is saved first and shown only once saved, so the lists on
  // screen are always what the feed will actually use.
  const [liked, setLiked] = useState<string[]>(summary.likedInterests ?? summary.topInterests);
  const [guessed, setGuessed] = useState<string[]>(summary.guessedInterests ?? []);
  const [draft, setDraft] = useState('');
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const addInputId = useId();

  const removeInterest = async (topic: string) => {
    if (saving) return;
    setSaving(true);
    setSaveError(null);
    try {
      await cmd('remove_interest', { topic });
      setLiked((prev) => prev.filter((i) => i !== topic));
      setGuessed((prev) => prev.filter((i) => i !== topic));
    } catch (e) {
      setSaveError(t('onboarding.calib.saveFailed', { error: String(e) }));
    } finally {
      setSaving(false);
    }
  };

  const addInterest = async () => {
    const topic = draft.trim();
    if (saving || !topic) return;
    if (liked.some((i) => i.toLowerCase() === topic.toLowerCase())) {
      setDraft('');
      return;
    }
    setSaving(true);
    setSaveError(null);
    try {
      // Typed by the user: their own choice, full weight. A guess they type
      // again is promoted out of the guess list.
      await cmd('add_interest', { topic });
      setLiked((prev) => [...prev, topic]);
      setGuessed((prev) => prev.filter((i) => i.toLowerCase() !== topic.toLowerCase()));
      setDraft('');
    } catch (e) {
      // The draft is kept so the user can retry without retyping.
      setSaveError(t('onboarding.calib.saveFailed', { error: String(e) }));
    } finally {
      setSaving(false);
    }
  };

  const chip = (interest: string, isGuess: boolean) => (
    <span
      key={interest}
      data-guess={isGuess || undefined}
      className={`inline-flex items-center gap-1.5 text-xs px-2.5 py-1 rounded-md ${
        isGuess
          ? 'border border-dashed border-border text-text-muted'
          : 'bg-bg-tertiary text-text-secondary'
      }`}
    >
      {interest}
      <button
        onClick={() => { void removeInterest(interest); }}
        disabled={saving}
        aria-label={t('onboarding.calib.removeAria', { interest })}
        className="text-text-muted hover:text-error transition-colors leading-none text-sm"
      >
        <span aria-hidden="true">{'✕'}</span>
      </button>
    </span>
  );

  return (
    <div className="space-y-6 animate-in fade-in duration-300">
      {/* Header */}
      <div className="text-center">
        <h2 className="text-xl font-semibold text-text-primary mb-2">{t('onboarding.calib.title')}</h2>
        <p className="text-text-secondary text-sm">
          {t('onboarding.calib.basedOn', { count: summary.itemsShown, confidence: confidencePct })}
        </p>
      </div>

      {/* Closest persona — never presented as "you" when the answers rule it out */}
      <div className="bg-bg-secondary border border-border rounded-lg p-5">
        <div className="text-xs text-text-muted uppercase tracking-wider mb-2">
          {t('onboarding.calib.developerProfile')}
        </div>
        {summary.personaContradicted ? (
          <>
            <h3 className="text-text-primary font-medium text-lg mb-1">{t('onboarding.calib.noClearProfile')}</h3>
            <p className="text-text-secondary text-sm">{t('onboarding.calib.noClearProfileBody')}</p>
          </>
        ) : (
          <>
            <h3 className="text-text-primary font-medium text-lg mb-1">{summary.dominantPersonaName}</h3>
            <p className="text-text-secondary text-sm">{summary.dominantPersonaDescription}</p>
          </>
        )}
      </div>

      {/* Persona blend bar chart */}
      {summary.personaWeights.length > 1 && (
        <div className="bg-bg-secondary border border-border rounded-lg p-5">
          <div className="text-xs text-text-muted uppercase tracking-wider mb-3">{t('onboarding.calib.personaBlend')}</div>
          <div className="space-y-2">
            {[...summary.personaWeights]
              .sort((a, b) => b.weight - a.weight)
              .map((pw) => (
                <div key={pw.name} className="flex items-center gap-3">
                  <span className="text-xs text-text-secondary w-40 truncate">{pw.name}</span>
                  <div className="flex-1 bg-bg-tertiary rounded-full h-2 overflow-hidden">
                    <div
                      className="bg-accent-primary h-full rounded-full transition-all duration-500"
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

      {/* Interests — editable (remove what doesn't fit, add your own) */}
      <div className="bg-bg-secondary border border-border rounded-lg p-5">
        <div className="flex items-center justify-between mb-1">
          <div className="text-xs text-text-muted uppercase tracking-wider">{t('onboarding.calib.detectedInterests')}</div>
          <span className="text-[10px] text-text-muted/70">{t('onboarding.calib.makeItYours')}</span>
        </div>
        <p className="text-[11px] text-text-muted mb-3">
          {t('onboarding.calib.interestsHint')}
        </p>
        <div className="flex flex-wrap gap-2 mb-3">
          {liked.length === 0 && (
            <span className="text-xs text-text-muted">{t('onboarding.calib.noInterests')}</span>
          )}
          {liked.map((interest) => chip(interest, false))}
        </div>
        {guessed.length > 0 && (
          <div className="mb-3">
            <div className="text-[11px] text-text-secondary">{t('onboarding.calib.guessesLabel')}</div>
            <p className="text-[11px] text-text-muted mb-2">{t('onboarding.calib.guessesHint')}</p>
            <div className="flex flex-wrap gap-2">
              {guessed.map((interest) => chip(interest, true))}
            </div>
          </div>
        )}
        {saveError && (
          <p role="alert" className="text-xs text-red-400 mb-2">{saveError}</p>
        )}
        <div className="flex gap-2">
          <label htmlFor={addInputId} className="sr-only">{t('onboarding.calib.addPlaceholder')}</label>
          <input
            id={addInputId}
            type="text"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault();
                void addInterest();
              }
            }}
            placeholder={t('onboarding.calib.addPlaceholder')}
            className="flex-1 bg-bg-primary border border-border rounded-md px-2.5 py-1.5 text-xs text-text-primary placeholder-text-muted focus:border-orange-500 focus:outline-none"
          />
          <button
            onClick={() => { void addInterest(); }}
            disabled={!draft.trim() || saving}
            className="px-3 py-1.5 text-xs font-medium bg-bg-tertiary text-text-secondary border border-border rounded-md hover:text-text-primary hover:border-gray-500 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
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
