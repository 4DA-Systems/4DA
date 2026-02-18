import { useState, useCallback, useEffect, useMemo } from 'react';
import { useAppStore } from '../../store';
import { useLicense } from '../../hooks/use-license';
import { TOOLS, getToolById } from './tool-registry';
import { ToolkitCard } from './ToolkitCard';
import { ToolkitShell } from './ToolkitShell';
import { ProGate } from '../ProGate';
import type { ToolCategory } from '../../types/toolkit';

const CATEGORY_LABELS: Record<ToolCategory, string> = {
  formatters: 'Formatters & Viewers',
  encoders: 'Encode / Decode',
  generators: 'Generators',
  system: 'System',
  intelligence: 'Intelligence',
  capture: 'Capture',
};

export function ToolkitView() {
  const [activeTool, setActiveTool] = useState<string | null>(null);
  const [search, setSearch] = useState('');
  const { pinnedTools, addRecentTool, togglePinnedTool } = useAppStore((s) => ({
    pinnedTools: s.pinnedTools,
    addRecentTool: s.addRecentTool,
    togglePinnedTool: s.togglePinnedTool,
  }));
  const { isPro } = useLicense();

  const openTool = useCallback((toolId: string) => {
    setActiveTool(toolId);
    addRecentTool(toolId);
  }, [addRecentTool]);

  const closeTool = useCallback(() => setActiveTool(null), []);

  // Escape to close active tool
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && activeTool) {
        e.stopPropagation();
        closeTool();
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [activeTool, closeTool]);

  // Filter tools by search
  const filtered = useMemo(() => {
    if (!search.trim()) return TOOLS;
    const q = search.toLowerCase();
    return TOOLS.filter((t) =>
      t.name.toLowerCase().includes(q) ||
      t.description.toLowerCase().includes(q) ||
      t.keywords.some((k) => k.includes(q)),
    );
  }, [search]);

  // Group by category
  const grouped = useMemo(() => {
    const pinned = filtered.filter((t) => pinnedTools.includes(t.id));
    const unpinned = filtered.filter((t) => !pinnedTools.includes(t.id));
    const categories = new Map<ToolCategory, typeof TOOLS>();
    for (const tool of unpinned) {
      const list = categories.get(tool.category) || [];
      list.push(tool);
      categories.set(tool.category, list);
    }
    return { pinned, categories };
  }, [filtered, pinnedTools]);

  // Render active tool
  if (activeTool) {
    const tool = getToolById(activeTool);
    if (!tool) { setActiveTool(null); return null; }
    const ToolComponent = tool.component;
    const content = (
      <ToolkitShell toolName={tool.name} onBack={closeTool}>
        <ToolComponent />
      </ToolkitShell>
    );
    return tool.pro && !isPro ? <ProGate feature={tool.name}>{content}</ProGate> : content;
  }

  // Render tool grid
  return (
    <div>
      {/* Search */}
      <div className="mb-5">
        <div className="relative max-w-sm">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-600">
            <circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>
          </svg>
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search tools..."
            className="w-full pl-9 pr-3 py-2 text-sm bg-bg-secondary border border-border rounded-lg text-white placeholder:text-gray-600 focus:outline-none focus:border-white/30 transition-colors"
          />
        </div>
      </div>

      {/* Pinned tools */}
      {grouped.pinned.length > 0 && (
        <div className="mb-6">
          <h3 className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-3">Pinned</h3>
          <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-3">
            {grouped.pinned.map((tool) => (
              <ToolkitCard
                key={tool.id}
                tool={tool}
                pinned={true}
                onOpen={() => openTool(tool.id)}
                onTogglePin={() => togglePinnedTool(tool.id)}
              />
            ))}
          </div>
        </div>
      )}

      {/* Categorized tools */}
      {Array.from(grouped.categories.entries()).map(([category, tools]) => (
        <div key={category} className="mb-6">
          <h3 className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-3">
            {CATEGORY_LABELS[category]}
          </h3>
          <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-3">
            {tools.map((tool) => (
              <ToolkitCard
                key={tool.id}
                tool={tool}
                pinned={false}
                onOpen={() => openTool(tool.id)}
                onTogglePin={() => togglePinnedTool(tool.id)}
              />
            ))}
          </div>
        </div>
      ))}

      {/* Empty state */}
      {filtered.length === 0 && (
        <div className="text-center py-12">
          <p className="text-sm text-gray-500">No tools match "{search}"</p>
        </div>
      )}
    </div>
  );
}
