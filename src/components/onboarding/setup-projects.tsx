import { useTranslation } from 'react-i18next';

interface SetupProjectsProps {
  discoveryDone: boolean;
  detectedTech: string[];
  onRemoveTag: (tag: string) => void;
}

export function SetupProjects({
  discoveryDone,
  detectedTech,
  onRemoveTag,
}: SetupProjectsProps) {
  const { t } = useTranslation();
  return (
    <div className="mt-2 p-4 bg-bg-secondary rounded-lg border border-border">
      {!discoveryDone ? (
        <div className="flex items-center gap-2 text-sm text-gray-400 py-2">
          <div className="w-4 h-4 border-2 border-orange-500 border-t-transparent rounded-full animate-spin" />
          {t('onboarding.projects.scanning')}
        </div>
      ) : detectedTech.length > 0 ? (
        <div>
          <p className="text-xs text-gray-500 mb-3">{t('onboarding.projects.detected')}</p>
          <div className="flex flex-wrap gap-2">
            {detectedTech.map((tech) => (
              <span
                key={tech}
                className="px-3 py-1.5 bg-green-500/10 text-green-400 rounded-lg border border-green-500/20 text-sm flex items-center gap-2"
              >
                {tech}
                <button
                  onClick={() => onRemoveTag(tech)}
                  aria-label={`Remove ${tech}`}
                  className="hover:text-white text-green-400/70"
                >
                  &times;
                </button>
              </span>
            ))}
          </div>
        </div>
      ) : (
        <p className="text-sm text-gray-400 py-2">
          {t('onboarding.projects.noTech')}
        </p>
      )}
      <p className="text-xs text-gray-500 mt-3">
        {t('onboarding.projects.manageHint')}
      </p>
    </div>
  );
}
