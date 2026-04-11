import { useTranslation } from 'react-i18next';

export type ExperienceLevel = 'learning' | 'building' | 'leading' | 'architecting';

interface SetupExperienceProps {
  selected: ExperienceLevel | null;
  onSelect: (level: ExperienceLevel) => void;
}

const levels: { id: ExperienceLevel; icon: string }[] = [
  { id: 'learning', icon: '\u{1F4DA}' },
  { id: 'building', icon: '\u{1F6E0}' },
  { id: 'leading', icon: '\u{1F9ED}' },
  { id: 'architecting', icon: '\u{1F3D7}' },
];

export function SetupExperience({ selected, onSelect }: SetupExperienceProps) {
  const { t } = useTranslation();

  return (
    <div className="mt-2 p-4 bg-bg-secondary rounded-lg border border-border space-y-2">
      <p className="text-xs text-text-muted mb-2">
        {t('onboarding.experience.description')}
      </p>
      <div className="grid grid-cols-1 gap-2">
        {levels.map((level) => {
          const isSelected = selected === level.id;
          return (
            <button
              key={level.id}
              onClick={() => onSelect(level.id)}
              aria-pressed={isSelected}
              className={`flex items-start gap-3 p-3 rounded-lg border text-start transition-all ${
                isSelected
                  ? 'border-orange-500 bg-orange-500/10'
                  : 'border-border bg-bg-tertiary hover:border-[#3A3A3A]'
              }`}
            >
              <span className="text-lg flex-shrink-0 mt-0.5" aria-hidden="true">
                {level.icon}
              </span>
              <div>
                <div className={`text-sm font-medium ${isSelected ? 'text-orange-400' : 'text-white'}`}>
                  {t(`onboarding.experience.${level.id}.label`)}
                </div>
                <div className="text-xs text-text-muted mt-0.5">
                  {t(`onboarding.experience.${level.id}.description`)}
                </div>
              </div>
            </button>
          );
        })}
      </div>
    </div>
  );
}
