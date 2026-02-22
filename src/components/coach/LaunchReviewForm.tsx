import { useCallback, useState } from 'react';
import { useAppStore } from '../../store';

// ---------------------------------------------------------------------------
// Score Ring (circular indicator 0-100)
// ---------------------------------------------------------------------------

function ScoreRing({ score }: { score: number }) {
  const r = 38;
  const circ = 2 * Math.PI * r;
  const pct = Math.min(Math.max(score, 0), 100);
  const offset = circ - (pct / 100) * circ;

  const color =
    pct >= 80 ? '#22C55E' : pct >= 50 ? '#D4AF37' : pct >= 30 ? '#F97316' : '#EF4444';

  return (
    <svg width="100" height="100" viewBox="0 0 100 100" className="flex-shrink-0">
      <circle cx="50" cy="50" r={r} fill="none" stroke="#1F1F1F" strokeWidth="5" />
      <circle
        cx="50"
        cy="50"
        r={r}
        fill="none"
        stroke={color}
        strokeWidth="5"
        strokeDasharray={circ}
        strokeDashoffset={offset}
        strokeLinecap="round"
        transform="rotate(-90 50 50)"
        className="transition-all duration-700"
      />
      <text
        x="50"
        y="46"
        textAnchor="middle"
        dominantBaseline="middle"
        fill="#FFFFFF"
        fontSize="22"
        fontFamily="Inter"
        fontWeight="600"
      >
        {pct}
      </text>
      <text
        x="50"
        y="62"
        textAnchor="middle"
        dominantBaseline="middle"
        fill="#666666"
        fontSize="9"
        fontFamily="Inter"
      >
        SCORE
      </text>
    </svg>
  );
}

// ---------------------------------------------------------------------------
// Main Component
// ---------------------------------------------------------------------------

export function LaunchReviewForm() {
  const review = useAppStore((s) => s.launchReview);
  const loading = useAppStore((s) => s.coachLoading);
  const submitReview = useAppStore((s) => s.submitLaunchReview);

  const [description, setDescription] = useState('');

  const handleSubmit = useCallback(() => {
    if (!description.trim()) return;
    submitReview(description.trim());
  }, [description, submitReview]);

  return (
    <div className="space-y-5">
      {/* Header */}
      <div>
        <h3 className="text-sm font-semibold text-white">Launch Review</h3>
        <p className="text-xs text-[#666] mt-0.5">
          Submit your project for AI-powered launch readiness analysis
        </p>
      </div>

      {/* Input Form */}
      <div className="space-y-3">
        <textarea
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder="Describe your project, target audience, pricing model, and current status..."
          rows={5}
          className="w-full bg-[#0A0A0A] border border-[#2A2A2A] rounded-lg px-4 py-3 text-sm text-[#A0A0A0] placeholder-[#666] resize-none focus:border-[#D4AF37] focus:outline-none transition-colors leading-relaxed"
        />
        <div className="flex items-center justify-between">
          <span className="text-[10px] text-[#666]">
            {description.length > 0 ? `${description.length} characters` : ''}
          </span>
          <button
            onClick={handleSubmit}
            disabled={loading || !description.trim()}
            className="px-4 py-2 text-sm font-medium bg-[#D4AF37] text-black rounded-lg hover:bg-[#C4A030] transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {loading ? 'Reviewing...' : 'Submit for Review'}
          </button>
        </div>
      </div>

      {/* Loading State */}
      {loading && !review && (
        <div className="flex items-center justify-center py-12">
          <div className="flex flex-col items-center gap-3">
            <div className="w-5 h-5 border-2 border-[#D4AF37] border-t-transparent rounded-full animate-spin" />
            <p className="text-xs text-[#A0A0A0]">
              Analyzing project launch readiness...
            </p>
          </div>
        </div>
      )}

      {/* Review Results */}
      {review && (
        <div className="bg-[#141414] border border-[#2A2A2A] rounded-xl p-5 space-y-5">
          {/* Score */}
          <div className="flex items-center gap-6">
            <ScoreRing score={review.overall_score} />
            <div className="flex-1">
              <p className="text-sm font-semibold text-white">Launch Readiness</p>
              <p className="text-xs text-[#A0A0A0] mt-1">
                {review.overall_score >= 80
                  ? 'Strong position. Address remaining gaps to maximize launch success.'
                  : review.overall_score >= 50
                    ? 'Promising foundation. Several areas need attention before launch.'
                    : 'Early stage. Focus on the recommendations below before launching.'}
              </p>
            </div>
          </div>

          {/* Strengths */}
          {review.strengths.length > 0 && (
            <div>
              <p className="text-[10px] text-[#666] uppercase tracking-wide mb-2 font-medium">
                Strengths
              </p>
              <ul className="space-y-1.5">
                {review.strengths.map((s, i) => (
                  <li key={i} className="flex items-start gap-2 text-xs">
                    <span className="text-[#22C55E] mt-0.5 flex-shrink-0">+</span>
                    <span className="text-[#A0A0A0]">{s}</span>
                  </li>
                ))}
              </ul>
            </div>
          )}

          {/* Gaps */}
          {review.gaps.length > 0 && (
            <div>
              <p className="text-[10px] text-[#666] uppercase tracking-wide mb-2 font-medium">
                Gaps
              </p>
              <ul className="space-y-1.5">
                {review.gaps.map((g, i) => (
                  <li key={i} className="flex items-start gap-2 text-xs">
                    <span className="text-[#F97316] mt-0.5 flex-shrink-0">!</span>
                    <span className="text-[#A0A0A0]">{g}</span>
                  </li>
                ))}
              </ul>
            </div>
          )}

          {/* Recommendations */}
          {review.recommendations.length > 0 && (
            <div>
              <p className="text-[10px] text-[#666] uppercase tracking-wide mb-2 font-medium">
                Recommendations
              </p>
              <ol className="space-y-2">
                {review.recommendations.map((rec, i) => (
                  <li key={i} className="flex items-start gap-3 text-xs">
                    <span className="w-5 h-5 rounded-md bg-[#1F1F1F] text-[#D4AF37] flex items-center justify-center flex-shrink-0 text-[10px] font-bold">
                      {i + 1}
                    </span>
                    <span className="text-[#A0A0A0] leading-relaxed">{rec}</span>
                  </li>
                ))}
              </ol>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
