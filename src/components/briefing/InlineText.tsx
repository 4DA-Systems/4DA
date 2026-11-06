// SPDX-License-Identifier: FSL-1.1-Apache-2.0
import { parseInline } from '../../utils/briefing-parser';
import { openExternalUrl } from '../../lib/open-url';

interface InlineTextProps {
  line: string;
  /**
   * Render `[text](https://...)` as a clickable link (the Brief). When false
   * the label renders as plain text (card prose, where a click target inside
   * a sentence is not wanted).
   */
  allowLinks?: boolean;
}

/**
 * One line of inline Markdown as React nodes: `**bold**`, `` `code` `` and
 * http(s) links, via `parseInline`. Text only — every segment is a React text
 * child, so no HTML in the source can reach the DOM.
 */
export function InlineText({ line, allowLinks = true }: InlineTextProps) {
  return (
    <>
      {parseInline(line).map((seg, i) => {
        switch (seg.kind) {
          case 'bold':
            return <strong key={i} className="font-semibold text-text-primary">{seg.text}</strong>;
          case 'code':
            return <code key={i} className="font-mono text-[0.85em] px-1 rounded bg-bg-tertiary">{seg.text}</code>;
          case 'link':
            if (!allowLinks) return <span key={i}>{seg.text}</span>;
            return (
              <a
                key={i}
                href={seg.url}
                onClick={(e) => { e.preventDefault(); openExternalUrl(seg.url); }}
                className="text-text-primary underline decoration-border underline-offset-2 hover:decoration-text-secondary"
              >
                {seg.text}
              </a>
            );
          default:
            return <span key={i}>{seg.text}</span>;
        }
      })}
    </>
  );
}
