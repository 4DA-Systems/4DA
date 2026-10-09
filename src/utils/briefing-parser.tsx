// SPDX-License-Identifier: FSL-1.1-Apache-2.0

export interface ParsedSection {
  title: string;
  lines: string[];
  type: 'action' | 'upgrades' | 'worth_knowing' | 'still_open' | 'filtered' | 'general';
}

/** One run of inline brief text: plain, bold, code, or a link. */
export type InlineSegment =
  | { kind: 'text'; text: string }
  | { kind: 'bold'; text: string }
  | { kind: 'code'; text: string }
  | { kind: 'link'; text: string; url: string };

/**
 * Split one brief line into inline segments. Supports the subset the brief
 * prompt and the deterministic floor emit — `**bold**`, `` `code` `` and
 * `[text](url)` — and nothing else: no HTML ever reaches the DOM, and a link
 * is a link only when its URL is http(s) (anything else renders as text).
 *
 * Backslash escapes (`\[`, `\]`, `\*`, `\_`, `` \` ``, `\\`) read as the plain
 * character: the floor escapes titles it interpolates
 * (`briefing_deterministic::escape_markdown`), so LWN's "[$] ..." stays a
 * link label instead of breaking the link into raw Markdown.
 */
export function parseInline(line: string): InlineSegment[] {
  const out: InlineSegment[] = [];
  let text = '';
  const flush = () => {
    if (text) out.push({ kind: 'text', text });
    text = '';
  };
  const pattern = /\\([\\`*_[\]()#])|\*\*((?:\\.|[^*\\])+)\*\*|`([^`]+)`|\[((?:\\.|[^\]\\])+)\]\(([^)\s]+)\)/g;
  let last = 0;
  for (const m of line.matchAll(pattern)) {
    const start = m.index ?? 0;
    text += line.slice(last, start);
    last = start + m[0].length;
    if (m[1] !== undefined) {
      text += m[1];
    } else if (m[2] !== undefined) {
      flush();
      // `**[fastembed 7.1.0](url)**` — the floor bolds its upgrade links.
      const inner = /^\[((?:\\.|[^\]\\])+)\]\((https?:\/\/[^)\s]+)\)$/i.exec(m[2]);
      out.push(inner
        ? { kind: 'link', text: unescapeMarkdown(inner[1]!), url: inner[2]! }
        : { kind: 'bold', text: unescapeMarkdown(m[2]) });
    } else if (m[3] !== undefined) {
      flush();
      out.push({ kind: 'code', text: m[3] });
    } else if (m[4] !== undefined && m[5] !== undefined) {
      const url = m[5];
      // A bold link — [**fastembed 7.1.0**](url) — keeps its label text.
      const label = unescapeMarkdown(m[4].replace(/\*\*/g, ''));
      if (/^https?:\/\//i.test(url)) {
        flush();
        out.push({ kind: 'link', text: label, url });
      } else {
        text += label;
      }
    }
  }
  text += line.slice(last);
  flush();
  return out;
}

function unescapeMarkdown(s: string): string {
  return s.replace(/\\([\\`*_[\]()#])/g, '$1');
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
  // The facts-first brief's sections (Decision 2): Act now, Upgrades to plan,
  // Worth knowing, Still open.
  if (lower.includes('still open')) return 'still_open';
  if (lower.includes('upgrade')) return 'upgrades';
  if (lower.includes('act now')) return 'action';
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

