import { useTranslation } from 'react-i18next';

interface ContextDiscoverySectionProps {
  scanDirectories: string[];
  newScanDir: string;
  setNewScanDir: (val: string) => void;
  isScanning: boolean;
  discoveredContext: {
    tech: { name: string; category: string; confidence: number }[];
    topics: string[];
    lastScan: string | null;
  };
  runAutoDiscovery: () => void;
  runFullScan: () => void;
  addScanDirectory: () => void;
  removeScanDirectory: (dir: string) => void;
}

export function ContextDiscoverySection({
  scanDirectories,
  newScanDir,
  setNewScanDir,
  isScanning,
  discoveredContext,
  runAutoDiscovery,
  runFullScan,
  addScanDirectory,
  removeScanDirectory,
}: ContextDiscoverySectionProps) {
  const { t } = useTranslation();
  return (
    <div className="bg-bg-tertiary rounded-lg p-5 border border-border">
      <div className="flex items-start gap-3 mb-4">
        <div className="w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center flex-shrink-0">
          <span className="text-orange-400">&#x1f50d;</span>
        </div>
        <div className="flex-1">
          <div className="flex items-center gap-2">
            <h3 className="text-white font-medium">{t('settings.context.title')}</h3>
            <span className="px-2 py-0.5 text-[10px] bg-orange-500/20 text-orange-400 rounded-full font-medium">ACE</span>
          </div>
          <p className="text-text-muted text-sm mt-1">
            {t('settings.context.description')}
          </p>
        </div>
      </div>

      <div className="space-y-4">
        <button
          onClick={runAutoDiscovery}
          disabled={isScanning}
          aria-label={isScanning ? t('settings.context.discovering') : t('settings.context.autoDiscover')}
          className="w-full px-4 py-3 text-sm bg-gradient-to-r from-orange-500/20 to-orange-600/10 text-orange-400 border border-orange-500/30 rounded-lg hover:from-orange-500/30 hover:to-orange-600/20 transition-all disabled:opacity-50 disabled:cursor-not-allowed font-medium"
        >
          {isScanning ? t('settings.context.discovering') : t('settings.context.autoDiscover')}
        </button>

        <div className="flex gap-2">
          <input
            type="text"
            aria-label={t('settings.context.dirPathLabel')}
            value={newScanDir}
            onChange={(e) => setNewScanDir(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && addScanDirectory()}
            placeholder={t('settings.context.addDirPlaceholder')}
            className="flex-1 px-3 py-2.5 bg-bg-secondary border border-border rounded-lg text-sm text-white placeholder:text-text-muted focus:border-orange-500/50 focus:outline-none transition-colors"
          />
          <button
            onClick={addScanDirectory}
            aria-label={t('settings.context.addDirectory')}
            className="px-4 py-2.5 text-sm bg-bg-secondary border border-border rounded-lg text-text-secondary hover:text-white hover:border-orange-500/30 transition-all"
          >
            {t('action.add')}
          </button>
        </div>

        <div className="space-y-2">
          {scanDirectories.length === 0 ? (
            <p className="text-sm text-text-muted text-center py-3">{t('settings.context.noDirs')}</p>
          ) : (
            <>
              <div className="text-xs text-text-muted mb-2">{t('settings.context.configuredDirs', { count: scanDirectories.length })}</div>
              <div className="space-y-1.5 max-h-32 overflow-y-auto">
                {scanDirectories.map((dir) => (
                  <div key={dir} className="flex items-center justify-between px-3 py-2 bg-bg-secondary rounded-lg border border-border group">
                    <span className="font-mono text-sm text-white truncate">{dir}</span>
                    <button
                      onClick={() => removeScanDirectory(dir)}
                      aria-label={t('settings.context.removeDir', { dir })}
                      className="text-text-muted hover:text-red-400 ml-2 opacity-0 group-hover:opacity-100 focus:opacity-100 transition-opacity"
                    >
                      &times;
                    </button>
                  </div>
                ))}
              </div>
            </>
          )}
        </div>

        {scanDirectories.length > 0 && (
          <button
            onClick={runFullScan}
            disabled={isScanning}
            className="w-full px-4 py-2.5 text-sm bg-bg-secondary border border-border rounded-lg text-text-secondary hover:text-white hover:border-orange-500/30 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {isScanning ? t('settings.context.scanning') : t('settings.context.rescan')}
          </button>
        )}

        {(discoveredContext.tech.length > 0 || discoveredContext.topics.length > 0) && (
          <div className="bg-bg-secondary rounded-lg p-4 border border-border space-y-3">
            <div className="text-xs text-text-muted flex items-center gap-2">
              <span className="w-1.5 h-1.5 bg-green-500 rounded-full animate-pulse" />
              {t('settings.context.discoveredContext')} {discoveredContext.lastScan && `(${new Date(discoveredContext.lastScan).toLocaleDateString()})`}
            </div>
            {discoveredContext.tech.length > 0 && (
              <div>
                <div className="text-xs text-text-secondary mb-2">{t('settings.context.techStack')}</div>
                <div className="flex flex-wrap gap-1.5">
                  {discoveredContext.tech.slice(0, 10).map((tech) => (
                    <span
                      key={tech.name}
                      className="px-2 py-1 text-xs bg-green-500/10 text-green-400 rounded-md border border-green-500/20"
                      title={`${tech.category} - ${Math.round(tech.confidence * 100)}% confidence`}
                    >
                      {tech.name}
                    </span>
                  ))}
                  {discoveredContext.tech.length > 10 && (
                    <span className="text-xs text-text-muted self-center">{t('settings.context.more', { count: discoveredContext.tech.length - 10 })}</span>
                  )}
                </div>
              </div>
            )}
            {discoveredContext.topics.length > 0 && (
              <div>
                <div className="text-xs text-text-secondary mb-2">{t('settings.context.topics')}</div>
                <div className="flex flex-wrap gap-1.5">
                  {discoveredContext.topics.slice(0, 8).map((topic) => (
                    <span
                      key={topic}
                      className="px-2 py-1 text-xs bg-orange-500/10 text-orange-400 rounded-md border border-orange-500/20"
                    >
                      {topic}
                    </span>
                  ))}
                  {discoveredContext.topics.length > 8 && (
                    <span className="text-xs text-text-muted self-center">{t('settings.context.more', { count: discoveredContext.topics.length - 8 })}</span>
                  )}
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
