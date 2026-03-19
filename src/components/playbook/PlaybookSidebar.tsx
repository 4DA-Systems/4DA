import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import { MODULE_IDS, CheckIcon, ProgressRing } from './PlaybookIcons';
import type { PlaybookProgress, PlaybookModule } from '../../types/playbook';

interface PlaybookSidebarProps {
  playbookModules: PlaybookModule[];
  playbookProgress: PlaybookProgress | null;
  activeModuleId: string | null;
  streetsTier: string;
  showTemplates: boolean;
  onModuleClick: (moduleId: string) => void;
  onShowTemplates: () => void;
}

export const PlaybookSidebar = memo(function PlaybookSidebar({
  playbookModules,
  playbookProgress,
  activeModuleId,
  streetsTier: _streetsTier,
  showTemplates,
  onModuleClick,
  onShowTemplates,
}: PlaybookSidebarProps) {
  const { t } = useTranslation();
  const overallPct = playbookProgress?.overall_percentage ?? 0;

  return (
    <aside aria-label={t('streets:streets.title')} className="w-64 flex-shrink-0 bg-bg-secondary border border-border rounded-xl p-4 space-y-2 self-start sticky top-6">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-sm font-semibold text-white tracking-wide uppercase">{t('streets:streets.title')}</h2>
        <ProgressRing percentage={overallPct} />
      </div>

      {MODULE_IDS.map((modId) => {
        const progress = playbookProgress?.modules.find((m) => m.module_id === modId);
        const pct = progress?.percentage ?? 0;
        const isActive = activeModuleId === modId;
        const moduleData = playbookModules.find((m) => m.id === modId);
        const lessonCount = moduleData?.lesson_count ?? 0;

        return (
          <button
            key={modId}
            onClick={() => onModuleClick(modId)}
            className={`w-full text-left px-3 py-2.5 rounded-lg transition-all flex items-center gap-3 group ${
              isActive
                ? 'bg-[#D4AF37]/15 border border-[#D4AF37]/30'
                : 'hover:bg-bg-tertiary border border-transparent'
            }`}
          >
            <span
              className={`w-7 h-7 rounded-md flex items-center justify-center text-xs font-bold flex-shrink-0 ${
                pct >= 100
                  ? 'bg-[#22C55E]/20 text-[#22C55E]'
                  : isActive
                    ? 'bg-[#D4AF37]/20 text-[#D4AF37]'
                    : 'bg-bg-tertiary text-text-secondary'
              }`}
            >
              {modId}
            </span>
            <div className="flex-1 min-w-0">
              <p className={`text-sm truncate ${isActive ? 'text-white font-medium' : 'text-text-secondary'}`}>
                {t(`streets:streets.module.${modId}`)}
              </p>
              <p className="text-[10px] text-[#8A8A8A]">
                {lessonCount} {lessonCount !== 1 ? t('streets:streets.lessons').toLowerCase() : t('streets:streets.lesson').toLowerCase()}
                {pct > 0 && pct < 100 && ` - ${Math.round(pct)}%`}
              </p>
            </div>
            {pct >= 100 && <CheckIcon />}
          </button>
        );
      })}

      {/* Templates */}
      <button
        onClick={onShowTemplates}
        className={`w-full text-left px-3 py-2.5 rounded-lg transition-all flex items-center gap-3 group ${
          showTemplates
            ? 'bg-[#D4AF37]/15 border border-[#D4AF37]/30'
            : 'hover:bg-bg-tertiary border border-transparent'
        }`}
      >
        <span
          className={`w-7 h-7 rounded-md flex items-center justify-center text-xs flex-shrink-0 ${
            showTemplates
              ? 'bg-[#D4AF37]/20 text-[#D4AF37]'
              : 'bg-bg-tertiary text-text-secondary'
          }`}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
            <polyline points="14 2 14 8 20 8" />
          </svg>
        </span>
        <div className="flex-1 min-w-0">
          <p className={`text-sm truncate ${showTemplates ? 'text-white font-medium' : 'text-text-secondary'}`}>
            {t('playbook.templates')}
          </p>
          <p className="text-[10px] text-[#8A8A8A]">{t('playbook.templatesSubtitle')}</p>
        </div>
      </button>

      {/* Free tier notice */}
      <div className="mt-4 pt-4 border-t border-border">
        <p className="text-[10px] text-[#8A8A8A] text-center">
          {t('streets:streets.freeForever')}
        </p>
      </div>
    </aside>
  );
});
