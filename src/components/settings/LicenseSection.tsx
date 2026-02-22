import { useState, useEffect } from 'react';
import { useAppStore } from '../../store';

export function LicenseSection({ onStatus }: { onStatus: (s: string) => void }) {
  const tier = useAppStore(s => s.tier);
  const trialStatus = useAppStore(s => s.trialStatus);
  const licenseLoading = useAppStore(s => s.licenseLoading);
  const activateLicense = useAppStore(s => s.activateLicense);
  const startTrial = useAppStore(s => s.startTrial);
  const loadLicense = useAppStore(s => s.loadLicense);
  const loadTrialStatus = useAppStore(s => s.loadTrialStatus);
  const expired = useAppStore(s => s.expired);
  const daysRemaining = useAppStore(s => s.daysRemaining);
  const expiresAt = useAppStore(s => s.expiresAt);

  const [key, setKey] = useState('');
  const [starting, setStarting] = useState(false);

  useEffect(() => {
    loadLicense();
    loadTrialStatus();
  }, [loadLicense, loadTrialStatus]);

  const isPro = !expired && (tier === 'pro' || tier === 'team');
  const trialActive = trialStatus?.active === true;
  const trialExpired = trialStatus != null && !trialStatus.active && trialStatus.started_at != null;
  const canStartTrial = !isPro && !trialStatus?.started_at;
  const expiryWarning = isPro && daysRemaining > 0 && daysRemaining <= 14;

  const handleActivate = async () => {
    if (!key.trim()) return;
    const ok = await activateLicense(key.trim());
    if (ok) {
      onStatus('License activated successfully');
      setKey('');
    } else {
      onStatus('Error: Invalid license key');
    }
    setTimeout(() => onStatus(''), 3000);
  };

  const handleStartTrial = async () => {
    setStarting(true);
    const ok = await startTrial();
    setStarting(false);
    if (ok) {
      onStatus('30-day free trial started');
    } else {
      onStatus('Error: Could not start trial');
    }
    setTimeout(() => onStatus(''), 3000);
  };

  const tierConfig: Record<string, { label: string; color: string }> = {
    free: { label: 'Free', color: 'text-gray-400' },
    pro: { label: 'Pro', color: 'text-[#D4AF37]' },
    team: { label: 'Team', color: 'text-[#22C55E]' },
  };
  const { label: tierLabel, color: tierColor } = tierConfig[tier] ?? tierConfig.free;

  return (
    <div className="bg-bg-tertiary rounded-lg p-4 border border-border">
      <h3 className="text-sm font-medium text-white mb-3">License</h3>

      {/* Current tier */}
      <div className="flex items-center gap-2 mb-3">
        <span className="text-xs text-gray-500">Current tier:</span>
        <span className={`text-xs font-semibold ${tierColor}`}>{tierLabel}</span>
        {trialActive && (
          <span className="text-[10px] px-1.5 py-0.5 bg-[#D4AF37]/15 text-[#D4AF37] rounded">
            Trial: {trialStatus.days_remaining}d left
          </span>
        )}
        {trialExpired && (
          <span className="text-[10px] px-1.5 py-0.5 bg-[#EF4444]/15 text-[#EF4444] rounded">
            Trial expired
          </span>
        )}
      </div>

      {/* Expired license banner */}
      {expired && (
        <div className="mb-3 p-2.5 rounded-lg bg-[#EF4444]/10 border border-[#EF4444]/30">
          <p className="text-xs font-medium text-[#EF4444] mb-1">License expired</p>
          <p className="text-[10px] text-[#EF4444]/70">
            Your license expired{expiresAt ? ` on ${new Date(expiresAt).toLocaleDateString()}` : ''}. Renew to restore Pro features.
          </p>
          <a
            href="https://4da.ai/streets"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-block mt-2 px-3 py-1.5 text-[10px] font-semibold text-black bg-[#D4AF37] rounded hover:bg-[#C4A030] transition-colors"
          >
            Renew License
          </a>
        </div>
      )}

      {/* Expiry warning (< 14 days) */}
      {expiryWarning && (
        <div className="mb-3 p-2.5 rounded-lg bg-[#D4AF37]/10 border border-[#D4AF37]/30">
          <p className="text-[10px] text-[#D4AF37]">
            License expires in {daysRemaining} day{daysRemaining !== 1 ? 's' : ''}.{' '}
            <a href="https://4da.ai/streets" target="_blank" rel="noopener noreferrer" className="underline font-medium">
              Renew now
            </a>
          </p>
        </div>
      )}

      {/* Pro badge — show what's unlocked */}
      {isPro && (
        <p className="text-xs text-gray-500 mb-3">
          All Pro features unlocked.{expiresAt && !expiryWarning ? ` Renews ${new Date(expiresAt).toLocaleDateString()}.` : ' License verified.'}
        </p>
      )}

      {/* License key input — show when not Pro or expired */}
      {(!isPro || expired) && (
        <div className="space-y-3">
          <div className="flex gap-2">
            <input
              type="text"
              value={key}
              onChange={e => setKey(e.target.value)}
              placeholder="4DA-xxxxx.xxxxx"
              onKeyDown={e => e.key === 'Enter' && handleActivate()}
              className="flex-1 px-3 py-2 bg-bg-primary border border-border rounded-lg text-sm text-white placeholder-gray-600 focus:outline-none focus:border-[#D4AF37]/50 font-mono text-xs"
            />
            <button
              onClick={handleActivate}
              disabled={licenseLoading || !key.trim()}
              className="px-4 py-2 text-sm font-medium text-black bg-[#D4AF37] rounded-lg hover:bg-[#C4A030] transition-colors disabled:opacity-50"
            >
              {licenseLoading ? '...' : 'Activate'}
            </button>
          </div>

          {/* Trial button */}
          {canStartTrial && (
            <button
              onClick={handleStartTrial}
              disabled={starting}
              className="w-full px-4 py-2 text-xs font-medium text-gray-300 border border-gray-600 rounded-lg hover:border-gray-400 hover:text-white transition-colors disabled:opacity-50"
            >
              {starting ? 'Starting...' : 'Start 30-Day Free Trial'}
            </button>
          )}

          {/* Upgrade link */}
          <a
            href="https://4da.ai/streets"
            target="_blank"
            rel="noopener noreferrer"
            className="block text-center text-xs text-[#D4AF37] hover:underline"
          >
            Get a license key at 4da.ai/streets
          </a>
        </div>
      )}
    </div>
  );
}
