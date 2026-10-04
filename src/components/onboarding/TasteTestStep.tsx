// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useCallback, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { cmd } from '../../lib/commands';

import type { TasteProfileSummary } from '../../types/calibration';
import { TasteTestCard } from './TasteTestCard';
import { CalibrationSummary } from './CalibrationSummary';

interface TasteTestStepProps {
  isAnimating: boolean;
  onComplete: () => void;
  onSkip: () => void;
}

type Phase = 'intro' | 'cards' | 'finalizing' | 'complete';

interface CardState {
  id: number;
  slot: number;
  title: string;
  snippet: string;
  sourceHint: string;
  categoryHint: string;
}

export function TasteTestStep({ isAnimating, onComplete, onSkip }: TasteTestStepProps) {
  const { t } = useTranslation();
  const [phase, setPhase] = useState<Phase>('intro');
  const [currentCard, setCurrentCard] = useState<CardState | null>(null);
  const [progress, setProgress] = useState(0);
  const [confidence, setConfidence] = useState(0);
  const [summary, setSummary] = useState<TasteProfileSummary | null>(null);
  const [cardAnimating, setCardAnimating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);
  const cardShownAt = useRef<number>(0);

  // Every answer is already recorded; this only builds the profile from them,
  // so a failure here is retryable without re-asking any card.
  const finalize = useCallback(async () => {
    setPhase('finalizing');
    setError(null);
    try {
      setSummary(await cmd('taste_test_finalize'));
      setPhase('complete');
    } catch (e) {
      setError(t('tasteTest.finalizeFailed', { error: String(e) }));
    }
  }, [t]);

  const startTest = useCallback(async () => {
    setStarting(true);
    setError(null);
    try {
      const result = await cmd('taste_test_start');
      if (result.type === 'nextCard') {
        setCurrentCard(result.card);
        setProgress(result.progress);
        setConfidence(result.confidence);
        cardShownAt.current = Date.now();
        setPhase('cards');
      } else {
        await finalize();
      }
    } catch (e) {
      setError(t('tasteTest.startFailed', { error: String(e) }));
    } finally {
      setStarting(false);
    }
  }, [finalize, t]);

  const respond = useCallback(async (response: string) => {
    if (!currentCard) return;

    const responseTimeMs = cardShownAt.current > 0
      ? Date.now() - cardShownAt.current
      : undefined;

    setCardAnimating(true);
    setError(null);
    await new Promise(r => setTimeout(r, 150));

    try {
      const result = await cmd('taste_test_respond', {
        itemSlot: currentCard.slot,
        response,
        responseTimeMs,
      });

      if (result.type === 'nextCard') {
        setCurrentCard(result.card);
        setProgress(result.progress);
        setConfidence(result.confidence);
        cardShownAt.current = Date.now();
        setCardAnimating(false);
      } else {
        await finalize();
      }
    } catch (e) {
      setError(t('tasteTest.respondFailed', { error: String(e) }));
      setCardAnimating(false);
    }
  }, [currentCard, finalize, t]);

  // Keyboard navigation for taste test cards
  useEffect(() => {
    if (phase !== 'cards' || !currentCard || cardAnimating) return;
    const handler = (e: KeyboardEvent) => {
      // Don't capture if user is in an input
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      switch (e.key) {
        case 'ArrowRight':
        case 'y':
        case '1':
          e.preventDefault();
          void respond('interested');
          break;
        case 'ArrowLeft':
        case 'n':
        case '2':
          e.preventDefault();
          void respond('not_interested');
          break;
        case 'ArrowUp':
        case 's':
        case '3':
          e.preventDefault();
          void respond('strong_interest');
          break;
        case 'Escape':
          e.preventDefault();
          onSkip();
          break;
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [phase, currentCard, cardAnimating, respond, onSkip]);

  // Intro phase
  if (phase === 'intro') {
    return (
      <div className={`text-center space-y-6 transition-opacity duration-300 ${isAnimating ? 'opacity-0' : 'opacity-100'}`}>
        <div className="text-4xl mb-2" aria-hidden="true">&#x1f3af;</div>
        <h2 className="text-xl font-semibold text-text-primary">
          {t('tasteTest.introTitle')}
        </h2>
        <p className="text-text-secondary text-sm max-w-md mx-auto">
          {t('tasteTest.introBody')}
        </p>
        {error && <p role="alert" className="text-red-400 text-xs">{error}</p>}
        <div className="flex items-center justify-center gap-4 pt-2">
          <button
            onClick={() => { void startTest(); }}
            disabled={starting}
            className="bg-white text-black font-medium text-sm py-2.5 px-6 rounded-md hover:bg-gray-100 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {starting ? (
              <span className="flex items-center gap-2">
                <span className="w-3.5 h-3.5 border-2 border-black/30 border-t-black rounded-full animate-spin" />
                {t('tasteTest.starting')}
              </span>
            ) : t('tasteTest.start')}
          </button>
          <button
            onClick={onSkip}
            className="text-text-muted text-sm hover:text-text-secondary transition-colors"
          >
            {t('onboarding.nav.skipForNow')}
          </button>
        </div>
      </div>
    );
  }

  // Cards phase
  if (phase === 'cards' && currentCard) {
    const kbd = 'px-1 py-0.5 bg-bg-tertiary rounded text-[9px]';
    return (
      <div className="space-y-4">
        {/* Progress bar */}
        <div className="mb-2">
          <div className="flex items-center gap-3">
            <div className="flex-1 bg-bg-tertiary rounded-full h-1.5 overflow-hidden">
              <div
                className="bg-white h-full rounded-full transition-all duration-300"
                style={{ width: `${Math.round(progress * 100)}%` }}
              />
            </div>
            <span className="text-xs text-text-muted">
              {t('tasteTest.confident', { percent: Math.round(confidence * 100) })}
            </span>
          </div>
          <p className="text-[10px] text-text-muted mt-1 text-end">
            {confidence < 0.3
              ? t('tasteTest.keepGoing')
              : confidence < 0.7
                ? t('tasteTest.goodStart')
                : t('tasteTest.strongCalibration')
            }
          </p>
        </div>

        {error && <p role="alert" className="text-red-400 text-xs">{error}</p>}

        <TasteTestCard
          card={currentCard}
          onInterested={() => { void respond('interested'); }}
          onSkip={() => { void respond('not_interested'); }}
          onStrongInterest={() => { void respond('strong_interest'); }}
          isAnimating={cardAnimating}
        />

        <div className="text-center space-y-2">
          {/* eslint-disable i18next/no-literal-string -- key glyphs, not words */}
          <p className="text-[10px] text-text-muted/60">
            {t('tasteTest.keyboard')} <kbd className={kbd}>&rarr;</kbd> {t('tasteTest.keyInterested')} &middot; <kbd className={kbd}>&larr;</kbd> {t('tasteTest.keySkip')} &middot; <kbd className={kbd}>&uarr;</kbd> {t('tasteTest.keyLove')} &middot; <kbd className={kbd}>Esc</kbd> {t('tasteTest.keyDone')}
          </p>
          {/* eslint-enable i18next/no-literal-string */}
          <button
            onClick={onSkip}
            className="text-text-muted text-xs hover:text-text-secondary transition-colors"
          >
            {t('tasteTest.skipCalibration')}
          </button>
        </div>
      </div>
    );
  }

  // Finalizing phase — or its failure, which offers a retry instead of a dead end
  if (phase === 'finalizing') {
    if (error) {
      return (
        <div className="text-center space-y-4">
          <p role="alert" className="text-red-400 text-sm">{error}</p>
          <div className="flex items-center justify-center gap-4">
            <button
              onClick={() => { void finalize(); }}
              className="bg-white text-black font-medium text-sm py-2 px-5 rounded-md hover:bg-gray-100 transition-colors"
            >
              {t('action.retry')}
            </button>
            <button
              onClick={onSkip}
              className="text-text-muted text-sm hover:text-text-secondary transition-colors"
            >
              {t('tasteTest.skipCalibration')}
            </button>
          </div>
        </div>
      );
    }
    return (
      <div className="text-center space-y-4" role="status">
        <div className="animate-spin w-8 h-8 border-2 border-white border-t-transparent rounded-full mx-auto" />
        <p className="text-text-secondary text-sm">{t('tasteTest.analyzing')}</p>
      </div>
    );
  }

  // Complete phase
  if (phase === 'complete' && summary) {
    return <CalibrationSummary summary={summary} onContinue={onComplete} />;
  }

  return null;
}
