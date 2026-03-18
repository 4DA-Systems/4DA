import { useState, memo, useCallback } from 'react';

interface WaitlistSignupProps {
  tier: 'team' | 'enterprise';
  onClose?: () => void;
  inline?: boolean;
}

const TIER_INFO = {
  team: {
    name: 'Team',
    price: '$29/seat/mo',
    tagline: 'Collective developer intelligence — your team sees what no individual can.',
    features: [
      'Shared signal detection across seats',
      'Team blind spot analysis',
      'Bus factor warnings',
      'Collective tech radar',
      'Shared decision journal',
      'Encrypted metadata relay',
    ],
  },
  enterprise: {
    name: 'Enterprise',
    price: 'Custom',
    tagline: 'Organizational intelligence with the compliance your security team requires.',
    features: [
      'Everything in Team',
      'SSO / SAML license activation',
      'SCIM directory sync',
      'Audit logging (exportable)',
      'Multi-team organization view',
      'Cross-team signal correlation',
      'Webhook integrations (Slack, Teams, PagerDuty)',
      'Priority support SLA',
    ],
  },
};

/**
 * WaitlistSignup — captures interest for Team and Enterprise tiers.
 *
 * Stores signups locally (privacy-first — no external service needed at launch).
 * When Team/Enterprise tiers activate, these contacts are the first to know.
 *
 * Can be rendered inline (in settings) or as a modal overlay.
 */
const WaitlistSignup = memo(function WaitlistSignup({
  tier,
  onClose,
  inline = false,
}: WaitlistSignupProps) {
  const info = TIER_INFO[tier];
  const [email, setEmail] = useState('');
  const [teamSize, setTeamSize] = useState('');
  const [company, setCompany] = useState('');
  const [submitted, setSubmitted] = useState(false);
  const [submitting, setSubmitting] = useState(false);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      if (!email.trim()) return;

      setSubmitting(true);
      try {
        // Store locally — privacy-first, no external API
        const entry = {
          tier,
          email: email.trim(),
          team_size: teamSize.trim() || null,
          company: company.trim() || null,
          signed_up_at: new Date().toISOString(),
        };

        // Persist to localStorage for now — will migrate to DB when tiers activate
        const existing = JSON.parse(
          localStorage.getItem('4da_waitlist') || '[]',
        );
        existing.push(entry);
        localStorage.setItem('4da_waitlist', JSON.stringify(existing));

        setSubmitted(true);
      } finally {
        setSubmitting(false);
      }
    },
    [email, teamSize, company, tier],
  );

  const containerClass = inline
    ? ''
    : 'fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm';

  const cardClass = inline
    ? 'rounded-lg border border-[#2A2A2A] bg-[#141414] overflow-hidden'
    : 'w-full max-w-md rounded-lg border border-[#2A2A2A] bg-[#141414] shadow-2xl overflow-hidden';

  if (submitted) {
    return (
      <div className={containerClass}>
        <div className={cardClass}>
          <div className="p-8 text-center">
            <div className="w-12 h-12 mx-auto mb-4 rounded-full bg-[#22C55E]/10 flex items-center justify-center">
              <svg
                className="w-6 h-6 text-[#22C55E]"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M5 13l4 4L19 7"
                />
              </svg>
            </div>
            <h3 className="text-lg font-semibold text-white mb-2">
              You&apos;re on the list
            </h3>
            <p className="text-sm text-[#A0A0A0] mb-4">
              We&apos;ll notify you when {info.name} is available. You&apos;ll
              be among the first to access it.
            </p>
            {onClose && (
              <button
                onClick={onClose}
                className="text-sm text-[#8A8A8A] hover:text-white transition-colors"
              >
                Close
              </button>
            )}
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className={containerClass}>
      <div className={cardClass}>
        {/* Header */}
        <div className="px-6 py-5 border-b border-[#2A2A2A]">
          <div className="flex items-center justify-between">
            <div>
              <div className="flex items-center gap-2">
                <h3 className="text-base font-semibold text-white">
                  4DA {info.name}
                </h3>
                <span className="text-xs px-2 py-0.5 rounded-full bg-[#D4AF37]/10 text-[#D4AF37] font-medium">
                  Coming Soon
                </span>
              </div>
              <p className="text-xs text-[#8A8A8A] mt-1">{info.price}</p>
            </div>
            {onClose && (
              <button
                onClick={onClose}
                className="text-[#8A8A8A] hover:text-white transition-colors"
                aria-label="Close"
              >
                <svg
                  className="w-5 h-5"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M6 18L18 6M6 6l12 12"
                  />
                </svg>
              </button>
            )}
          </div>
          <p className="text-sm text-[#A0A0A0] mt-3">{info.tagline}</p>
        </div>

        {/* Features */}
        <div className="px-6 py-4 border-b border-[#2A2A2A]">
          <ul className="space-y-2">
            {info.features.map((feature) => (
              <li
                key={feature}
                className="flex items-start gap-2 text-sm text-[#A0A0A0]"
              >
                <span className="text-[#22C55E] mt-0.5 shrink-0">+</span>
                {feature}
              </li>
            ))}
          </ul>
        </div>

        {/* Form */}
        <form onSubmit={handleSubmit} className="px-6 py-5 space-y-3">
          <div>
            <input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="Work email"
              required
              className="w-full text-sm px-3 py-2 rounded bg-[#1F1F1F] border border-[#2A2A2A] text-white placeholder:text-[#8A8A8A]/50 focus:outline-none focus:border-[#8A8A8A] transition-colors"
            />
          </div>
          <div className="flex gap-3">
            <input
              type="text"
              value={teamSize}
              onChange={(e) => setTeamSize(e.target.value)}
              placeholder="Team size"
              className="w-1/2 text-sm px-3 py-2 rounded bg-[#1F1F1F] border border-[#2A2A2A] text-white placeholder:text-[#8A8A8A]/50 focus:outline-none focus:border-[#8A8A8A] transition-colors"
            />
            <input
              type="text"
              value={company}
              onChange={(e) => setCompany(e.target.value)}
              placeholder="Company (optional)"
              className="w-1/2 text-sm px-3 py-2 rounded bg-[#1F1F1F] border border-[#2A2A2A] text-white placeholder:text-[#8A8A8A]/50 focus:outline-none focus:border-[#8A8A8A] transition-colors"
            />
          </div>
          <button
            type="submit"
            disabled={submitting || !email.trim()}
            className="w-full text-sm font-medium px-4 py-2.5 rounded bg-white text-[#0A0A0A] hover:bg-white/90 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          >
            {submitting ? 'Joining...' : `Join ${info.name} Waitlist`}
          </button>
          <p className="text-[10px] text-[#8A8A8A] text-center">
            Stored locally. We&apos;ll only contact you when {info.name}{' '}
            launches.
          </p>
        </form>
      </div>
    </div>
  );
});

export default WaitlistSignup;
