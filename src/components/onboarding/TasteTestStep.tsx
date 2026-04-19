// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useCallback, useRef, useEffect } from 'react';
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
  const [phase, setPhase] = useState<Phase>('intro');
  const [currentCard, setCurrentCard] = useState<CardState | null>(null);
  const [progress, setProgress] = useState(0);
  const [confidence, setConfidence] = useState(0);
  const [summary, setSummary] = useState<TasteProfileSummary | null>(null);
  const [cardAnimating, setCardAnimating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);
  const cardShownAt = useRef<number>(0);

  const startTest = useCallback(async () => {
    setStarting(true);
    try {
      const result = await cmd('taste_test_start');
      if (result.type === 'nextCard') {
        setCurrentCard(result.card);
        setProgress(result.progress);
        setConfidence(result.confidence);
        cardShownAt.current = Date.now();
        setPhase('cards');
      }
    } catch (e) {
      setError(`Failed to start taste test: ${e}`);
      setStarting(false);
    }
  }, []);

  const respond = useCallback(async (response: string) => {
    if (!currentCard) return;

    const responseTimeMs = cardShownAt.current > 0
      ? Date.now() - cardShownAt.current
      : undefined;

    setCardAnimating(true);
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
      } else if (result.type === 'complete') {
        setPhase('finalizing');
        try {
          const finalSummary = await cmd('taste_test_finalize');
          setSummary(finalSummary);
          setPhase('complete');
        } catch (e) {
          setError(`Failed to finalize: ${e}`);
          setPhase('cards');
        }
      }
    } catch (e) {
      setError(`Failed to respond: ${e}`);
      setCardAnimating(false);
    }
  }, [currentCard]);

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
          respond('interested');
          break;
        case 'ArrowLeft':
        case 'n':
        case '2':
          e.preventDefault();
          respond('not_interested');
          break;
        case 'ArrowUp':
        case 's':
        case '3':
          e.preventDefault();
          respond('strong_interest');
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
        <div className="text-4xl mb-2">&#x1f3af;</div>
        <h2 className="text-xl font-semibold text-white">
          Let's calibrate your feed
        </h2>
        <p className="text-text-secondary text-sm max-w-md mx-auto">
          We'll show you up to 15 articles — just tell us which ones you'd read.
          Your responses train 4DA's scoring engine so it surfaces content that matches your interests,
          and helps AI tools working on your behalf make better decisions.
        </p>
        {error && <p className="text-red-400 text-xs">{error}</p>}
        <div className="flex items-center justify-center gap-4 pt-2">
          <button
            onClick={startTest}
            disabled={starting}
            className="bg-white text-black font-medium text-sm py-2.5 px-6 rounded-md hover:bg-gray-100 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {starting ? (
              <span className="flex items-center gap-2">
                <span className="w-3.5 h-3.5 border-2 border-black/30 border-t-black rounded-full animate-spin" />
                Starting...
              </span>
            ) : 'Start calibration'}
          </button>
          <button
            onClick={onSkip}
            className="text-text-muted text-sm hover:text-text-secondary transition-colors"
          >
            Skip for now
          </button>
        </div>
      </div>
    );
  }

  // Cards phase
  if (phase === 'cards' && currentCard) {
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
              {Math.round(confidence * 100)}% confident
            </span>
          </div>
          <p className="text-[10px] text-text-muted mt-1 text-right">
            {confidence < 0.3
              ? 'Keep going \u2014 more responses improve accuracy'
              : confidence < 0.7
                ? 'Good start \u2014 a few more will help'
                : 'Strong calibration \u2014 you can continue or finish now'
            }
          </p>
        </div>

        {error && <p className="text-red-400 text-xs">{error}</p>}

        <TasteTestCard
          card={currentCard}
          onInterested={() => respond('interested')}
          onSkip={() => respond('not_interested')}
          onStrongInterest={() => respond('strong_interest')}
          isAnimating={cardAnimating}
        />

        <div className="text-center space-y-2">
          <p className="text-[10px] text-text-muted/60">
            Keyboard: <kbd className="px-1 py-0.5 bg-bg-tertiary rounded text-[9px]">&rarr;</kbd> interested &middot; <kbd className="px-1 py-0.5 bg-bg-tertiary rounded text-[9px]">&larr;</kbd> skip &middot; <kbd className="px-1 py-0.5 bg-bg-tertiary rounded text-[9px]">&uarr;</kbd> love &middot; <kbd className="px-1 py-0.5 bg-bg-tertiary rounded text-[9px]">Esc</kbd> done
          </p>
          <button
            onClick={onSkip}
            className="text-text-muted text-xs hover:text-text-secondary transition-colors"
          >
            Skip calibration
          </button>
        </div>
      </div>
    );
  }

  // Finalizing phase
  if (phase === 'finalizing') {
    return (
      <div className="text-center space-y-4">
        <div className="animate-spin w-8 h-8 border-2 border-white border-t-transparent rounded-full mx-auto" />
        <p className="text-text-secondary text-sm">Analyzing your preferences...</p>
      </div>
    );
  }

  // Complete phase
  if (phase === 'complete' && summary) {
    return <CalibrationSummary summary={summary} onContinue={onComplete} />;
  }

  return null;
}
