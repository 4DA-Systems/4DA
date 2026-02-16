import type { ScanProgress } from './types';

interface FirstScanStepProps {
  isAnimating: boolean;
  isScanning: boolean;
  scanProgress: ScanProgress | null;
  onRunScan: () => void;
  onComplete: () => void;
  onBack: () => void;
}

export function FirstScanStep({
  isAnimating,
  isScanning,
  scanProgress,
  onRunScan,
  onComplete,
  onBack,
}: FirstScanStepProps) {
  return (
    <div className={`transition-all duration-300 ${isAnimating ? 'opacity-0 translate-y-4' : 'opacity-100 translate-y-0'}`}>
      <h2 className="text-3xl font-semibold text-white mb-2 text-center">Deep Intelligence Scan</h2>
      <p className="text-gray-400 mb-6 text-center">
        Comprehensive scan of 300-500+ items across all sources to build your personalized feed.
      </p>

      <div className="bg-bg-secondary p-6 rounded-lg mb-6">
        {/* Scanning state visualization */}
        {scanProgress?.phase === 'fetching' && (
          <div className="text-center py-8">
            <div className="w-20 h-20 mx-auto mb-4 relative">
              <div className="absolute inset-0 rounded-full border-4 border-orange-500/20" />
              <div className="absolute inset-0 rounded-full border-4 border-orange-500 border-t-transparent animate-spin" style={{ animationDuration: '1.5s' }} />
              <span className="absolute inset-0 flex items-center justify-center text-3xl">&#x1f52c;</span>
            </div>
            <h3 className="text-white font-medium mb-2">Deep Scan in Progress</h3>
            <p className="text-sm text-gray-400 mb-3">{scanProgress.message}</p>
            <div className="flex flex-wrap justify-center gap-2 mt-4">
              <span className="px-2 py-1 bg-orange-500/20 text-orange-300 text-xs rounded animate-pulse">HN Top</span>
              <span className="px-2 py-1 bg-orange-500/20 text-orange-300 text-xs rounded animate-pulse delay-100">HN New</span>
              <span className="px-2 py-1 bg-orange-500/20 text-orange-300 text-xs rounded animate-pulse delay-200">HN Best</span>
              <span className="px-2 py-1 bg-purple-500/20 text-purple-300 text-xs rounded animate-pulse delay-300">arXiv AI</span>
              <span className="px-2 py-1 bg-purple-500/20 text-purple-300 text-xs rounded animate-pulse delay-400">arXiv ML</span>
              <span className="px-2 py-1 bg-blue-500/20 text-blue-300 text-xs rounded animate-pulse delay-500">Reddit</span>
              <span className="px-2 py-1 bg-green-500/20 text-green-300 text-xs rounded animate-pulse delay-600">GitHub</span>
            </div>
            <p className="text-xs text-gray-500 mt-4">This comprehensive scan may take 2-5 minutes</p>
          </div>
        )}

        {scanProgress?.phase === 'scoring' && (
          <div className="text-center py-8">
            <div className="w-20 h-20 mx-auto mb-4 relative">
              <div className="absolute inset-0 rounded-full border-4 border-cyan-500/20" />
              <div className="absolute inset-0 rounded-full border-4 border-cyan-500 border-t-transparent animate-spin" />
              <span className="absolute inset-0 flex items-center justify-center text-3xl">&#x1f916;</span>
            </div>
            <h3 className="text-white font-medium mb-2">Analyzing Relevance</h3>
            <p className="text-sm text-gray-400">{scanProgress.message}</p>
            <div className="w-48 h-1 bg-bg-tertiary rounded-full mx-auto mt-4 overflow-hidden">
              <div className="h-full bg-gradient-to-r from-cyan-500 to-orange-500 rounded-full animate-pulse" style={{ width: '70%' }} />
            </div>
          </div>
        )}

        {scanProgress?.phase === 'done' && (
          <div className="py-4">
            <div className="flex items-center justify-center gap-3 mb-6">
              <div className="w-12 h-12 bg-green-500/20 rounded-full flex items-center justify-center">
                <span className="text-2xl">&#x2713;</span>
              </div>
              <div className="text-left">
                <h3 className="text-white font-medium">{scanProgress.message}</h3>
                <p className="text-sm text-gray-400">
                  Analyzed {scanProgress.total} items, {scanProgress.relevant} match your profile
                </p>
              </div>
            </div>

            {scanProgress.results && scanProgress.results.length > 0 ? (
              <div className="space-y-2">
                <p className="text-xs text-gray-500 mb-3">Top results for you:</p>
                {scanProgress.results.map((result, i) => (
                  <div
                    key={i}
                    className="flex items-center gap-3 p-3 bg-bg-tertiary rounded-lg hover:bg-border transition-colors"
                  >
                    <span className={`px-2 py-0.5 text-xs rounded ${
                      result.source === 'HN' ? 'bg-orange-500/20 text-orange-300' :
                      result.source === 'arXiv' ? 'bg-purple-500/20 text-purple-300' :
                      'bg-blue-500/20 text-blue-300'
                    }`}>
                      {result.source}
                    </span>
                    <span className="flex-1 text-sm text-gray-300 truncate">{result.title}</span>
                    <span className="text-xs text-green-400 font-mono">{result.score}%</span>
                  </div>
                ))}
              </div>
            ) : (
              <div className="text-center py-4 bg-bg-tertiary rounded-lg">
                <p className="text-gray-400">No highly relevant items found yet.</p>
                <p className="text-sm text-gray-500 mt-1">
                  4DA will learn your preferences as you give feedback.
                </p>
              </div>
            )}
          </div>
        )}

        {scanProgress?.phase === 'error' && (
          <div className="text-center py-8">
            <div className="w-16 h-16 mx-auto mb-4 bg-red-500/20 rounded-full flex items-center justify-center">
              <span className="text-3xl">&#x26a0;</span>
            </div>
            <h3 className="text-red-300 font-medium mb-2">Scan encountered an issue</h3>
            <p className="text-sm text-gray-400">{scanProgress.message}</p>
            <button
              onClick={onRunScan}
              className="mt-4 px-4 py-2 bg-bg-tertiary text-gray-300 rounded-lg hover:bg-border transition-colors"
            >
              Retry Scan
            </button>
          </div>
        )}

        {!scanProgress && (
          <div className="text-center py-8">
            <div className="w-16 h-16 mx-auto mb-4 bg-gradient-to-br from-orange-500/20 to-purple-500/20 rounded-full flex items-center justify-center">
              <span className="text-3xl">&#x1f52c;</span>
            </div>
            <h3 className="text-white font-medium mb-2">Ready for Deep Scan</h3>
            <p className="text-sm text-gray-400 mb-4 max-w-sm mx-auto">
              We&apos;ll comprehensively scan 300-500+ items from HN (5 categories), arXiv (16 fields),
              Reddit (40+ subs), and GitHub to build your personalized intelligence feed.
            </p>
            <button
              onClick={onRunScan}
              className="px-6 py-3 bg-gradient-to-r from-orange-500 to-orange-600 text-white rounded-lg hover:from-orange-600 hover:to-orange-700 transition-all font-medium shadow-lg shadow-orange-500/20"
            >
              Start Deep Scan
            </button>
            <p className="text-xs text-gray-500 mt-3">Takes 2-5 minutes for comprehensive results</p>
          </div>
        )}
      </div>

      <div className="flex justify-between items-center">
        <button
          onClick={onBack}
          disabled={isScanning}
          className="px-6 py-2 text-gray-400 hover:text-white transition-colors disabled:opacity-50"
        >
          &larr; Back
        </button>
        <div className="flex items-center gap-3">
          {scanProgress?.phase !== 'done' && !isScanning && (
            <button
              onClick={onComplete}
              className="px-4 py-2 text-gray-500 hover:text-gray-300 text-sm transition-colors"
            >
              Skip for now
            </button>
          )}
          <button
            onClick={onComplete}
            disabled={isScanning || (scanProgress?.phase !== 'done' && scanProgress?.phase !== 'error')}
            className="px-8 py-3 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-colors font-medium disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {scanProgress?.phase === 'done' ? 'See My Results \u2192' : 'Continue'}
          </button>
        </div>
      </div>
    </div>
  );
}
