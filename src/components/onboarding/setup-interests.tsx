import { useTranslation } from 'react-i18next';

interface SetupInterestsProps {
  roles: string[];
  role: string;
  interests: string[];
  newInterest: string;
  suggestions: string[];
  onRoleChange: (role: string) => void;
  onNewInterestChange: (value: string) => void;
  onAddInterest: () => void;
  onToggleInterest: (topic: string) => void;
}

export function SetupInterests({
  roles,
  role,
  interests,
  newInterest,
  suggestions,
  onRoleChange,
  onNewInterestChange,
  onAddInterest,
  onToggleInterest,
}: SetupInterestsProps) {
  const { t } = useTranslation();
  return (
    <div className="mt-2 p-4 bg-bg-secondary rounded-lg border border-border space-y-3">
      {/* Role selector */}
      <div>
        <label className="block text-xs text-text-muted mb-2">{t('onboarding.interests.roleLabel')}</label>
        <select
          value={role}
          onChange={(e) => onRoleChange(e.target.value)}
          className="w-full px-3 py-2 bg-bg-tertiary border border-border rounded-lg text-white text-sm focus:border-orange-500 focus:outline-none"
        >
          {roles.map((r) => (
            <option key={r} value={r}>{r}</option>
          ))}
        </select>
      </div>

      {/* Selected interests */}
      {interests.length > 0 && (
        <div className="flex flex-wrap gap-2 p-3 bg-bg-tertiary rounded-lg border border-border">
          {interests.map((interest) => (
            <span
              key={interest}
              className="px-3 py-1.5 bg-orange-500/20 text-orange-300 rounded-full text-sm flex items-center gap-2"
            >
              {interest}
              <button
                onClick={() => onToggleInterest(interest)}
                aria-label={t('onboarding.interests.remove', { topic: interest })}
                className="hover:text-white text-orange-400/70"
              >
                &times;
              </button>
            </span>
          ))}
        </div>
      )}

      {/* Custom interest input */}
      <div className="flex gap-2">
        <input
          type="text"
          aria-label={t('onboarding.interests.addLabel')}
          value={newInterest}
          onChange={(e) => onNewInterestChange(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && onAddInterest()}
          placeholder={t('onboarding.interests.placeholder')}
          className="flex-1 px-4 py-2 bg-bg-tertiary border border-border rounded-lg text-white placeholder-gray-600 focus:border-orange-500 focus:outline-none text-sm"
        />
        <button
          onClick={onAddInterest}
          disabled={!newInterest.trim()}
          className="px-4 py-2 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-colors disabled:opacity-50 disabled:cursor-not-allowed text-sm"
        >
          {t('onboarding.interests.add')}
        </button>
      </div>

      {/* Suggestions */}
      <div>
        <p className="text-xs text-text-muted mb-2">{t('onboarding.interests.quickAdd')}:</p>
        <div className="flex flex-wrap gap-2">
          {suggestions
            .filter(s => !interests.includes(s))
            .slice(0, 10)
            .map((suggestion) => (
              <button
                key={suggestion}
                onClick={() => onToggleInterest(suggestion)}
                className="px-3 py-1.5 bg-bg-tertiary text-text-secondary rounded-full text-sm hover:bg-border hover:text-white transition-all"
              >
                + {suggestion}
              </button>
            ))}
        </div>
      </div>

      <p className="text-xs text-text-muted">
        {t('onboarding.interests.hint')}
      </p>
    </div>
  );
}
