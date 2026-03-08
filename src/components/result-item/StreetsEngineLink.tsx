import { useTranslation } from 'react-i18next';
import type { SourceRelevance } from '../../types';
import { useAppStore } from '../../store';

interface StreetsEngineLinkProps {
  item: SourceRelevance;
}

interface EngineTemplate {
  /** Template with {topic} placeholder for contextual descriptions */
  contextual: string;
  /** Fallback if title doesn't yield a useful topic */
  fallback: string;
}

const ENGINE_TEMPLATES: Record<string, EngineTemplate> = {
  'Engine 1: Digital Products': {
    contextual: 'Package {topic} knowledge into a premium template, starter kit, or guide.',
    fallback: 'Build templates, starter kits, or premium guides from this knowledge.',
  },
  'Engine 2: Content': {
    contextual: 'Turn {topic} into a tutorial series, course, or newsletter that builds authority.',
    fallback: 'Turn this into tutorials, courses, or newsletter content.',
  },
  'Engine 3: Micro-SaaS': {
    contextual: 'Wrap {topic} as a small SaaS tool — solve one problem, charge monthly.',
    fallback: 'This signals a Micro-SaaS opportunity — small tool, recurring revenue.',
  },
  'Engine 4: Automation': {
    contextual: 'Automate {topic} workflows — bots, scrapers, or CI/CD pipelines others need.',
    fallback: 'Build bots, scrapers, or workflow automations around this.',
  },
  'Engine 5: API Products': {
    contextual: 'Expose {topic} as an API or data service developers would pay to use.',
    fallback: 'Wrap this as an API or data service others would pay for.',
  },
  'Engine 6: Consulting': {
    contextual: '{topic} expertise is scarce — position yourself for advisory or consulting.',
    fallback: 'Deep expertise signal — position as advisory or consulting.',
  },
  'Engine 7: Open Source+': {
    contextual: 'Open-source {topic} with a premium tier for teams or enterprise support.',
    fallback: 'Open-source core with a premium tier or support layer.',
  },
};

/**
 * Extract a short topic phrase from the item title for contextual descriptions.
 * Strips common prefixes like "Show HN:", trims to key subject.
 */
function extractTopic(title: string): string | null {
  let t = title
    .replace(/^(Show HN|Ask HN|Tell HN|Launch HN):\s*/i, '')
    .replace(/^(Introducing|Announcing|New|Just released):\s*/i, '')
    .trim();

  // Take first meaningful phrase (before a dash, colon, or pipe)
  const sep = t.search(/\s[—–\-|:]\s/);
  if (sep > 5) t = t.slice(0, sep);

  // Too short or too long = not useful
  if (t.length < 4 || t.length > 60) return null;

  // Lowercase first char for natural sentence insertion
  return t.charAt(0).toLowerCase() + t.slice(1);
}

/**
 * Shows a STREETS revenue engine connection inline when a feed item
 * maps to a monetization engine. Links to the STREETS playbook view.
 */
export function StreetsEngineLink({ item }: StreetsEngineLinkProps) {
  const { t } = useTranslation();
  const setActiveView = useAppStore((s) => s.setActiveView);

  if (!item.streets_engine) return null;

  const template = ENGINE_TEMPLATES[item.streets_engine];
  const topic = extractTopic(item.title);

  const description = template
    ? (topic ? template.contextual.replace('{topic}', topic) : template.fallback)
    : t('results.streetsDefault');

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
      <p className="text-xs text-text-secondary leading-relaxed">
        {description}
      </p>
      <button
        onClick={() => setActiveView('playbook')}
        className="mt-1.5 text-[10px] text-yellow-400/60 hover:text-yellow-400 transition-colors"
      >
        {t('results.exploreInPlaybook', { engine: engineShort })}
      </button>
    </div>
  );
}
