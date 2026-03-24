import { useState, memo, useCallback } from 'react';
import { cmd } from '../lib/commands';

interface WaitlistSignupProps {
  tier: 'team' | 'enterprise';
  onClose?: () => void;
  inline?: boolean;
}

const TIER_CONFIG = {
  team: {
    name: 'Team',
    price: '$29/seat/mo',
    annual: '$249/seat/yr',
    tagline: 'Your team already tracks the same ecosystems. 4DA Team turns that overlap into a multiplier.',
    value: 'Shared signal detection, collective blind spot elimination, coordinated response to critical events.',
    icon: (
      <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
        <circle cx="7" cy="7" r="3" stroke="currentColor" strokeWidth="1.5"/>
        <circle cx="13" cy="7" r="3" stroke="currentColor" strokeWidth="1.5"/>
        <circle cx="10" cy="13" r="3" stroke="currentColor" strokeWidth="1.5"/>
      </svg>
    ),
    features: [
      { text: 'Shared signal detection across all seats', detail: 'When 2+ developers detect the same CVE, confidence multiplies' },
      { text: 'Team blind spot analysis', detail: 'Topics no one on the team monitors — but should' },
      { text: 'Bus factor warnings', detail: 'Tech known by only one person flagged as risk' },
      { text: 'Collective tech radar', detail: 'Aggregated adoption signals from every seat' },
      { text: 'Shared decision journal', detail: 'Architecture decisions tracked with evidence' },
      { text: 'Encrypted metadata relay', detail: 'E2E encrypted — relay cannot read your data' },
    ],
  },
  enterprise: {
    name: 'Enterprise',
    price: 'From $22/seat/mo',
    annual: '25+ seats, annual',
    tagline: 'Same app your developers already love. Now with the trust your security team requires.',
    value: 'No server to audit. No data to breach. No cloud to secure. Privacy by architecture.',
    icon: (
      <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
        <rect x="3" y="3" width="14" height="14" rx="2" stroke="currentColor" strokeWidth="1.5"/>
        <path d="M3 8h14M8 8v9" stroke="currentColor" strokeWidth="1.5"/>
      </svg>
    ),
    features: [
      { text: 'Everything in Team', detail: null },
      { text: 'SSO license activation', detail: 'Okta, Azure AD, Google Workspace via SAML/OIDC' },
      { text: 'SCIM directory sync', detail: 'Auto-provision seats when employees join, revoke when they leave' },
      { text: 'Audit logging', detail: 'Structured actions, exportable CSV/JSON, configurable retention' },
      { text: 'Multi-team organizations', detail: 'Cross-team signal correlation — org-wide intelligence' },
      { text: 'Webhook integrations', detail: 'Slack, Teams, PagerDuty with HMAC-signed payloads' },
      { text: 'Procurement documentation', detail: 'Security whitepaper, DPA, vendor risk assessment — pre-filled' },
      { text: 'Priority support SLA', detail: '1-business-day response, dedicated channel' },
    ],
  },
};

const WaitlistSignup = memo(function WaitlistSignup({
  tier,
  onClose,
  inline = false,
}: WaitlistSignupProps) {
  const config = TIER_CONFIG[tier];
  const [email, setEmail] = useState('');
  const [name, setName] = useState('');
  const [teamSize, setTeamSize] = useState('');
  const [company, setCompany] = useState('');
  const [role, setRole] = useState('');
  const [submitted, setSubmitted] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [expandedFeature, setExpandedFeature] = useState<number | null>(null);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      if (!email.trim()) return;

      setSubmitting(true);
      try {
        // Store in SQLite via Tauri command (privacy-first, persistent)
        await cmd('save_waitlist_signup', {
          tier,
          email: email.trim(),
          name: name.trim() || null,
          teamSize: teamSize.trim() || null,
          company: company.trim() || null,
          role: role.trim() || null,
        });
        setSubmitted(true);
      } catch {
        // Fallback to localStorage if Tauri command not available
        const entry = {
          tier,
          email: email.trim(),
          signed_up_at: new Date().toISOString(),
        };
        const existing = JSON.parse(localStorage.getItem('4da_waitlist') || '[]');
        if (!existing.some((e: { email: string; tier: string }) => e.email === entry.email && e.tier === entry.tier)) {
          existing.push(entry);
          localStorage.setItem('4da_waitlist', JSON.stringify(existing));
        }
        setSubmitted(true);
      } finally {
        setSubmitting(false);
      }
    },
    [email, name, teamSize, company, role, tier],
  );

  // ---- Success State ----
  if (submitted) {
    return (
      <WaitlistContainer inline={inline}>
        <div className="max-w-md w-full rounded-xl border border-border bg-bg-secondary shadow-2xl overflow-hidden">
          <div className="px-8 py-10 text-center">
            <div className="w-14 h-14 mx-auto mb-5 rounded-full bg-success/10 flex items-center justify-center">
              <svg className="w-7 h-7 text-success" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
              </svg>
            </div>
            <h3 className="text-xl font-semibold text-white mb-2">
              You&apos;re on the list
            </h3>
            <p className="text-sm text-text-secondary leading-relaxed mb-6">
              We&apos;ll reach out when 4DA {config.name} is ready.
              <br />
              You&apos;ll be among the first to experience it.
            </p>
            <div className="inline-flex items-center gap-2 px-4 py-2 rounded-lg bg-bg-tertiary border border-border">
              <div className="w-2 h-2 rounded-full bg-success animate-pulse" />
              <span className="text-xs text-text-secondary">
                Position secured for {config.name} early access
              </span>
            </div>
            {onClose && (
              <button
                onClick={onClose}
                className="block mx-auto mt-6 text-sm text-text-muted hover:text-white transition-colors"
              >
                Continue using 4DA
              </button>
            )}
          </div>
        </div>
      </WaitlistContainer>
    );
  }

  // ---- Main Form ----
  return (
    <WaitlistContainer inline={inline}>
      <div className="max-w-lg w-full rounded-xl border border-border bg-bg-secondary shadow-2xl overflow-hidden">
        {/* Header */}
        <div className="relative px-6 pt-6 pb-5">
          {onClose && (
            <button
              onClick={onClose}
              className="absolute top-4 end-4 p-1 text-text-muted hover:text-white transition-colors rounded-lg hover:bg-white/5"
              aria-label="Close"
            >
              <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          )}

          <div className="flex items-start gap-3">
            <div className="w-10 h-10 rounded-lg bg-accent-gold/10 flex items-center justify-center text-accent-gold shrink-0 mt-0.5">
              {config.icon}
            </div>
            <div className="pe-6">
              <div className="flex items-center gap-2.5">
                <h2 className="text-lg font-semibold text-white">4DA {config.name}</h2>
                <span className="text-[10px] px-2 py-0.5 rounded-full bg-accent-gold/10 text-accent-gold font-semibold uppercase tracking-wider">
                  Coming Soon
                </span>
              </div>
              <p className="text-xs text-text-muted mt-0.5">
                {config.price} &middot; {config.annual}
              </p>
            </div>
          </div>

          <p className="text-[13px] text-text-secondary mt-4 leading-relaxed">
            {config.tagline}
          </p>
          <p className="text-xs text-text-muted mt-2 italic">
            {config.value}
          </p>
        </div>

        {/* Features */}
        <div className="px-6 py-4 border-t border-bg-tertiary bg-[#0F0F0F]">
          <p className="text-[10px] text-text-muted uppercase tracking-wider font-medium mb-3">
            What&apos;s included
          </p>
          <ul className="space-y-1">
            {config.features.map((feature, i) => (
              <li key={i}>
                <button
                  type="button"
                  onClick={() => setExpandedFeature(expandedFeature === i ? null : i)}
                  className="w-full flex items-start gap-2.5 py-1.5 text-start group"
                >
                  <span className="text-success text-xs mt-0.5 shrink-0 font-mono">+</span>
                  <span className="text-[13px] text-[#C0C0C0] group-hover:text-white transition-colors">
                    {feature.text}
                  </span>
                </button>
                {expandedFeature === i && feature.detail && (
                  <p className="ms-5 pb-2 text-xs text-text-muted leading-relaxed">
                    {feature.detail}
                  </p>
                )}
              </li>
            ))}
          </ul>
        </div>

        {/* Form */}
        <form onSubmit={handleSubmit} className="px-6 py-5 border-t border-bg-tertiary">
          <p className="text-[10px] text-text-muted uppercase tracking-wider font-medium mb-3">
            Get early access
          </p>

          <div className="space-y-2.5">
            <input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="Work email *"
              required
              autoComplete="email"
              className="w-full text-sm px-3.5 py-2.5 rounded-lg bg-bg-tertiary border border-border text-white placeholder:text-text-muted focus:outline-none focus:border-accent-gold/40 focus:ring-1 focus:ring-accent-gold/20 transition-all"
            />
            <div className="flex gap-2.5">
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Your name"
                autoComplete="name"
                className="w-1/2 text-sm px-3.5 py-2.5 rounded-lg bg-bg-tertiary border border-border text-white placeholder:text-text-muted focus:outline-none focus:border-accent-gold/40 focus:ring-1 focus:ring-accent-gold/20 transition-all"
              />
              <input
                type="text"
                value={teamSize}
                onChange={(e) => setTeamSize(e.target.value)}
                placeholder="Team size"
                className="w-1/2 text-sm px-3.5 py-2.5 rounded-lg bg-bg-tertiary border border-border text-white placeholder:text-text-muted focus:outline-none focus:border-accent-gold/40 focus:ring-1 focus:ring-accent-gold/20 transition-all"
              />
            </div>
            <div className="flex gap-2.5">
              <input
                type="text"
                value={company}
                onChange={(e) => setCompany(e.target.value)}
                placeholder="Company"
                autoComplete="organization"
                className="w-1/2 text-sm px-3.5 py-2.5 rounded-lg bg-bg-tertiary border border-border text-white placeholder:text-text-muted focus:outline-none focus:border-accent-gold/40 focus:ring-1 focus:ring-accent-gold/20 transition-all"
              />
              <input
                type="text"
                value={role}
                onChange={(e) => setRole(e.target.value)}
                placeholder="Role (e.g. Eng Manager)"
                className="w-1/2 text-sm px-3.5 py-2.5 rounded-lg bg-bg-tertiary border border-border text-white placeholder:text-text-muted focus:outline-none focus:border-accent-gold/40 focus:ring-1 focus:ring-accent-gold/20 transition-all"
              />
            </div>
          </div>

          <button
            type="submit"
            disabled={submitting || !email.trim()}
            className="w-full mt-4 text-sm font-semibold px-4 py-3 rounded-lg bg-white text-bg-primary hover:bg-[#F0F0F0] active:bg-[#E0E0E0] transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
          >
            {submitting ? 'Joining...' : `Join the ${config.name} Waitlist`}
          </button>

          <p className="mt-3 text-[10px] text-text-muted text-center leading-relaxed">
            Stored on your device only. No data sent externally.
            <br />
            We&apos;ll reach out when {config.name} launches.
          </p>
        </form>
      </div>
    </WaitlistContainer>
  );
});

/** Container handles modal vs inline rendering */
function WaitlistContainer({
  inline,
  children,
}: {
  inline: boolean;
  children: React.ReactNode;
}) {
  if (inline) {
    return <div className="flex justify-center">{children}</div>;
  }
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm animate-in fade-in duration-200">
      {children}
    </div>
  );
}

export default WaitlistSignup;
