import { useCallback, useMemo, useState, useEffect } from 'react';
import { BriefingCard } from './BriefingCard';
import { SignalActionCard } from './briefing/SignalActionCard';
import { ProGate } from './ProGate';
import { useAppStore } from '../store';

function getRelativeTime(date: Date): string {
  const diffMs = Date.now() - date.getTime();
  const mins = Math.floor(diffMs / 60_000);
  if (mins < 1) return 'Just now';
  if (mins < 60) return `${mins} min ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return days === 1 ? 'Yesterday' : `${days}d ago`;
}

function getFreshnessColor(date: Date): string {
  const hours = (Date.now() - date.getTime()) / 3_600_000;
  if (hours < 1) return 'text-green-400';
  if (hours < 4) return 'text-yellow-400';
  if (hours < 12) return 'text-orange-400';
  return 'text-red-400';
}

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
      return <div className="w-1 h-full bg-border rounded-full flex-shrink-0" />;
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
          <code key={`${i}-${j}`} className="px-1 py-0.5 bg-border text-orange-400 rounded text-xs font-mono">
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
  const lastBackgroundResultsAt = useAppStore(s => s.lastBackgroundResultsAt);
  const sourceHealth = useAppStore(s => s.sourceHealth);
  const addToast = useAppStore(s => s.addToast);

  const [gapExpanded, setGapExpanded] = useState(false);

  // Intelligence gaps — non-healthy sources
  const gaps = useMemo(
    () => sourceHealth.filter(s => s.status !== 'healthy' && s.gap_message),
    [sourceHealth],
  );

  // Copy raw briefing markdown
  const copyBriefing = useCallback(async () => {
    if (!briefing.content) return;
    await window.navigator.clipboard.writeText(briefing.content);
    addToast('success', 'Briefing copied to clipboard');
  }, [briefing.content, addToast]);

  // Share condensed briefing
  const shareBriefing = useCallback(async () => {
    if (!briefing.content) return;
    const sections = parseBriefingContent(briefing.content);
    const kept = sections.filter(s => s.type === 'action' || s.type === 'worth_knowing');
    const date = new Date().toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
    const lines = [`4DA Intelligence Briefing — ${date}\n`];
    for (const s of kept) {
      lines.push(`## ${s.title}`);
      lines.push(s.lines.join('\n'));
      lines.push('');
    }
    lines.push('Generated by 4DA (4da.dev)');
    await window.navigator.clipboard.writeText(lines.join('\n'));
    addToast('success', 'Condensed briefing copied to clipboard');
  }, [briefing.content, addToast]);

  // Auto-updating relative time (tick every 60s)
  const [, setTick] = useState(0);
  useEffect(() => {
    const interval = setInterval(() => setTick(t => t + 1), 60_000);
    return () => clearInterval(interval);
  }, []);

  // Detect stale briefing with new items available
  const isStale = useMemo(() => {
    if (!briefing.lastGenerated || !lastBackgroundResultsAt) return false;
    return lastBackgroundResultsAt.getTime() > briefing.lastGenerated.getTime();
  }, [briefing.lastGenerated, lastBackgroundResultsAt]);

  const sections = useMemo(() => {
    if (!briefing.content) return [];
    return parseBriefingContent(briefing.content);
  }, [briefing.content]);

  // Critical/high signal items for action cards
  const signalItems = useMemo(() => {
    return results
      .filter(r => r.signal_priority === 'critical' || r.signal_priority === 'high')
      .slice(0, 3);
  }, [results]);

  // Top picks from results for the briefing cards (exclude signal items to avoid duplicates)
  const topItems = useMemo(() => {
    const signalIds = new Set(signalItems.map(s => s.id));
    return results
      .filter(r => r.relevant && r.top_score >= 0.5 && !signalIds.has(r.id))
      .slice(0, 8);
  }, [results, signalItems]);

  // Loading skeleton
  if (briefing.loading) {
    return (
      <div className="bg-bg-primary rounded-lg">
        <div className="space-y-4">
          {/* Skeleton header */}
          <div className="bg-bg-secondary rounded-lg border border-border p-6">
            <div className="flex items-center gap-3 mb-4">
              <div className="w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center">
                <div className="w-4 h-4 border-2 border-orange-400 border-t-transparent rounded-full animate-spin" />
              </div>
              <div>
                <div className="h-5 w-48 bg-bg-tertiary rounded animate-pulse" />
                <div className="h-3 w-32 bg-bg-tertiary rounded animate-pulse mt-2" />
              </div>
            </div>
            {/* Skeleton lines */}
            <div className="space-y-3">
              {Array.from({ length: 6 }).map((_, i) => (
                <div key={i} className="h-4 bg-bg-tertiary rounded animate-pulse" style={{ width: `${75 + Math.random() * 25}%` }} />
              ))}
            </div>
          </div>
          {/* Skeleton cards */}
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            {Array.from({ length: 4 }).map((_, i) => (
              <div key={i} className="bg-bg-secondary rounded-lg border border-border p-4">
                <div className="flex gap-3">
                  <div className="w-10 h-6 bg-bg-tertiary rounded animate-pulse" />
                  <div className="flex-1 space-y-2">
                    <div className="h-4 bg-bg-tertiary rounded animate-pulse" />
                    <div className="h-3 bg-bg-tertiary rounded animate-pulse w-3/4" />
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
      <div className="bg-bg-primary rounded-lg">
        <div className="flex flex-col items-center justify-center py-20 px-8">
          <div className="w-20 h-20 mb-6 bg-bg-secondary rounded-2xl border border-border flex items-center justify-center">
            <div className="w-6 h-6 border-2 border-orange-400 border-t-transparent rounded-full animate-spin" />
          </div>
          <h2 className="text-xl font-medium text-white mb-2">Preparing Your Briefing</h2>
          <p className="text-sm text-gray-500 text-center max-w-md">
            {results.length === 0
              ? '4DA is gathering intelligence from your sources...'
              : `Analyzing ${results.length} results to surface what matters most...`}
          </p>
          {results.length > 0 && (
            <button
              onClick={() => setActiveView('results')}
              className="mt-6 text-sm text-gray-500 hover:text-gray-300 transition-colors"
            >
              Browse {results.length} results while you wait
            </button>
          )}
        </div>
      </div>
    );
  }

  // Briefing content view
  return (
    <ProGate feature="AI Briefings">
    <div className="bg-bg-primary rounded-lg space-y-6">
      {/* Signal Action Cards — critical/high priority items */}
      {signalItems.length > 0 && (
        <div className="space-y-3">
          {signalItems.map(item => (
            <SignalActionCard
              key={item.id}
              item={item}
              feedbackGiven={feedbackGiven[item.id]}
              onSave={(it) => recordInteraction(it.id, 'save', it)}
              onDismiss={(it) => recordInteraction(it.id, 'dismiss', it)}
            />
          ))}
        </div>
      )}

      {/* Briefing header */}
      <div className="bg-bg-secondary rounded-lg border border-orange-500/20 overflow-hidden">
        <div className="px-5 py-4 border-b border-orange-500/10 flex items-center justify-between bg-orange-500/5">
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 bg-orange-500/20 rounded-lg flex items-center justify-center">
              <span className="text-orange-400 text-sm">*</span>
            </div>
            <div>
              <h2 className="font-medium text-orange-400">Intelligence Briefing</h2>
            </div>
          </div>
          <div className="flex items-center gap-2">
            {briefing.lastGenerated && (
              <span className={`text-xs font-medium ${getFreshnessColor(briefing.lastGenerated)}`}>
                {getRelativeTime(briefing.lastGenerated)}
              </span>
            )}
            <button
              onClick={copyBriefing}
              className="px-2.5 py-1.5 text-xs bg-bg-tertiary text-gray-400 border border-border rounded-lg hover:text-white hover:border-[#3A3A3A] transition-all"
              title="Copy briefing markdown"
            >
              Copy
            </button>
            <button
              onClick={shareBriefing}
              className="px-2.5 py-1.5 text-xs bg-bg-tertiary text-gray-400 border border-border rounded-lg hover:text-white hover:border-[#3A3A3A] transition-all"
              title="Copy condensed briefing for sharing"
            >
              Share
            </button>
            <button
              onClick={generateBriefing}
              className="px-3 py-1.5 text-xs bg-bg-tertiary text-orange-400 border border-orange-500/30 rounded-lg hover:bg-orange-500/10 transition-all font-medium"
              title="Refresh briefing"
            >
              Refresh
            </button>
          </div>
        </div>

        {/* Stale briefing indicator */}
        {isStale && (
          <div className="px-5 py-2.5 bg-yellow-500/5 border-b border-yellow-500/10 flex items-center justify-between">
            <span className="text-xs text-yellow-400">New items found since this briefing.</span>
            <button
              onClick={generateBriefing}
              className="text-xs text-yellow-400 hover:text-yellow-300 underline font-medium"
            >
              Refresh
            </button>
          </div>
        )}

        {/* Intelligence gap banner */}
        {gaps.length > 0 && (
          <div className="px-5 py-2.5 bg-amber-500/5 border-b border-amber-500/10">
            <button
              onClick={() => setGapExpanded(!gapExpanded)}
              className="w-full flex items-center justify-between text-left"
            >
              <span className="text-xs text-amber-400">
                {gaps.length} source{gaps.length > 1 ? 's' : ''} offline: {gaps.map(g => g.gap_message).join(', ')}
              </span>
              <span className="text-xs text-amber-500 ml-2 flex-shrink-0">{gapExpanded ? '\u25B2' : '\u25BC'}</span>
            </button>
            {gapExpanded && (
              <div className="mt-2 space-y-1">
                {sourceHealth.map(s => (
                  <div key={s.source_type} className="flex items-center justify-between text-xs py-0.5">
                    <span className={s.status === 'healthy' ? 'text-green-400' : 'text-amber-400'}>
                      {s.source_type}
                    </span>
                    <span className="text-gray-500">
                      {s.status === 'healthy'
                        ? `${s.items_fetched} items${s.last_success_relative ? ` \u00B7 ${s.last_success_relative}` : ''}`
                        : s.status === 'circuit_open' ? 'circuit open' : 'error'}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

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
          <div className="px-5 py-3 border-t border-border text-xs text-gray-600">
            Generated {briefing.lastGenerated.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
            {briefing.model && <span className="ml-2">via {briefing.model}</span>}
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
            {topItems.map(item => {
              const hasWorkMatch = item.score_breakdown?.intent_boost && item.score_breakdown.intent_boost > 0;
              const hasDep = item.score_breakdown?.dep_match_score && item.score_breakdown.dep_match_score > 0;
              const matchedDeps = item.score_breakdown?.matched_deps;
              return (
                <div key={item.id} className="relative">
                  {(hasWorkMatch || hasDep) && (
                    <div className="flex items-center gap-1.5 mb-1.5">
                      {hasWorkMatch && (
                        <span className="text-[10px] px-1.5 py-0.5 bg-purple-500/10 text-purple-400 border border-purple-500/20 rounded font-medium">
                          Working on
                        </span>
                      )}
                      {hasDep && (
                        <span className="text-[10px] px-1.5 py-0.5 bg-blue-500/10 text-blue-400 border border-blue-500/20 rounded font-medium">
                          Stack{matchedDeps ? `: ${matchedDeps.slice(0, 3).join(', ')}` : ''}
                        </span>
                      )}
                    </div>
                  )}
                  <BriefingCard
                    item={item}
                    explanation={item.explanation}
                    feedbackGiven={feedbackGiven[item.id]}
                    onSave={(it) => recordInteraction(it.id, 'save', it)}
                    onDismiss={(it) => recordInteraction(it.id, 'dismiss', it)}
                  />
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* View all results link */}
      <div className="flex justify-center pt-2 pb-4">
        <button
          onClick={() => setActiveView('results')}
          className="px-6 py-2.5 text-sm text-orange-400 bg-bg-secondary border border-orange-500/20 rounded-lg hover:bg-orange-500/10 hover:border-orange-500/30 transition-all font-medium"
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
    </ProGate>
  );
}
