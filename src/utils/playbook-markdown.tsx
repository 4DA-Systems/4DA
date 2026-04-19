// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import type { ReactNode } from 'react';
import { StreetsCodeBlock } from '../components/playbook/StreetsCodeBlock';

/**
 * Lightweight inline markdown renderer for Playbook content.
 * No external dependencies. Handles: headings, bold, code, code blocks,
 * lists, blockquotes, tables, links, paragraph breaks, and L3-L5 injection
 * markers ({@ type block_id ... @}) from the Sovereign Content Engine.
 *
 * When config.moduleId and config.lessonIdx are provided, bash/shell/powershell
 * code blocks render as interactive StreetsCodeBlock components with run buttons.
 */

/** Regex for injection markers: {@ type block_id [params] @} */
const INJECTION_MARKER_RE = /^\{@\s+(\w+)\s+([\w.-]+)(?:\s+(.*?))?\s*@\}$/;

function processInline(text: string): ReactNode[] {
  const parts: ReactNode[] = [];
  let remaining = text;
  let key = 0;

  while (remaining.length > 0) {
    // Links [text](url)
    const linkMatch = remaining.match(/^(.*?)\[([^\]]+)\]\(([^)]+)\)(.*)/s);
    if (linkMatch) {
      if (linkMatch[1]) parts.push(...processInlineSimple(linkMatch[1]!, key));
      key++;
      parts.push(
        <a key={`link-${key}`} href={linkMatch[3]!} target="_blank" rel="noopener noreferrer"
          className="text-[#D4AF37] hover:underline">
          {linkMatch[2]}
        </a>,
      );
      remaining = linkMatch[4] ?? '';
      continue;
    }

    // No more complex patterns, process simple inline
    parts.push(...processInlineSimple(remaining, key));
    break;
  }

  return parts;
}

function processInlineSimple(text: string, baseKey: number): ReactNode[] {
  const parts: ReactNode[] = [];
  // Split by bold **text** and code `text`
  const regex = /(\*\*(.+?)\*\*|`([^`]+)`)/g;
  let lastIdx = 0;
  let match;
  let key = baseKey;

  while ((match = regex.exec(text)) !== null) {
    if (match.index > lastIdx) {
      parts.push(text.slice(lastIdx, match.index));
    }
    if (match[2]) {
      // Bold
      parts.push(<strong key={`b-${key++}`} className="text-white font-semibold">{match[2]}</strong>);
    } else if (match[3]) {
      // Inline code
      parts.push(
        <code key={`c-${key++}`} className="px-1.5 py-0.5 bg-[#1F1F1F] text-[#D4AF37] rounded text-xs font-mono">
          {match[3]}
        </code>,
      );
    }
    lastIdx = match.index + match[0].length;
  }

  if (lastIdx < text.length) {
    parts.push(text.slice(lastIdx));
  }

  return parts;
}

export function renderMarkdown(content: string, config?: { moduleId?: string; lessonIdx?: number }): ReactNode {
  const lines = content.split('\n');
  const elements: ReactNode[] = [];
  let i = 0;
  let key = 0;

  while (i < lines.length) {
    const line = lines[i]!;

    // Code block
    if (line.startsWith('```')) {
      const lang = line.slice(3).trim();
      const codeLines: string[] = [];
      i++;
      while (i < lines.length && !lines[i]!.startsWith('```')) {
        codeLines.push(lines[i]!);
        i++;
      }
      i++; // skip closing ```

      // Interactive code block for executable languages when config is provided
      if (config?.moduleId && config?.lessonIdx !== undefined &&
          ['bash', 'shell', 'powershell', 'sh'].includes(lang.toLowerCase())) {
        elements.push(
          <StreetsCodeBlock
            key={`code-${key++}`}
            code={codeLines.join('\n')}
            language={lang}
            moduleId={config.moduleId}
            lessonIdx={config.lessonIdx}
            blockIndex={key}
          />,
        );
        continue;
      }

      // Default: static code block
      elements.push(
        <pre key={`pre-${key++}`} className="bg-[#0A0A0A] border border-[#2A2A2A] rounded-lg p-4 overflow-x-auto my-3">
          <code className="text-xs font-mono text-[#A0A0A0] leading-relaxed">
            {lang && <span className="text-[#666] text-[10px] block mb-2">{lang}</span>}
            {codeLines.join('\n')}
          </code>
        </pre>,
      );
      continue;
    }

    // Headings
    if (line.startsWith('### ')) {
      elements.push(
        <h4 key={`h3-${key++}`} className="text-sm font-semibold text-white mt-4 mb-2">
          {processInline(line.slice(4))}
        </h4>,
      );
      i++;
      continue;
    }
    if (line.startsWith('## ')) {
      elements.push(
        <h3 key={`h2-${key++}`} className="text-base font-semibold text-white mt-5 mb-2">
          {processInline(line.slice(3))}
        </h3>,
      );
      i++;
      continue;
    }
    if (line.startsWith('# ')) {
      elements.push(
        <h2 key={`h1-${key++}`} className="text-lg font-bold text-white mt-6 mb-3">
          {processInline(line.slice(2))}
        </h2>,
      );
      i++;
      continue;
    }

    // Blockquote
    if (line.startsWith('> ')) {
      const quoteLines: string[] = [];
      while (i < lines.length && lines[i]!.startsWith('> ')) {
        quoteLines.push(lines[i]!.slice(2));
        i++;
      }
      elements.push(
        <blockquote key={`bq-${key++}`} className="border-l-2 border-[#D4AF37] pl-4 my-3 text-[#A0A0A0] italic">
          {quoteLines.map((ql, qi) => (
            <p key={qi}>{processInline(ql)}</p>
          ))}
        </blockquote>,
      );
      continue;
    }

    // Table (pipe-delimited)
    if (line.includes('|') && line.trim().startsWith('|')) {
      const tableLines: string[] = [];
      while (i < lines.length && lines[i]!.includes('|') && lines[i]!.trim().startsWith('|')) {
        tableLines.push(lines[i]!);
        i++;
      }
      // Parse header + separator + rows
      const rows = tableLines
        .filter((tl) => !tl.match(/^\|\s*[-:]+/)) // skip separator
        .map((tl) =>
          tl.split('|').filter((_, ci) => ci > 0 && ci < tl.split('|').length - 1).map((c) => c.trim()),
        );

      if (rows.length > 0) {
        const header = rows[0]!;
        const body = rows.slice(1);
        elements.push(
          <div key={`tbl-${key++}`} className="overflow-x-auto my-3">
            <table className="w-full text-xs border border-[#2A2A2A] rounded">
              <thead>
                <tr className="bg-[#1F1F1F]">
                  {header.map((h, hi) => (
                    <th key={hi} className="px-3 py-2 text-left text-white font-medium border-b border-[#2A2A2A]">
                      {processInline(h)}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {body.map((row, ri) => (
                  <tr key={ri} className="border-b border-[#2A2A2A] last:border-0">
                    {row.map((cell, ci) => (
                      <td key={ci} className="px-3 py-2 text-[#A0A0A0]">
                        {processInline(cell)}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>,
        );
      }
      continue;
    }

    // Unordered list
    if (line.match(/^\s*[-*]\s/)) {
      const listItems: string[] = [];
      while (i < lines.length && lines[i]!.match(/^\s*[-*]\s/)) {
        listItems.push(lines[i]!.replace(/^\s*[-*]\s/, ''));
        i++;
      }
      elements.push(
        <ul key={`ul-${key++}`} className="list-disc list-outside ml-5 my-2 space-y-1">
          {listItems.map((item, li) => (
            <li key={li} className="text-[#A0A0A0]">{processInline(item)}</li>
          ))}
        </ul>,
      );
      continue;
    }

    // Ordered list
    if (line.match(/^\s*\d+\.\s/)) {
      const listItems: string[] = [];
      while (i < lines.length && lines[i]!.match(/^\s*\d+\.\s/)) {
        listItems.push(lines[i]!.replace(/^\s*\d+\.\s/, ''));
        i++;
      }
      elements.push(
        <ol key={`ol-${key++}`} className="list-decimal list-outside ml-5 my-2 space-y-1">
          {listItems.map((item, li) => (
            <li key={li} className="text-[#A0A0A0]">{processInline(item)}</li>
          ))}
        </ol>,
      );
      continue;
    }

    // Injection marker: {@ type block_id [params] @}
    const injectionMatch = line.trim().match(INJECTION_MARKER_RE);
    if (injectionMatch) {
      const blockType = injectionMatch[1]!;
      const blockId = injectionMatch[2]!;
      elements.push(
        <div
          key={`inject-${key++}`}
          data-block-type={blockType}
          data-block-id={blockId}
          className="personalization-injection-point"
        />,
      );
      i++;
      continue;
    }

    // Empty line = paragraph break
    if (line.trim() === '') {
      i++;
      continue;
    }

    // Regular paragraph
    elements.push(
      <p key={`p-${key++}`} className="my-1.5 text-[#A0A0A0] leading-relaxed">
        {processInline(line)}
      </p>,
    );
    i++;
  }

  return <div>{elements}</div>;
}
