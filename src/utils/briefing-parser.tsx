// SPDX-License-Identifier: FSL-1.1-Apache-2.0

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

