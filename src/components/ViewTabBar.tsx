import { useAppStore } from '../store';

type ViewId = 'briefing' | 'results' | 'insights' | 'saved' | 'toolkit' | 'playbook';

const TABS: Array<{ id: ViewId; label: string; subtitle: string; activeColor: string }> = [
  { id: 'briefing', label: 'Intelligence', subtitle: 'AI briefing & signals', activeColor: 'bg-orange-500/20 text-orange-400' },
  { id: 'results', label: 'All Results', subtitle: 'Scored content feed', activeColor: 'bg-orange-500/20 text-orange-400' },
  { id: 'insights', label: 'Insights', subtitle: 'Trends & knowledge gaps', activeColor: 'bg-amber-500/20 text-amber-400' },
  { id: 'saved', label: 'Saved', subtitle: 'Bookmarked items', activeColor: 'bg-green-500/20 text-green-400' },
  { id: 'toolkit', label: 'Toolkit', subtitle: 'Dev tools & health', activeColor: 'bg-purple-500/20 text-purple-400' },
  { id: 'playbook', label: 'Playbook', subtitle: 'STREETS modules', activeColor: 'bg-yellow-500/20 text-yellow-400' },
];

export function ViewTabBar() {
  const activeView = useAppStore(s => s.activeView);
  const setActiveView = useAppStore(s => s.setActiveView);

  return (
    <nav aria-label="Main views">
    <div className="mb-6 flex items-center gap-1 bg-bg-secondary rounded-lg p-1 border border-border w-fit" role="tablist" aria-label="Content views">
      {TABS.map(tab => (
        <button
          key={tab.id}
          role="tab"
          aria-selected={activeView === tab.id}
          aria-controls={`view-panel-${tab.id}`}
          onClick={() => setActiveView(tab.id)}
          className={`px-4 py-1.5 text-sm rounded-md transition-all ${
            activeView === tab.id
              ? `${tab.activeColor} font-medium`
              : 'text-gray-500 hover:text-gray-300'
          }`}
          title={tab.subtitle}
        >
          <span>{tab.label}</span>
          <span className={`block text-[10px] leading-tight ${
            activeView === tab.id ? 'opacity-70' : 'opacity-40'
          }`}>{tab.subtitle}</span>
        </button>
      ))}
    </div>
    </nav>
  );
}
