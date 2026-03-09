import { memo } from 'react';
import { useAppStore } from '../../store';

export const ShowAllViewsToggle = memo(function ShowAllViewsToggle() {
  const showAllViews = useAppStore(s => s.showAllViews);
  const setShowAllViews = useAppStore(s => s.setShowAllViews);

  return (
    <div className="flex items-center justify-between py-3">
      <div>
        <span className="text-white text-sm">Show all views</span>
        <p className="text-text-muted text-xs">Display all 9 navigation tabs regardless of usage</p>
      </div>
      <button
        onClick={() => setShowAllViews(!showAllViews)}
        role="switch"
        aria-checked={showAllViews}
        aria-label="Show all views"
        className={`relative w-10 h-5 rounded-full transition-colors ${
          showAllViews ? 'bg-green-500/40' : 'bg-gray-600'
        }`}
      >
        <span className={`absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-white transition-transform ${
          showAllViews ? 'translate-x-5' : 'translate-x-0'
        }`} />
      </button>
    </div>
  );
});
