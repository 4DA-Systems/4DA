// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { useState, useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../../store';
import { TOOLS, getToolById } from './tool-registry';
import { ToolkitShell } from './ToolkitShell';

export function ToolkitView() {
  const { t } = useTranslation();
  const [activeTool, setActiveTool] = useState<string | null>(null);
  const addRecentTool = useAppStore((s) => s.addRecentTool);

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

  // Render active tool
  const activeToolDescriptor = activeTool ? getToolById(activeTool) : null;
  if (activeTool && activeToolDescriptor) {
    const ToolComponent = activeToolDescriptor.component;
    return (
      <ToolkitShell toolName={activeToolDescriptor.name} onBack={closeTool}>
        <div role="tabpanel" aria-labelledby={`toolkit-tab-${activeTool}`}>
          <ToolComponent />
        </div>
      </ToolkitShell>
    );
  }

  // Render tool cards
  return (
    <section aria-label={t('toolkit.title')}>
      <div role="tablist" aria-label={t('toolkit.title')} className="grid grid-cols-1 sm:grid-cols-2 gap-4 max-w-2xl">
        {TOOLS.map((tool) => (
          <button
            key={tool.id}
            id={`toolkit-tab-${tool.id}`}
            role="tab"
            aria-selected={activeTool === tool.id}
            onClick={() => openTool(tool.id)}
            className="flex flex-col gap-2 p-5 bg-bg-secondary border border-border rounded-xl text-start hover:border-white/20 hover:bg-bg-tertiary transition-all group"
          >
            <h3 className="text-sm font-medium text-white group-hover:text-orange-400 transition-colors">
              {tool.name}
            </h3>
            <p className="text-xs text-text-muted leading-relaxed">
              {tool.description}
            </p>
          </button>
        ))}
      </div>
    </section>
  );
}
