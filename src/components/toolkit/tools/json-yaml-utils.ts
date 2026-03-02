export type Mode = 'editor' | 'tree' | 'diff';
export type JsonValue = string | number | boolean | null | JsonValue[] | { [key: string]: JsonValue };

export interface ParseResult {
  data: JsonValue | undefined;
  error: string | null;
}

export function parseJson(raw: string): ParseResult {
  if (!raw.trim()) return { data: undefined, error: null };
  try {
    return { data: JSON.parse(raw), error: null };
  } catch (e: unknown) {
    const msg = e instanceof SyntaxError ? e.message : 'Invalid JSON';
    const posMatch = msg.match(/position\s+(\d+)/i);
    if (posMatch) {
      const pos = parseInt(posMatch[1], 10);
      let line = 1;
      let col = 1;
      for (let i = 0; i < pos && i < raw.length; i++) {
        if (raw[i] === '\n') { line++; col = 1; } else { col++; }
      }
      return { data: undefined, error: `${msg} (line ${line}, column ${col})` };
    }
    return { data: undefined, error: msg };
  }
}

export function computeDiff(a: string, b: string): { type: 'same' | 'added' | 'removed'; text: string }[] {
  const linesA = a.split('\n');
  const linesB = b.split('\n');
  const result: { type: 'same' | 'added' | 'removed'; text: string }[] = [];
  const max = Math.max(linesA.length, linesB.length);
  for (let i = 0; i < max; i++) {
    const la = i < linesA.length ? linesA[i] : undefined;
    const lb = i < linesB.length ? linesB[i] : undefined;
    if (la === lb) {
      result.push({ type: 'same', text: la! });
    } else {
      if (la !== undefined) result.push({ type: 'removed', text: la });
      if (lb !== undefined) result.push({ type: 'added', text: lb });
    }
  }
  return result;
}

export function copyToClipboard(text: string) {
  navigator.clipboard.writeText(text).catch(() => {});
}
