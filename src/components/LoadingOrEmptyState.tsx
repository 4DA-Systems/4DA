// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useTranslation } from 'react-i18next';

import { SmartEmptyState } from './SmartEmptyState';
import { getStageLabel } from '../utils/score';
import { PlatonicSVG } from './geometry/PlatonicSVG';
import { TETRAHEDRON } from './geometry/geometries';

interface LoadingOrEmptyStateProps {
  loading: boolean;
  progressMessage: string;
  progress: number;
  progressStage: string;
  detectedStack: string[];
  onStartAnalysis: () => void;
}

export function LoadingOrEmptyState({
  loading,
  progressMessage,
  progress,
  progressStage,
  detectedStack,
  onStartAnalysis,
}: LoadingOrEmptyStateProps) {
  const { t } = useTranslation();

  if (loading) {
    return (
      <div className="text-center py-16" role="status" aria-busy>
        <div className="w-16 h-16 mx-auto mb-4 bg-orange-500/20 rounded-full flex items-center justify-center">
          <div className="w-8 h-8 border-3 border-orange-500 border-t-transparent rounded-full animate-spin" />
        </div>
        <p className="text-lg text-text-primary mb-2">{t('action.analyzing')}</p>
        <p className="text-sm text-text-muted">{progressMessage}</p>
        {progress > 0 && (
          <div className="mt-4 max-w-xs mx-auto">
            <div className="flex justify-between text-xs text-text-muted mb-1">
              <span>{getStageLabel(progressStage)}</span>
              <span>{Math.round(progress * 100)}%</span>
            </div>
            <div className="w-full h-2 bg-bg-tertiary rounded-full overflow-hidden">
              <div
                className="h-full bg-gradient-to-r from-orange-600 to-orange-400 transition-all duration-300 ease-out rounded-full"
                style={{ width: `${progress * 100}%` }}
              />
            </div>
          </div>
        )}
      </div>
    );
  }

  return (
    <div className="text-center py-16" role="status" aria-busy={false}>
      <div className="w-16 h-16 mx-auto mb-4 rounded-xl border border-border/30 overflow-hidden" role="img" aria-label="4DA">
        <PlatonicSVG {...TETRAHEDRON} size={64} />
      </div>
      <p className="text-lg text-text-primary mb-2">{t('results.noResults')}</p>
      <p className="text-sm text-text-muted mb-3">
        {t('results.startAnalysis')}
      </p>
      <p className="text-xs text-text-muted/70 mb-5 max-w-md mx-auto leading-relaxed">
        {t('results.howItWorks')}
      </p>
      <SmartEmptyState detectedStack={detectedStack} />
      <button
        onClick={onStartAnalysis}
        className="mt-5 px-6 py-2.5 bg-orange-500 text-white text-sm font-medium rounded-lg hover:bg-orange-600 transition-colors"
      >
        {t('results.analyzeNow')}
      </button>
      {/* eslint-disable-next-line i18next/no-literal-string */}
      <p className="text-xs text-text-muted mt-3">
        or press <kbd className="px-1.5 py-0.5 bg-bg-tertiary rounded text-text-muted">R</kbd>
      </p>
      <p className="text-[10px] text-text-muted/50 mt-2">
        {t('results.analyzeHint')}
      </p>
    </div>
  );
}
