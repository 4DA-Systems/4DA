import type { SourceRelevance } from '../../types';
import { useAppStore } from '../../store';

interface StreetsEngineLinkProps {
  item: SourceRelevance;
}

const ENGINE_DESCRIPTIONS: Record<string, string> = {
  'Engine 1: Digital Products': 'Build templates, starter kits, or premium guides from this knowledge.',
  'Engine 2: Content': 'Turn this into tutorials, courses, or newsletter content.',
  'Engine 3: Micro-SaaS': 'This signals a Micro-SaaS opportunity — small tool, recurring revenue.',
  'Engine 4: Automation': 'Build bots, scrapers, or workflow automations around this.',
  'Engine 5: API Products': 'Wrap this as an API or data service others would pay for.',
  'Engine 6: Consulting': 'Deep expertise signal — position as advisory or consulting.',
  'Engine 7: Open Source+': 'Open-source core with a premium tier or support layer.',
};

/**
 * Shows a STREETS revenue engine connection inline when a feed item
 * maps to a monetization engine. Links to the STREETS playbook view.
 */
export function StreetsEngineLink({ item }: StreetsEngineLinkProps) {
  const setActiveView = useAppStore((s) => s.setActiveView);

  if (!item.streets_engine) return null;

  const description = ENGINE_DESCRIPTIONS[item.streets_engine] ||
    'This content maps to a STREETS revenue engine.';

  const engineShort = item.streets_engine.replace(/^Engine \d+: /, '');

  return (
    <div className="mb-3 p-2 bg-yellow-500/5 rounded border border-yellow-500/20">
      <div className="flex items-center gap-2 mb-1">
        <span className="text-[10px] font-medium text-yellow-400 uppercase tracking-wider">
          STREETS
        </span>
        <span className="text-[10px] text-yellow-400/70">
          {item.streets_engine}
        </span>
      </div>
      <p className="text-xs text-gray-400 leading-relaxed">
        {description}
      </p>
      <button
        onClick={() => setActiveView('playbook')}
        className="mt-1.5 text-[10px] text-yellow-400/60 hover:text-yellow-400 transition-colors"
      >
        Explore {engineShort} in STREETS Playbook &rarr;
      </button>
    </div>
  );
}
