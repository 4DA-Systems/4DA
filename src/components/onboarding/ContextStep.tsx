interface ContextStepProps {
  isAnimating: boolean;
  isDiscovering: boolean;
  discoveryResult: string | null;
  onDiscovery: () => void;
  onNext: () => void;
  onBack: () => void;
}

export function ContextStep({
  isAnimating,
  isDiscovering,
  discoveryResult,
  onDiscovery,
  onNext,
  onBack,
}: ContextStepProps) {
  return (
    <div className={`transition-all duration-300 ${isAnimating ? 'opacity-0 translate-y-4' : 'opacity-100 translate-y-0'}`}>
      <h2 className="text-3xl font-semibold text-white mb-2 text-center">Discover Your Context</h2>
      <p className="text-gray-400 mb-6 text-center">
        4DA learns what matters to you by scanning your projects. 100% local, 100% private.
      </p>

      <div className="bg-bg-secondary p-6 rounded-lg mb-4">
        {/* Discovery state visualization */}
        <div className="flex items-center gap-4 mb-6">
          <div className={`w-16 h-16 rounded-full flex items-center justify-center ${
            discoveryResult ? 'bg-green-500/20' : isDiscovering ? 'bg-orange-500/20' : 'bg-bg-tertiary'
          }`}>
            {discoveryResult ? (
              <span className="text-3xl">&#x2713;</span>
            ) : isDiscovering ? (
              <div className="w-8 h-8 border-3 border-orange-500 border-t-transparent rounded-full animate-spin" />
            ) : (
              <span className="text-3xl">&#x1f4c1;</span>
            )}
          </div>
          <div className="flex-1">
            <h3 className="text-white font-medium">
              {discoveryResult ? 'Discovery Complete!' : isDiscovering ? 'Scanning your projects...' : 'Auto-Discovery'}
            </h3>
            <p className="text-sm text-gray-400 mt-1">
              {discoveryResult || (isDiscovering
                ? 'Looking for code, notes, documents...'
                : 'Scans ~/Projects, ~/Code, ~/Documents and similar locations'
              )}
            </p>
          </div>
        </div>

        {/* Discovery action or result */}
        {!discoveryResult && !isDiscovering && (
          <button
            onClick={onDiscovery}
            className="w-full py-4 bg-orange-500/20 border-2 border-dashed border-orange-500/50 text-orange-300 rounded-lg hover:bg-orange-500/30 hover:border-orange-500 transition-all font-medium"
          >
            <span className="text-lg">&#x1f50d;</span> Scan My Computer
          </button>
        )}

        {isDiscovering && (
          <div className="w-full h-2 bg-bg-tertiary rounded-full overflow-hidden">
            <div className="h-full bg-orange-500 rounded-full animate-pulse" style={{ width: '60%' }} />
          </div>
        )}

        {discoveryResult && (
          <div className="bg-green-900/20 border border-green-500/30 text-green-300 p-4 rounded-lg">
            <div className="flex items-center gap-2">
              <span className="text-green-500">&#x2713;</span>
              {discoveryResult}
            </div>
            <p className="text-xs text-green-400/70 mt-2">
              4DA will continuously learn from your activity in these directories.
            </p>
          </div>
        )}

        <p className="text-xs text-gray-500 mt-4 text-center">
          {discoveryResult
            ? 'You can manage directories anytime in Settings'
            : 'Or skip this and add directories manually later'
          }
        </p>
      </div>

      {/* FAQ Section */}
      <div className="bg-bg-secondary rounded-lg p-4 mb-6">
        <details className="group">
          <summary className="flex items-center justify-between cursor-pointer text-sm text-gray-400 hover:text-gray-300 transition-colors">
            <span className="flex items-center gap-2">
              <span className="text-orange-400">?</span>
              Common questions about context scanning
            </span>
            <span className="text-xs group-open:rotate-180 transition-transform">&#x25bc;</span>
          </summary>
          <div className="mt-4 space-y-4 text-sm">
            <div className="bg-bg-tertiary rounded-lg p-3">
              <h4 className="text-white font-medium mb-1">What files are being scanned?</h4>
              <p className="text-gray-400 text-xs">
                4DA looks for project markers (package.json, Cargo.toml, README files, etc.),
                code files, and documents. It reads file names and contents to understand your work context.
              </p>
            </div>
            <div className="bg-bg-tertiary rounded-lg p-3">
              <h4 className="text-white font-medium mb-1">Is my data sent anywhere?</h4>
              <p className="text-gray-400 text-xs">
                <span className="text-green-400 font-medium">No.</span> All scanning happens 100% locally on your machine.
                Your file contents never leave your computer. Only when you use the AI features,
                small text snippets are sent to your chosen AI provider (and you control that).
              </p>
            </div>
            <div className="bg-bg-tertiary rounded-lg p-3">
              <h4 className="text-white font-medium mb-1">What does 4DA do with this information?</h4>
              <p className="text-gray-400 text-xs">
                It builds a local understanding of your interests (e.g., &quot;you work with Rust and React&quot;)
                to filter internet content and show you only what&apos;s relevant. This context stays in a
                local database on your machine.
              </p>
            </div>
            <div className="bg-bg-tertiary rounded-lg p-3">
              <h4 className="text-white font-medium mb-1">Can I control what gets scanned?</h4>
              <p className="text-gray-400 text-xs">
                Yes! You can add or remove directories anytime in Settings. 4DA automatically
                ignores sensitive locations like node_modules, .git folders, and hidden system files.
              </p>
            </div>
            <div className="bg-bg-tertiary rounded-lg p-3">
              <h4 className="text-white font-medium mb-1">How long does scanning take?</h4>
              <p className="text-gray-400 text-xs">
                Usually a few seconds to a minute depending on how many projects you have.
                The initial scan is the longest - after that, 4DA only checks for changes.
              </p>
            </div>
          </div>
        </details>
      </div>

      <div className="flex justify-between items-center">
        <button
          onClick={onBack}
          className="px-6 py-2 text-gray-400 hover:text-white transition-colors"
        >
          &larr; Back
        </button>
        <div className="flex items-center gap-3">
          {!discoveryResult && (
            <button
              onClick={onNext}
              className="px-4 py-2 text-gray-500 hover:text-gray-300 text-sm transition-colors"
            >
              Skip for now
            </button>
          )}
          <button
            onClick={onNext}
            className="px-8 py-3 bg-orange-500 text-white rounded-lg hover:bg-orange-600 transition-colors font-medium"
          >
            Continue
          </button>
        </div>
      </div>
    </div>
  );
}
