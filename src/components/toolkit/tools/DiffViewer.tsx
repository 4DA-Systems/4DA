import { useState, useMemo } from 'react';

function computeDiff(a: string, b: string): Array<{ type: 'equal' | 'added' | 'removed'; line: string }> {
  const linesA = a.split('\n');
  const linesB = b.split('\n');
  const result: Array<{ type: 'equal' | 'added' | 'removed'; line: string }> = [];

  // Simple LCS-based diff
  const m = linesA.length;
  const n = linesB.length;
  const dp: number[][] = Array.from({ length: m + 1 }, () => Array(n + 1).fill(0));
  for (let i = 1; i <= m; i++) {
    for (let j = 1; j <= n; j++) {
      dp[i][j] = linesA[i - 1] === linesB[j - 1]
        ? dp[i - 1][j - 1] + 1
        : Math.max(dp[i - 1][j], dp[i][j - 1]);
    }
  }

  // Backtrack
  const ops: Array<{ type: 'equal' | 'added' | 'removed'; line: string }> = [];
  let i = m, j = n;
  while (i > 0 || j > 0) {
    if (i > 0 && j > 0 && linesA[i - 1] === linesB[j - 1]) {
      ops.push({ type: 'equal', line: linesA[i - 1] });
      i--; j--;
    } else if (j > 0 && (i === 0 || dp[i][j - 1] >= dp[i - 1][j])) {
      ops.push({ type: 'added', line: linesB[j - 1] });
      j--;
    } else {
      ops.push({ type: 'removed', line: linesA[i - 1] });
      i--;
    }
  }
  ops.reverse();
  return ops.length ? ops : result;
}

export default function DiffViewer() {
  const [left, setLeft] = useState('');
  const [right, setRight] = useState('');
  const [mode, setMode] = useState<'unified' | 'side'>('unified');

  const diff = useMemo(() => computeDiff(left, right), [left, right]);

  const stats = useMemo(() => {
    let added = 0, removed = 0;
    for (const d of diff) {
      if (d.type === 'added') added++;
      if (d.type === 'removed') removed++;
    }
    return { added, removed };
  }, [diff]);

  return (
    <div className="space-y-4">
      {/* Input panes */}
      <div className="grid grid-cols-2 gap-3">
        <div>
          <label className="block text-xs text-gray-500 mb-1">Original</label>
          <textarea
            value={left}
            onChange={(e) => setLeft(e.target.value)}
            placeholder="Paste original text..."
            className="w-full h-48 px-3 py-2 text-sm font-mono bg-bg-secondary border border-border rounded-lg text-white placeholder:text-gray-600 focus:outline-none focus:border-white/30 resize-y"
          />
        </div>
        <div>
          <label className="block text-xs text-gray-500 mb-1">Modified</label>
          <textarea
            value={right}
            onChange={(e) => setRight(e.target.value)}
            placeholder="Paste modified text..."
            className="w-full h-48 px-3 py-2 text-sm font-mono bg-bg-secondary border border-border rounded-lg text-white placeholder:text-gray-600 focus:outline-none focus:border-white/30 resize-y"
          />
        </div>
      </div>

      {/* Controls */}
      <div className="flex items-center gap-3">
        <div className="flex bg-bg-secondary border border-border rounded-lg p-0.5">
          <button
            onClick={() => setMode('unified')}
            className={`px-3 py-1 text-xs rounded-md transition-all ${mode === 'unified' ? 'bg-bg-tertiary text-white' : 'text-gray-500'}`}
          >
            Unified
          </button>
          <button
            onClick={() => setMode('side')}
            className={`px-3 py-1 text-xs rounded-md transition-all ${mode === 'side' ? 'bg-bg-tertiary text-white' : 'text-gray-500'}`}
          >
            Side by Side
          </button>
        </div>
        {(left || right) && (
          <span className="text-xs text-gray-500">
            <span className="text-green-400">+{stats.added}</span>
            {' / '}
            <span className="text-red-400">-{stats.removed}</span>
          </span>
        )}
      </div>

      {/* Diff output */}
      {(left || right) && (
        <div className="bg-bg-secondary border border-border rounded-lg overflow-auto max-h-96">
          {mode === 'unified' ? (
            <div className="font-mono text-xs">
              {diff.map((d, i) => (
                <div
                  key={i}
                  className={`px-3 py-0.5 ${
                    d.type === 'added' ? 'bg-green-500/10 text-green-300' :
                    d.type === 'removed' ? 'bg-red-500/10 text-red-300' :
                    'text-gray-400'
                  }`}
                >
                  <span className="inline-block w-5 text-gray-600 select-none">
                    {d.type === 'added' ? '+' : d.type === 'removed' ? '-' : ' '}
                  </span>
                  {d.line || '\u00A0'}
                </div>
              ))}
            </div>
          ) : (
            <div className="grid grid-cols-2 font-mono text-xs">
              <div className="border-r border-border">
                {diff.filter(d => d.type !== 'added').map((d, i) => (
                  <div key={i} className={`px-3 py-0.5 ${d.type === 'removed' ? 'bg-red-500/10 text-red-300' : 'text-gray-400'}`}>
                    {d.line || '\u00A0'}
                  </div>
                ))}
              </div>
              <div>
                {diff.filter(d => d.type !== 'removed').map((d, i) => (
                  <div key={i} className={`px-3 py-0.5 ${d.type === 'added' ? 'bg-green-500/10 text-green-300' : 'text-gray-400'}`}>
                    {d.line || '\u00A0'}
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
