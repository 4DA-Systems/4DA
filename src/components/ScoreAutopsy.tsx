import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';

interface ScoreAutopsyProps {
  itemId: number;
  sourceType: string;
  currentScore: number;
}

interface AutopsyResult {
  item: {
    id: number;
    title: string;
    url: string | null;
    source_type: string;
    created_at: string;
    age_hours: number;
  };
  final_score: number;
  components: Array<{
    name: string;
    raw_value: number;
    weight: number;
    contribution: number;
    explanation: string;
  }>;
  matching_context: {
    interests: string[];
    tech_stack: string[];
    active_topics: string[];
    learned_affinities: string[];
    exclusions_hit: string[];
  };
  similar_items: Array<{
    id: number;
    title: string;
    score: number;
    score_difference: number;
    key_difference: string;
  }>;
  recommendations: string[];
  narrative: string;
  ai_analysis?: {
    verdict: string;
    score_assessment: string;
    reasoning: string;
    suggested_action: string;
    model_used: string;
  };
}

export const ScoreAutopsy: React.FC<ScoreAutopsyProps> = ({
  itemId,
  sourceType,
}) => {
  const { t } = useTranslation();
  const [autopsy, setAutopsy] = useState<AutopsyResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const runAutopsy = async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<AutopsyResult>('mcp_score_autopsy', {
        itemId,
        sourceType,
        synthesize: true,
        compact: false,
      });
      setAutopsy(result);
    } catch (err) {
      console.error('Autopsy failed:', err);
      setError(err instanceof Error ? err.message : t('autopsy.failed'));
    } finally {
      setLoading(false);
    }
  };

  if (!autopsy) {
    return (
      <div className="score-autopsy-trigger">
        <button
          onClick={runAutopsy}
          disabled={loading}
          className="autopsy-button px-3 py-1.5 text-xs bg-bg-secondary text-text-primary border border-border rounded hover:bg-bg-tertiary transition-colors font-medium disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {loading ? (
            <span className="flex items-center gap-1.5">
              <span className="w-3 h-3 border-2 border-white/30 border-t-white rounded-full animate-spin" />
              {t('action.analyzing')}
            </span>
          ) : t('autopsy.title')}
        </button>
        {error && (
          <div className="error-message mt-2 text-error text-xs">{error}</div>
        )}
      </div>
    );
  }

  return (
    <div className="score-autopsy-results mt-4 p-4 bg-bg-secondary border border-border rounded">
      {/* AI Verdict */}
      {autopsy.ai_analysis && (
        <div
          className={`ai-verdict p-3 rounded mb-4 border-l-4 ${
            autopsy.ai_analysis.score_assessment === 'accurate'
              ? 'border-l-success bg-success/10'
              : autopsy.ai_analysis.score_assessment === 'too_high'
              ? 'border-l-[#F59E0B] bg-[#F59E0B]/10'
              : autopsy.ai_analysis.score_assessment === 'too_low'
              ? 'border-l-[#3B82F6] bg-[#3B82F6]/10'
              : 'border-l-text-muted bg-text-muted/10'
          }`}
        >
          <div className="verdict-header flex items-center gap-2 mb-2">
            <span className="verdict-icon">🤖</span>
            <strong className="text-text-primary text-sm">{t('autopsy.aiAssessment')}</strong>
          </div>
          <p className="verdict-text text-xs text-text-primary mb-2">
            {autopsy.ai_analysis.verdict}
          </p>
          <p className="verdict-reasoning text-xs text-text-secondary">
            {autopsy.ai_analysis.reasoning}
          </p>
        </div>
      )}

      {/* Narrative Summary */}
      <div className="narrative-summary mb-4 p-3 bg-bg-primary rounded">
        <p className="text-xs text-text-secondary">{autopsy.narrative}</p>
      </div>

      {/* Component Breakdown */}
      <div className="components-section mb-4">
        <h4 className="text-sm font-medium text-text-primary mb-3">
          {t('autopsy.components')}
        </h4>
        {autopsy.components.map((comp, idx) => (
          <div key={idx} className="component-item mb-3">
            <div className="component-header flex justify-between mb-1 text-xs">
              <span className="component-name text-text-primary">
                {comp.name}
              </span>
              <span className="component-contribution text-text-secondary font-semibold">
                {(comp.contribution * 100).toFixed(1)}%
              </span>
            </div>
            <div className="component-bar-container h-2 bg-bg-primary rounded overflow-hidden mb-1">
              <div
                className="component-bar h-full transition-all duration-300"
                style={{
                  width: `${Math.abs(comp.contribution) * 100}%`,
                  backgroundColor:
                    comp.contribution >= 0 ? 'var(--color-success)' : 'var(--color-error)',
                }}
              />
            </div>
            <p className="component-explanation text-xs text-text-secondary">
              {comp.explanation}
            </p>
          </div>
        ))}
      </div>

      {/* Matching Context */}
      <div className="matching-context-section mb-4">
        <h4 className="text-sm font-medium text-text-primary mb-3">
          {t('autopsy.whatMatched')}
        </h4>
        {autopsy.matching_context.interests.length > 0 && (
          <div className="context-group mb-2 text-xs">
            <strong className="text-text-primary">{t('autopsy.interests')}:</strong>{' '}
            <span className="text-text-secondary">
              {autopsy.matching_context.interests.join(', ')}
            </span>
          </div>
        )}
        {autopsy.matching_context.tech_stack.length > 0 && (
          <div className="context-group mb-2 text-xs">
            <strong className="text-text-primary">{t('autopsy.techStack')}:</strong>{' '}
            <span className="text-text-secondary">
              {autopsy.matching_context.tech_stack.join(', ')}
            </span>
          </div>
        )}
        {autopsy.matching_context.active_topics.length > 0 && (
          <div className="context-group mb-2 text-xs">
            <strong className="text-text-primary">{t('autopsy.recentWork')}:</strong>{' '}
            <span className="text-text-secondary">
              {autopsy.matching_context.active_topics.join(', ')}
            </span>
          </div>
        )}
        {autopsy.matching_context.learned_affinities.length > 0 && (
          <div className="context-group mb-2 text-xs">
            <strong className="text-text-primary">{t('autopsy.learnedPreferences')}:</strong>{' '}
            <span className="text-text-secondary">
              {autopsy.matching_context.learned_affinities.join(', ')}
            </span>
          </div>
        )}
      </div>

      {/* Recommendations */}
      {autopsy.recommendations.length > 0 && (
        <div className="recommendations-section mb-4">
          <h4 className="text-sm font-medium text-text-primary mb-3">
            {t('autopsy.howToImprove')}
          </h4>
          <ul className="list-none p-0">
            {autopsy.recommendations.map((rec, idx) => (
              <li
                key={idx}
                className="p-2 bg-bg-primary border-l-[3px] border-l-accent-gold mb-2 text-xs text-text-secondary"
              >
                {rec}
              </li>
            ))}
          </ul>
        </div>
      )}

      {/* Similar Items Comparison */}
      {autopsy.similar_items.length > 0 && (
        <div className="similar-items-section mb-4">
          <h4 className="text-sm font-medium text-text-primary mb-3">
            {t('autopsy.comparedToSimilar')}
          </h4>
          {autopsy.similar_items.map((similar, idx) => (
            <div
              key={idx}
              className="similar-item p-3 bg-bg-primary rounded mb-2"
            >
              <div className="similar-title text-xs text-text-primary mb-1">
                {similar.title}
              </div>
              <div className="similar-score flex gap-2 items-center text-xs text-text-secondary">
                <span>Score: {(similar.score * 100).toFixed(0)}%</span>
                <span
                  className={`score-diff font-semibold ${
                    similar.score_difference >= 0
                      ? 'text-success'
                      : 'text-error'
                  }`}
                >
                  {similar.score_difference >= 0 ? '+' : ''}
                  {(similar.score_difference * 100).toFixed(0)}%
                </span>
              </div>
              <div className="similar-explanation text-xs text-text-muted mt-1">
                {similar.key_difference}
              </div>
            </div>
          ))}
        </div>
      )}

      <button
        onClick={() => setAutopsy(null)}
        className="close-autopsy-button mt-4 bg-bg-tertiary text-text-secondary border-none px-4 py-2 rounded cursor-pointer text-xs hover:bg-bg-primary transition-colors"
      >
        {t('autopsy.close')}
      </button>
    </div>
  );
};
