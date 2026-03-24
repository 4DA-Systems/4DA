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
  const confidencePct = Math.round(summary.confidence * 100);

  return (
    <div className="space-y-6 animate-in fade-in duration-300">
      {/* Header */}
      <div className="text-center">
        <h2 className="text-xl font-semibold text-white mb-2">
          Your feed is calibrated
        </h2>
        <p className="text-text-secondary text-sm">
          Based on {summary.itemsShown} responses, {confidencePct}% confidence
        </p>
      </div>

      {/* Dominant persona */}
      <div className="bg-bg-secondary border border-border rounded-lg p-5">
        <div className="text-xs text-text-muted uppercase tracking-wider mb-2">
          Your developer profile
        </div>
        <h3 className="text-white font-medium text-lg mb-1">
          {summary.dominantPersonaName}
        </h3>
        <p className="text-text-secondary text-sm">
          {summary.dominantPersonaDescription}
        </p>
      </div>

      {/* Persona blend bar chart */}
      {summary.personaWeights.length > 1 && (
        <div className="bg-bg-secondary border border-border rounded-lg p-5">
          <div className="text-xs text-text-muted uppercase tracking-wider mb-3">
            Persona blend
          </div>
          <div className="space-y-2">
            {summary.personaWeights
              .sort((a, b) => b.weight - a.weight)
              .map((pw) => (
                <div key={pw.name} className="flex items-center gap-3">
                  <span className="text-xs text-text-secondary w-40 truncate">
                    {pw.name}
                  </span>
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

      {/* Detected interests */}
      {summary.topInterests.length > 0 && (
        <div className="bg-bg-secondary border border-border rounded-lg p-5">
          <div className="text-xs text-text-muted uppercase tracking-wider mb-3">
            Detected interests
          </div>
          <div className="flex flex-wrap gap-2">
            {summary.topInterests.map((interest) => (
              <span
                key={interest}
                className="text-xs bg-bg-tertiary text-text-secondary px-2.5 py-1 rounded-md"
              >
                {interest}
              </span>
            ))}
          </div>
        </div>
      )}

      {/* Continue button */}
      <button
        onClick={onContinue}
        className="w-full bg-orange-500 hover:bg-orange-600 text-white font-medium py-3 rounded-lg transition-colors"
      >
        Continue
      </button>
    </div>
  );
}
