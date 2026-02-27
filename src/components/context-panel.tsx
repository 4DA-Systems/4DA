import { useTranslation } from 'react-i18next';

interface ContextFile {
  path: string;
  lines: number;
}

interface DiscoveredTech {
  name: string;
  category: string;
  confidence: number;
}

interface DiscoveredContext {
  tech: DiscoveredTech[];
  topics: string[];
}

interface ContextPanelProps {
  contextFiles: ContextFile[];
  discoveredContext: DiscoveredContext;
  loading: boolean;
  onReload: () => void;
  onIndex: () => void;
  onClear: () => void;
}

export function ContextPanel({
  contextFiles,
  discoveredContext,
  loading,
  onReload,
  onIndex,
  onClear,
}: ContextPanelProps) {
  const { t } = useTranslation();
  return (
    <section aria-label={t('context.title')} className="bg-bg-secondary rounded-lg border border-border overflow-hidden">
      <div className="px-5 py-4 border-b border-border flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 bg-bg-tertiary rounded-lg flex items-center justify-center">
            <span className="text-gray-500">F</span>
          </div>
          <div>
            <h2 className="font-medium text-white">{t('context.title')}</h2>
            <p className="text-xs text-gray-500">{t('context.filesIndexed', { count: contextFiles.length })}</p>
          </div>
        </div>
        <div className="flex gap-2">
          <button
            onClick={onReload}
            aria-label="Reload context files"
            className="w-8 h-8 flex items-center justify-center text-sm bg-bg-tertiary text-gray-400 rounded-lg hover:bg-border hover:text-white transition-all"
            title={t('context.reloadFiles')}
          >
            R
          </button>
          {contextFiles.length > 0 && (
            <>
              <button
                onClick={onIndex}
                disabled={loading}
                className="px-3 py-1.5 text-xs bg-green-500/10 text-green-400 border border-green-500/30 rounded-lg hover:bg-green-500/20 transition-all disabled:opacity-50"
                title={t('context.indexFiles')}
              >
                {t('context.index')}
              </button>
              <button
                onClick={onClear}
                className="px-3 py-1.5 text-xs bg-red-500/10 text-red-400 border border-red-500/30 rounded-lg hover:bg-red-500/20 transition-all"
                title={t('context.clear')}
              >
                {t('context.clear')}
              </button>
            </>
          )}
        </div>
      </div>
      <div className="p-4 max-h-[calc(100vh-320px)] overflow-y-auto">
        {contextFiles.length === 0 ? (
          <div className="text-center py-8 px-4">
            <div className="w-12 h-12 mx-auto mb-3 bg-bg-tertiary rounded-full flex items-center justify-center">
              <svg className="w-5 h-5 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
              </svg>
            </div>
            <p className="text-gray-400 text-sm mb-1">{t('context.autoDiscovered')}</p>
            <p className="text-xs text-gray-600">{t('context.autoDiscoveredHint')}</p>
          </div>
        ) : (
          <ul className="space-y-2">
            {contextFiles.map((file) => (
              <li
                key={file.path}
                className="px-3 py-2 bg-bg-tertiary rounded-lg border border-border hover:border-orange-500/30 transition-all"
              >
                <div className="font-mono text-white text-sm truncate">
                  {file.path.split('/').pop()?.split('\\').pop()}
                </div>
                <div className="text-xs text-gray-500 mt-1">{file.lines} lines</div>
              </li>
            ))}
          </ul>
        )}

        {/* ACE Discovered Context */}
        {(discoveredContext.tech.length > 0 || discoveredContext.topics.length > 0) && (
          <div className="mt-4 pt-4 border-t border-border">
            <div className="text-xs text-gray-500 mb-3 flex items-center gap-2">
              <span>{t('context.autoDiscoveredLabel')}</span>
              <span className="px-1.5 py-0.5 text-[10px] bg-orange-500/20 text-orange-400 rounded" title="Auto Context Engine - score boost from your local project context">ACE</span>
            </div>
            {discoveredContext.tech.length > 0 && (
              <div className="mb-3">
                <div className="flex flex-wrap gap-1.5">
                  {discoveredContext.tech.slice(0, 6).map((tech) => (
                    <span
                      key={tech.name}
                      className="px-2 py-1 text-[11px] bg-green-500/10 text-green-400 rounded-lg border border-green-500/20"
                      title={`${tech.category} - ${Math.round(tech.confidence * 100)}%`}
                    >
                      {tech.name}
                    </span>
                  ))}
                  {discoveredContext.tech.length > 6 && (
                    <span className="text-[11px] text-gray-500 self-center">+{discoveredContext.tech.length - 6}</span>
                  )}
                </div>
              </div>
            )}
            {discoveredContext.topics.length > 0 && (
              <div className="flex flex-wrap gap-1.5">
                {discoveredContext.topics.slice(0, 4).map((topic) => (
                  <span
                    key={topic}
                    className="px-2 py-1 text-[11px] bg-orange-500/10 text-orange-400 rounded-lg border border-orange-500/20"
                  >
                    {topic}
                  </span>
                ))}
                {discoveredContext.topics.length > 4 && (
                  <span className="text-[11px] text-gray-500 self-center">+{discoveredContext.topics.length - 4}</span>
                )}
              </div>
            )}
          </div>
        )}
      </div>
    </section>
  );
}
