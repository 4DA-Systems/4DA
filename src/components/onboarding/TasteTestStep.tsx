import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';

import type { TasteTestStepResult, TasteProfileSummary } from '../../types/calibration';
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
  title: string;
  snippet: string;
  sourceHint: string;
  categoryHint: string;
  slot: number;
}

// Map card IDs to slot indices (matches items.rs SLOT_TO_CORPUS_ID)
const SLOT_MAP: Record<number, number> = {
  1: 0, 11: 1, 16: 2, 28: 3, 19: 4, 21: 5, 4: 6,
  24: 7, 6: 8, 96: 9, 8: 10, 17: 11, 45: 12, 91: 13, 142: 14,
};

export function TasteTestStep({ isAnimating, onComplete, onSkip }: TasteTestStepProps) {
  const [phase, setPhase] = useState<Phase>('intro');
  const [currentCard, setCurrentCard] = useState<CardState | null>(null);
  const [progress, setProgress] = useState(0);
  const [confidence, setConfidence] = useState(0);
  const [summary, setSummary] = useState<TasteProfileSummary | null>(null);
  const [cardAnimating, setCardAnimating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const startTest = useCallback(async () => {
    try {
      const result = await invoke<TasteTestStepResult>('taste_test_start');
      if (result.type === 'nextCard') {
        const slot = SLOT_MAP[result.card.id] ?? 0;
        setCurrentCard({ ...result.card, slot });
        setProgress(result.progress);
        setConfidence(result.confidence);
        setPhase('cards');
      }
    } catch (e) {
      setError(`Failed to start taste test: ${e}`);
    }
  }, []);

  const respond = useCallback(async (response: string) => {
    if (!currentCard) return;

    setCardAnimating(true);
    await new Promise(r => setTimeout(r, 150));

    try {
      const result = await invoke<TasteTestStepResult>('taste_test_respond', {
        itemSlot: currentCard.slot,
        response,
      });

      if (result.type === 'nextCard') {
        const slot = SLOT_MAP[result.card.id] ?? 0;
        setCurrentCard({ ...result.card, slot });
        setProgress(result.progress);
        setConfidence(result.confidence);
        setCardAnimating(false);
      } else if (result.type === 'complete') {
        setPhase('finalizing');
        try {
          const finalSummary = await invoke<TasteProfileSummary>('taste_test_finalize');
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

  // Intro phase
  if (phase === 'intro') {
    return (
      <div className={`text-center space-y-6 transition-opacity duration-300 ${isAnimating ? 'opacity-0' : 'opacity-100'}`}>
        <div className="text-4xl mb-2">&#x1f3af;</div>
        <h2 className="text-xl font-semibold text-white">
          Let's calibrate your feed
        </h2>
        <p className="text-text-secondary text-sm max-w-md mx-auto">
          We'll show you up to 15 articles. Just tell us which ones you'd read.
          This takes about 60 seconds and dramatically improves your content recommendations.
        </p>
        {error && <p className="text-red-400 text-xs">{error}</p>}
        <div className="flex items-center justify-center gap-4 pt-2">
          <button
            onClick={startTest}
            className="bg-white text-black font-medium text-sm py-2.5 px-6 rounded-md hover:bg-gray-100 transition-colors"
          >
            Start calibration
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
        <div className="flex items-center gap-3 mb-2">
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

        {error && <p className="text-red-400 text-xs">{error}</p>}

        <TasteTestCard
          card={currentCard}
          onInterested={() => respond('interested')}
          onSkip={() => respond('not_interested')}
          onStrongInterest={() => respond('strong_interest')}
          isAnimating={cardAnimating}
        />

        <div className="text-center">
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
