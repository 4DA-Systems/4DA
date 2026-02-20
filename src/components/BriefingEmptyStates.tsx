import { useAppStore } from '../store';

/** Analysis in progress — spinner + "Gathering Intelligence" */
export function BriefingLoadingState() {
  const results = useAppStore(s => s.appState.relevanceResults);
  const setActiveView = useAppStore(s => s.setActiveView);

  return (
    <div className="bg-bg-primary rounded-lg" role="status" aria-busy="true" aria-label="Gathering intelligence">
      <div className="flex flex-col items-center justify-center py-20 px-8">
        <div className="w-20 h-20 mb-6 bg-orange-500/10 rounded-2xl border border-orange-500/20 flex items-center justify-center">
          <div className="w-6 h-6 border-2 border-orange-400 border-t-transparent rounded-full animate-spin" />
        </div>
        <h2 className="text-xl font-medium text-white mb-2">Gathering Intelligence</h2>
        <p className="text-sm text-gray-500 text-center max-w-md">
          Analysis is running. Your briefing will be generated when results are ready.
        </p>
        {results.length > 0 && (
          <button onClick={() => setActiveView('results')} className="mt-6 text-sm text-gray-500 hover:text-gray-300 transition-colors">
            Browse {results.length} results while you wait
          </button>
        )}
      </div>
    </div>
  );
}

/** Analysis done, briefing available to generate */
export function BriefingReadyState() {
  const results = useAppStore(s => s.appState.relevanceResults);
  const generateBriefing = useAppStore(s => s.generateBriefing);

  return (
    <div className="bg-bg-primary rounded-lg">
      <div className="flex flex-col items-center justify-center py-20 px-8">
        <div className="w-20 h-20 mb-6 bg-bg-secondary rounded-2xl border border-orange-500/20 flex items-center justify-center">
          <svg className="w-8 h-8 text-orange-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M11.049 2.927c.3-.921 1.603-.921 1.902 0l1.519 4.674a1 1 0 00.95.69h4.915c.969 0 1.371 1.24.588 1.81l-3.976 2.888a1 1 0 00-.363 1.118l1.518 4.674c.3.922-.755 1.688-1.538 1.118l-3.976-2.888a1 1 0 00-1.176 0l-3.976 2.888c-.783.57-1.838-.197-1.538-1.118l1.518-4.674a1 1 0 00-.363-1.118l-3.976-2.888c-.784-.57-.38-1.81.588-1.81h4.914a1 1 0 00.951-.69l1.519-4.674z" />
          </svg>
        </div>
        <h2 className="text-xl font-medium text-white mb-2">Briefing Ready to Generate</h2>
        <p className="text-sm text-gray-500 text-center max-w-md mb-6">
          {results.length} results analyzed. Generate an AI briefing to surface what matters most.
        </p>
        <button onClick={generateBriefing} className="px-6 py-2.5 bg-orange-500 text-white text-sm font-medium rounded-lg hover:bg-orange-600 transition-colors">
          Generate Briefing
        </button>
      </div>
    </div>
  );
}

/** No analysis yet — "Analyze Now" CTA */
export function BriefingNoDataState() {
  const startAnalysis = useAppStore(s => s.startAnalysis);

  return (
    <div className="bg-bg-primary rounded-lg">
      <div className="flex flex-col items-center justify-center py-20 px-8">
        <div className="w-20 h-20 mb-6 bg-bg-secondary rounded-2xl border border-border flex items-center justify-center">
          <svg className="w-8 h-8 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
          </svg>
        </div>
        <h2 className="text-xl font-medium text-white mb-2">No Intelligence Yet</h2>
        <p className="text-sm text-gray-500 text-center max-w-md mb-6">
          Run an analysis to gather content from your sources, then generate a briefing.
        </p>
        <button onClick={startAnalysis} className="px-6 py-2.5 bg-orange-500 text-white text-sm font-medium rounded-lg hover:bg-orange-600 transition-colors">
          Analyze Now
        </button>
        <p className="text-xs text-gray-600 mt-3">
          or press <kbd className="px-1.5 py-0.5 bg-bg-tertiary rounded text-gray-500">R</kbd>
        </p>
      </div>
    </div>
  );
}
