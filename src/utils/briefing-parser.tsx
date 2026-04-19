// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type React from 'react';

export interface ParsedSection {
  title: string;
  lines: string[];
  type: 'action' | 'worth_knowing' | 'filtered' | 'general';
}

export function getRelativeTime(date: Date): string {
  const diffMs = Date.now() - date.getTime();
  const mins = Math.floor(diffMs / 60_000);
  if (mins < 1) return 'Just now';
  if (mins < 60) return `${mins} min ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return days === 1 ? 'Yesterday' : `${days}d ago`;
}

export function getFreshnessColor(date: Date): string {
  const hours = (Date.now() - date.getTime()) / 3_600_000;
  if (hours < 1) return 'text-green-400';
  if (hours < 4) return 'text-yellow-400';
  if (hours < 12) return 'text-orange-400';
  return 'text-red-400';
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

export function parseBriefingContent(content: string): ParsedSection[] {
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

export function SectionAccent({ type }: { type: ParsedSection['type'] }) {
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

export function sectionTitleColor(type: ParsedSection['type']): string {
  switch (type) {
    case 'action': return 'text-orange-400';
    case 'worth_knowing': return 'text-blue-400';
    case 'filtered': return 'text-gray-500';
    default: return 'text-white';
  }
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

export function renderLine(line: string, index: number, type: ParsedSection['type']) {
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
