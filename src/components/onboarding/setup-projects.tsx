// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useTranslation } from 'react-i18next';

import { DiscoveryFolderPicker } from './DiscoveryFolderPicker';
import type { DiscoveryFolders } from './use-discovery-folders';

interface SetupProjectsProps {
  discovery: DiscoveryFolders;
  scanning: boolean;
  discoveryDone: boolean;
  detectedTech: string[];
  onScan: () => void;
  onRemoveTag: (tag: string) => void;
}

export function SetupProjects({
  discovery,
  scanning,
  discoveryDone,
  detectedTech,
  onScan,
  onRemoveTag,
}: SetupProjectsProps) {
  const { t } = useTranslation();
  return (
    <div className="mt-2 p-4 bg-bg-secondary rounded-lg border border-border space-y-3">
      {scanning ? (
        <div className="flex items-center gap-2 text-sm text-text-secondary py-2" role="status">
          <div className="w-4 h-4 border-2 border-orange-500 border-t-transparent rounded-full animate-spin" />
          {t('onboarding.projects.scanning')}
        </div>
      ) : discoveryDone ? (
        detectedTech.length > 0 ? (
          <div>
            <p className="text-xs text-text-muted mb-3">{t('onboarding.projects.detected')}</p>
            <div className="flex flex-wrap gap-2">
              {detectedTech.map((tech) => (
                <span
                  key={tech}
                  className="px-3 py-1.5 bg-green-500/10 text-green-400 rounded-lg border border-green-500/20 text-sm flex items-center gap-2"
                >
                  {tech}
                  <button
                    onClick={() => onRemoveTag(tech)}
                    aria-label={t('onboarding.projects.removeTech', { tech })}
                    className="hover:text-text-primary text-green-400/70"
                  >
                    &times;
                  </button>
                </span>
              ))}
            </div>
          </div>
        ) : (
          <p className="text-sm text-text-secondary py-2">{t('onboarding.projects.noTech')}</p>
        )
      ) : (
        <>
          <DiscoveryFolderPicker discovery={discovery} />
          <button
            type="button"
            onClick={onScan}
            disabled={discovery.selected.length === 0}
            className="px-4 py-2 text-sm font-medium bg-orange-500/15 text-orange-300 border border-orange-500/30 rounded-lg hover:bg-orange-500/25 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {t('onboarding.projects.scanSelected')}
          </button>
        </>
      )}
      <p className="text-xs text-text-muted">
        {t('onboarding.projects.manageHint')}
      </p>
    </div>
  );
}
