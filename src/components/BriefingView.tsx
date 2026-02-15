import { useMemo } from 'react';
import { BriefingCard } from './BriefingCard';
import { useAppStore } from '../store';

interface ParsedSection {
  title: string;
  lines: string[];
  type: 'action' | 'worth_knowing' | 'filtered' | 'general';
}

function classifySection(title: string): ParsedSection['type'] {
  const lower = title.toLowerCase();
  if (lower.includes('action') || lower.includes('urgent') || lower.includes('critical') || lower.includes('alert')) {
    return 'action';
  }
  if (lower.includes('worth knowing') || lower.includes('notable') || lower.includes('interesting') || lower.includes('watch')) {
    return 'worth_knowing';
  }
  if (lower.includes('filtered') || lower.includes('skip') || lower.includes('noise') || lower.includes('low')) {
    return 'filtered';
  }
  return 'general';
}

function parseBriefingContent(content: string): ParsedSection[] {
  const sections: ParsedSection[] = [];
  let currentSection: ParsedSection | null = null;

  for (const line of content.split('\n')) {
    if (line.startsWith('## ')) {
      if (currentSection) sections.push(currentSection);
      const title = line.replace('## ', '').trim();
      currentSection = {
        title,
        lines: [],
        type: classifySection(title),
      };
    } else if (currentSection) {
      currentSection.lines.push(line);
    } else {
      // Lines before the first section header
      if (!sections.length && line.trim()) {
        if (!currentSection) {
          currentSection = { title: 'Overview', lines: [], type: 'general' };
        }
        currentSection.lines.push(line);
      }
    }
  }

  if (currentSection) sections.push(currentSection);
  return sections;
}

function SectionAccent({ type }: { type: ParsedSection['type'] }) {
  switch (type) {
    case 'action':
      return <div className="w-1 h-full bg-orange-500 rounded-full flex-shrink-0" />;
    case 'worth_knowing':
      return <div className="w-1 h-full bg-blue-500 rounded-full flex-shrink-0" />;
    case 'filtered':
      return <div className="w-1 h-full bg-gray-600 rounded-full flex-shrink-0" />;
    default:
      return <div className="w-1 h-full bg-[#2A2A2A] rounded-full flex-shrink-0" />;
  }
}

function sectionTitleColor(type: ParsedSection['type']): string {
  switch (type) {
    case 'action': return 'text-orange-400';
    case 'worth_knowing': return 'text-blue-400';
    case 'filtered': return 'text-gray-500';
    default: return 'text-white';
  }
}

function renderLine(line: string, index: number, type: ParsedSection['type']) {
  const isMuted = type === 'filtered';
  const textColor = isMuted ? 'text-gray-600' : 'text-gray-300';

  // List items
  if (line.startsWith('- ') || line.startsWith('* ')) {
    const content = line.replace(/^[-*] /, '');
    return (
      <p key={index} className={`ml-3 my-1 text-sm ${textColor}`}>
        <span className={isMuted ? 'text-gray-600 mr-2' : 'text-orange-400 mr-2'}>--</span>
        {renderInlineFormatting(content)}
      </p>
    );
  }

  // Numbered items
  if (/^\d+\. /.test(line)) {
    const num = line.match(/^\d+/)?.[0];
    const content = line.replace(/^\d+\. /, '');
    return (
      <p key={index} className={`ml-3 my-1 text-sm ${textColor}`}>
        <span className={isMuted ? 'text-gray-600 mr-2' : 'text-orange-400 mr-2'}>{num}.</span>
        {renderInlineFormatting(content)}
      </p>
    );
  }

  // Empty lines
  if (!line.trim()) {
    return <div key={index} className="h-2" />;
  }

  // Regular text
  return (
    <p key={index} className={`my-1 text-sm ${textColor} leading-relaxed`}>
      {renderInlineFormatting(line)}
    </p>
  );
}

function renderInlineFormatting(text: string): React.ReactNode {
  // Handle bold **text**
  const parts = text.split(/(\*\*[^*]+\*\*)/g);
  return parts.map((part, i) => {
    if (part.startsWith('**') && part.endsWith('**')) {
      return <strong key={i} className="text-white font-medium">{part.slice(2, -2)}</strong>;
    }
    // Handle inline code `text`
    const codeParts = part.split(/(`[^`]+`)/g);
    return codeParts.map((codePart, j) => {
      if (codePart.startsWith('`') && codePart.endsWith('`')) {
        return (
          <code key={`${i}-${j}`} className="px-1 py-0.5 bg-[#2A2A2A] text-orange-400 rounded text-xs font-mono">
            {codePart.slice(1, -1)}
          </code>
        );
      }
      return codePart;
    });
  });
}

export function BriefingView() {
  // Read everything from store — zero props
  const briefing = useAppStore(s => s.aiBriefing);
  const results = useAppStore(s => s.appState.relevanceResults);
  const generateBriefing = useAppStore(s => s.generateBriefing);
  const recordInteraction = useAppStore(s => s.recordInteraction);
  const feedbackGiven = useAppStore(s => s.feedbackGiven);
  const setActiveView = useAppStore(s => s.setActiveView);

  const sections = useMemo(() => {
    if (!briefing.content) return [];
    return parseBriefingContent(briefing.content);
  }, [briefing.content]);

  // Top picks from results for the briefing cards
  const topItems = useMemo(() => {
    return results
      .filter(r => r.relevant && r.top_score >= 0.5)
      .slice(0, 8);
  }, [results]);

  // Loading skeleton
  if (briefing.loading) {
    return (
      <div className="bg-[#0A0A0A] rounded-lg">
        <div className="space-y-4">
          {/* Skeleton header */}
          <div className="bg-[#141414] rounded-lg border border-[#2A2A2A] p-6">
            <div className="flex items-center gap-3 mb-4">
              <div className="w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center">
                <div className="w-4 h-4 border-2 border-orange-400 border-t-transparent rounded-full animate-spin" />
              </div>
              <div>
                <div className="h-5 w-48 bg-[#1F1F1F] rounded animate-pulse" />
                <div className="h-3 w-32 bg-[#1F1F1F] rounded animate-pulse mt-2" />
              </div>
            </div>
            {/* Skeleton lines */}
            <div className="space-y-3">
              {Array.from({ length: 6 }).map((_, i) => (
                <div key={i} className="h-4 bg-[#1F1F1F] rounded animate-pulse" style={{ width: `${75 + Math.random() * 25}%` }} />
              ))}
            </div>
          </div>
          {/* Skeleton cards */}
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            {Array.from({ length: 4 }).map((_, i) => (
              <div key={i} className="bg-[#141414] rounded-lg border border-[#2A2A2A] p-4">
                <div className="flex gap-3">
                  <div className="w-10 h-6 bg-[#1F1F1F] rounded animate-pulse" />
                  <div className="flex-1 space-y-2">
                    <div className="h-4 bg-[#1F1F1F] rounded animate-pulse" />
                    <div className="h-3 bg-[#1F1F1F] rounded animate-pulse w-3/4" />
                  </div>
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    );
  }

  // Empty state: no briefing content and not loading
  if (!briefing.content) {
    return (
      <div className="bg-[#0A0A0A] rounded-lg">
        <div className="flex flex-col items-center justify-center py-20 px-8">
          <div className="w-20 h-20 mb-6 bg-[#141414] rounded-2xl border border-[#2A2A2A] flex items-center justify-center">
            <span className="text-4xl opacity-40">*</span>
          </div>
          <h2 className="text-xl font-medium text-white mb-2">Your Intelligence Briefing</h2>
          <p className="text-sm text-gray-500 text-center max-w-md mb-8">
            4DA will analyze results and surface what matters most.
            {results.length === 0
              ? ' Run an analysis first to gather results.'
              : ` ${results.length} results ready for analysis.`}
          </p>
          <button
            onClick={generateBriefing}
            disabled={results.length === 0}
            className="px-8 py-3.5 text-base bg-orange-500 text-white font-medium rounded-xl hover:bg-orange-600 transition-all disabled:opacity-30 disabled:cursor-not-allowed hover:scale-105 active:scale-95 shadow-lg shadow-orange-500/20"
          >
            {results.length === 0 ? 'Run Analysis First' : 'Generate Briefing'}
          </button>
          {results.length > 0 && (
            <button
              onClick={() => setActiveView('results')}
              className="mt-4 text-sm text-gray-500 hover:text-gray-300 transition-colors"
            >
              or view all {results.length} results directly
            </button>
          )}
        </div>
      </div>
    );
  }

  // Briefing content view
  return (
    <div className="bg-[#0A0A0A] rounded-lg space-y-6">
      {/* Briefing header */}
      <div className="bg-[#141414] rounded-lg border border-orange-500/20 overflow-hidden">
        <div className="px-5 py-4 border-b border-orange-500/10 flex items-center justify-between bg-orange-500/5">
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center">
              <span className="text-orange-400 text-sm">*</span>
            </div>
            <div>
              <h2 className="font-medium text-orange-400">Intelligence Briefing</h2>
              {briefing.model && (
                <span className="text-xs text-gray-500">via {briefing.model}</span>
              )}
            </div>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={generateBriefing}
              className="px-3 py-1.5 text-xs bg-[#1F1F1F] text-orange-400 border border-orange-500/30 rounded-lg hover:bg-orange-500/10 transition-all font-medium"
              title="Refresh briefing"
            >
              Refresh
            </button>
          </div>
        </div>

        {/* Parsed sections */}
        <div className="p-5 space-y-6">
          {sections.map((section, sIdx) => (
            <div key={sIdx} className="flex gap-3">
              <SectionAccent type={section.type} />
              <div className="flex-1 min-w-0">
                <h3 className={`text-sm font-medium mb-2 ${sectionTitleColor(section.type)}`}>
                  {section.title}
                </h3>
                <div>
                  {section.lines.map((line, lIdx) => renderLine(line, lIdx, section.type))}
                </div>
              </div>
            </div>
          ))}
        </div>

        {briefing.lastGenerated && (
          <div className="px-5 py-3 border-t border-[#2A2A2A] text-xs text-gray-500">
            Generated {briefing.lastGenerated.toLocaleTimeString()}
          </div>
        )}
      </div>

      {/* Top items as BriefingCards */}
      {topItems.length > 0 && (
        <div>
          <div className="flex items-center justify-between mb-3">
            <h3 className="text-sm font-medium text-white">Top Picks</h3>
            <span className="text-xs text-gray-500">{topItems.length} items</span>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            {topItems.map(item => (
              <BriefingCard
                key={item.id}
                item={item}
                explanation={item.explanation}
                feedbackGiven={feedbackGiven[item.id]}
                onSave={(it) => recordInteraction(it.id, 'save', it)}
                onDismiss={(it) => recordInteraction(it.id, 'dismiss', it)}
              />
            ))}
          </div>
        </div>
      )}

      {/* View all results link */}
      <div className="flex justify-center pt-2 pb-4">
        <button
          onClick={() => setActiveView('results')}
          className="px-6 py-2.5 text-sm text-orange-400 bg-[#141414] border border-orange-500/20 rounded-lg hover:bg-orange-500/10 hover:border-orange-500/30 transition-all font-medium"
        >
          View All {results.length} Results
        </button>
      </div>

      {/* Error display */}
      {briefing.error && (
        <div className="p-3 bg-red-900/20 border border-red-500/30 rounded-lg text-red-300 text-sm flex items-center gap-2">
          <span>!</span>
          {briefing.error}
        </div>
      )}
    </div>
  );
}
