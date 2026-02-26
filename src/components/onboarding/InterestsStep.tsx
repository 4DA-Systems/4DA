import { useTranslation } from 'react-i18next';

const suggestedInterests = [
  'Machine Learning', 'Rust', 'TypeScript', 'Web Development',
  'DevOps', 'Security', 'Startups', 'Open Source', 'AI/LLM',
  'Mobile Development', 'Cloud Infrastructure', 'Data Engineering',
];

interface InterestsStepProps {
  isAnimating: boolean;
  role: string;
  setRole: (role: string) => void;
  interests: string[];
  setInterests: React.Dispatch<React.SetStateAction<string[]>>;
  newInterest: string;
  setNewInterest: (val: string) => void;
  onSave: () => void;
  onBack: () => void;
}

export function InterestsStep({
  isAnimating,
  role,
  setRole,
  interests,
  setInterests,
  newInterest,
  setNewInterest,
  onSave,
  onBack,
}: InterestsStepProps) {
  const { t } = useTranslation();

  const addInterest = () => {
    if (newInterest.trim() && !interests.includes(newInterest.trim())) {
      setInterests([...interests, newInterest.trim()]);
      setNewInterest('');
    }
  };

  const removeInterest = (interest: string) => {
    setInterests(interests.filter(i => i !== interest));
  };

  return (
    <div className={`transition-all duration-300 ${isAnimating ? 'opacity-0 translate-y-4' : 'opacity-100 translate-y-0'}`}>
      <h2 className="text-3xl font-semibold text-white mb-2 text-center">{t('onboarding.interests.title')}</h2>
      <p className="text-gray-400 mb-6 text-center">
        {t('onboarding.interests.subtitle')}
      </p>

      <div className="space-y-5 bg-bg-secondary p-6 rounded-lg mb-6">
        {/* Role - simplified */}
        <div>
          <label className="block text-sm text-gray-400 mb-2">
            {t('onboarding.interests.roleLabel')} <span className="text-gray-600">({t('onboarding.interests.optional')})</span>
          </label>
          <input
            type="text"
            value={role}
            onChange={(e) => setRole(e.target.value)}
            placeholder={t('onboarding.interests.rolePlaceholder')}
            className="w-full px-4 py-3 bg-bg-tertiary border border-border rounded-lg text-white placeholder-gray-600 focus:border-orange-500 focus:outline-none"
          />
        </div>

        {/* Interests - improved */}
        <div>
          <label className="block text-sm text-gray-400 mb-2">
            {t('onboarding.interests.topicsLabel')}
          </label>

          {/* Selected interests first */}
          {interests.length > 0 && (
            <div className="flex flex-wrap gap-2 mb-3 p-3 bg-bg-tertiary rounded-lg border border-border">
              {interests.map((interest) => (
                <span
                  key={interest}
                  className="px-3 py-1.5 bg-orange-500/20 text-orange-300 rounded-full text-sm flex items-center gap-2 animate-in fade-in duration-200"
                >
                  {interest}
                  <button
                    onClick={() => removeInterest(interest)}
                    aria-label={`Remove ${interest}`}
                    className="hover:text-white text-orange-400/70"
                  >
                    &times;
                  </button>
                </span>
              ))}
            </div>
          )}

          {/* Add custom interest */}
          <div className="flex gap-2 mb-4">
            <input
              type="text"
              value={newInterest}
              onChange={(e) => setNewInterest(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && addInterest()}
              placeholder={t('onboarding.interests.placeholder')}
              className="flex-1 px-4 py-2 bg-bg-tertiary border border-border rounded-lg text-white placeholder-gray-600 focus:border-orange-500 focus:outline-none"
            />
            <button
              onClick={addInterest}
              disabled={!newInterest.trim()}
              className="px-4 py-2 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {t('onboarding.interests.add')}
            </button>
          </div>

          {/* Suggestions - categorized */}
          <div className="space-y-3">
            <p className="text-xs text-gray-500">{t('onboarding.interests.quickAdd')}:</p>
            <div className="flex flex-wrap gap-2">
              {suggestedInterests
                .filter((s) => !interests.includes(s))
                .slice(0, 10)
                .map((suggestion) => (
                  <button
                    key={suggestion}
                    onClick={() => setInterests([...interests, suggestion])}
                    className="px-3 py-1.5 bg-bg-tertiary text-gray-400 rounded-full text-sm hover:bg-border hover:text-white transition-all hover:scale-105"
                  >
                    + {suggestion}
                  </button>
                ))}
            </div>
          </div>
        </div>

        {/* Hint */}
        <p className="text-xs text-gray-500 text-center">
          {t('onboarding.interests.hint')}
        </p>
      </div>

      <div className="flex justify-between items-center">
        <button
          onClick={onBack}
          className="px-6 py-2 text-gray-400 hover:text-white transition-colors"
        >
          &larr; {t('onboarding.nav.back')}
        </button>
        <div className="flex items-center gap-3">
          <button
            onClick={() => {
              setInterests([]);
              setRole('');
              onSave();
            }}
            className="px-4 py-2 text-gray-500 hover:text-gray-300 text-sm transition-colors"
          >
            {t('onboarding.nav.skipForNow')}
          </button>
          <button
            onClick={onSave}
            className="px-8 py-3 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-colors font-medium"
          >
            {interests.length > 0 || role ? t('onboarding.interests.saveAndFinish') : t('onboarding.interests.finishSetup')}
          </button>
        </div>
      </div>
    </div>
  );
}
