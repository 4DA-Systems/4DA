// Module IDs (static, mirrors backend MODULE_DEFS)
export const MODULE_IDS = ['S', 'T', 'R', 'E1', 'E2', 'T2', 'S2'] as const;

export function CheckIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-success">
      <polyline points="20 6 9 17 4 12" />
    </svg>
  );
}

export function ProgressRing({ percentage }: { percentage: number }) {
  const r = 14;
  const circ = 2 * Math.PI * r;
  const offset = circ - (percentage / 100) * circ;
  return (
    <svg width="36" height="36" viewBox="0 0 36 36" className="flex-shrink-0" role="img" aria-label={`${Math.round(percentage)}% complete`}>
      <circle cx="18" cy="18" r={r} fill="none" stroke="#2A2A2A" strokeWidth="3" />
      <circle
        cx="18" cy="18" r={r} fill="none"
        stroke={percentage >= 100 ? '#22C55E' : '#D4AF37'}
        strokeWidth="3"
        strokeDasharray={circ}
        strokeDashoffset={offset}
        strokeLinecap="round"
        transform="rotate(-90 18 18)"
        className="transition-all duration-500"
      />
      <text x="18" y="19" textAnchor="middle" dominantBaseline="middle" fill="#A0A0A0" fontSize="8" fontFamily="Inter">
        {Math.round(percentage)}%
      </text>
    </svg>
  );
}
