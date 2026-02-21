import { useState, useCallback, useEffect, useMemo } from 'react';
import { useShallow } from 'zustand/react/shallow';
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

const UTILITY_CATEGORIES: Set<ToolCategory> = new Set(['formatters', 'encoders', 'generators', 'system', 'capture']);

export function ToolkitView() {
  const [activeTool, setActiveTool] = useState<string | null>(null);
  const [search, setSearch] = useState('');
  const [utilitiesExpanded, setUtilitiesExpanded] = useState(false);
  // Data selectors (may change, use useShallow)
  const { pinnedTools } = useAppStore(
    useShallow((s) => ({
      pinnedTools: s.pinnedTools,
    })),
  );
  // Action selectors (stable references, no need for useShallow)
  const addRecentTool = useAppStore((s) => s.addRecentTool);
  const togglePinnedTool = useAppStore((s) => s.togglePinnedTool);
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

  // Split into intelligence vs utility tools
  const { pinned, intelligenceTools, utilityCategories, utilityCount } = useMemo(() => {
    const pinned = filtered.filter((t) => pinnedTools.includes(t.id));
    const unpinned = filtered.filter((t) => !pinnedTools.includes(t.id));

    const intelligence = unpinned.filter((t) => t.category === 'intelligence');
    const utilities = unpinned.filter((t) => UTILITY_CATEGORIES.has(t.category));

    const categories = new Map<ToolCategory, typeof TOOLS>();
    for (const tool of utilities) {
      const list = categories.get(tool.category) || [];
      list.push(tool);
      categories.set(tool.category, list);
    }

    return {
      pinned,
      intelligenceTools: intelligence,
      utilityCategories: categories,
      utilityCount: utilities.length,
    };
  }, [filtered, pinnedTools]);

  // Auto-expand utilities when searching
  useEffect(() => {
    if (search.trim()) setUtilitiesExpanded(true);
  }, [search]);

  // Derive active tool descriptor (never setState during render)
  const activeToolDescriptor = activeTool ? getToolById(activeTool) : null;

  // Reset invalid tool ID outside of render
  useEffect(() => {
    if (activeTool && !activeToolDescriptor) {
      setActiveTool(null);
    }
  }, [activeTool, activeToolDescriptor]);

  // Render active tool
  if (activeTool && activeToolDescriptor) {
    const ToolComponent = activeToolDescriptor.component;
    const content = (
      <ToolkitShell toolName={activeToolDescriptor.name} onBack={closeTool}>
        <ToolComponent />
      </ToolkitShell>
    );
    return activeToolDescriptor.pro && !isPro ? <ProGate feature={activeToolDescriptor.name}>{content}</ProGate> : content;
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
      {pinned.length > 0 && (
        <div className="mb-6">
          <h3 className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-3">Pinned</h3>
          <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-3">
            {pinned.map((tool) => (
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

      {/* Intelligence tools — always visible */}
      {intelligenceTools.length > 0 && (
        <div className="mb-6">
          <h3 className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-3">
            {CATEGORY_LABELS.intelligence}
          </h3>
          <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-3">
            {intelligenceTools.map((tool) => (
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
      )}

      {/* Utility tools — collapsible */}
      {utilityCount > 0 && (
        <div className="mb-6">
          <button
            onClick={() => setUtilitiesExpanded(!utilitiesExpanded)}
            className="flex items-center gap-2 mb-3 group"
          >
            <svg
              width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"
              className={`text-gray-500 transition-transform ${utilitiesExpanded ? 'rotate-90' : ''}`}
            >
              <path d="m9 18 6-6-6-6"/>
            </svg>
            <h3 className="text-xs font-medium text-gray-500 uppercase tracking-wider group-hover:text-gray-400 transition-colors">
              Utilities ({utilityCount})
            </h3>
          </button>

          {utilitiesExpanded && (
            <div className="space-y-6">
              {Array.from(utilityCategories.entries()).map(([category, tools]) => (
                <div key={category}>
                  <h4 className="text-xs font-medium text-gray-600 uppercase tracking-wider mb-3 pl-5">
                    {CATEGORY_LABELS[category]}
                  </h4>
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
            </div>
          )}
        </div>
      )}

      {/* Empty state */}
      {filtered.length === 0 && (
        <div className="text-center py-12">
          <p className="text-sm text-gray-500">No tools match "{search}"</p>
        </div>
      )}
    </div>
  );
}
