import { useState, useEffect, useRef, useCallback } from 'react';

type SubTab = 'base64' | 'url' | 'jwt' | 'hash';
type Direction = 'encode' | 'decode';
type HashAlgo = 'SHA-256' | 'SHA-512';

const SUB_TABS: { id: SubTab; label: string }[] = [
  { id: 'base64', label: 'Base64' },
  { id: 'url', label: 'URL Encode' },
  { id: 'jwt', label: 'JWT' },
  { id: 'hash', label: 'Hash' },
];

function isBase64(str: string): boolean {
  if (str.length === 0) return false;
  try {
    return btoa(atob(str)) === str;
  } catch {
    return false;
  }
}

function base64Encode(input: string): string {
  try {
    const bytes = new TextEncoder().encode(input);
    const binary = Array.from(bytes, (b) => String.fromCharCode(b)).join('');
    return btoa(binary);
  } catch {
    return '[Error: could not encode]';
  }
}

function base64Decode(input: string): string {
  try {
    const binary = atob(input);
    const bytes = Uint8Array.from(binary, (c) => c.charCodeAt(0));
    return new TextDecoder().decode(bytes);
  } catch {
    return '[Error: invalid Base64]';
  }
}

function base64UrlDecode(segment: string): string {
  let base64 = segment.replace(/-/g, '+').replace(/_/g, '/');
  while (base64.length % 4 !== 0) base64 += '=';
  return atob(base64);
}

function formatExpiry(exp: number): { text: string; expired: boolean } {
  const now = Date.now() / 1000;
  const date = new Date(exp * 1000);
  const dateStr = date.toLocaleString();
  if (exp < now) {
    return { text: `Expired: ${dateStr}`, expired: true };
  }
  const diff = exp - now;
  const hours = Math.floor(diff / 3600);
  const minutes = Math.floor((diff % 3600) / 60);
  return {
    text: `Valid until ${dateStr} (${hours}h ${minutes}m remaining)`,
    expired: false,
  };
}

async function computeHash(input: string, algo: HashAlgo): Promise<string> {
  const data = new TextEncoder().encode(input);
  const buffer = await crypto.subtle.digest(algo, data);
  return Array.from(new Uint8Array(buffer))
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(() => {
    if (!text) return;
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  }, [text]);

  return (
    <button
      onClick={handleCopy}
      disabled={!text}
      className="px-3 py-1.5 text-xs font-medium rounded-md border border-border
        bg-bg-tertiary text-text-secondary hover:text-white hover:border-white/20
        transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
    >
      {copied ? 'Copied' : 'Copy'}
    </button>
  );
}

function DirectionToggle({
  direction,
  onChange,
}: {
  direction: Direction;
  onChange: (d: Direction) => void;
}) {
  return (
    <div className="flex items-center gap-1 bg-bg-primary rounded-md p-0.5 border border-border">
      {(['encode', 'decode'] as const).map((d) => (
        <button
          key={d}
          onClick={() => onChange(d)}
          className={`px-3 py-1 text-xs font-medium rounded transition-colors capitalize ${
            direction === d
              ? 'bg-bg-tertiary text-white'
              : 'text-text-muted hover:text-text-secondary'
          }`}
        >
          {d}
        </button>
      ))}
    </div>
  );
}

function Base64Tab() {
  const [input, setInput] = useState('');
  const [direction, setDirection] = useState<Direction>('encode');
  const [output, setOutput] = useState('');
  const [autoDetected, setAutoDetected] = useState(false);

  useEffect(() => {
    if (!input) {
      setOutput('');
      setAutoDetected(false);
      return;
    }
    const detected = isBase64(input);
    setAutoDetected(detected);
    const dir = detected && direction === 'encode' ? 'decode' : direction;
    setOutput(dir === 'encode' ? base64Encode(input) : base64Decode(input));
  }, [input, direction]);

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <DirectionToggle direction={direction} onChange={setDirection} />
        {autoDetected && (
          <span className="text-xs text-text-muted">Base64 detected</span>
        )}
      </div>
      <textarea
        value={input}
        onChange={(e) => setInput(e.target.value)}
        placeholder="Enter text to encode or Base64 to decode..."
        className="w-full h-32 bg-bg-primary border border-border rounded-lg p-3 text-sm
          font-mono text-text-primary placeholder:text-text-muted resize-none
          focus:outline-none focus:border-white/20"
      />
      <div className="flex items-center justify-between">
        <span className="text-xs text-text-muted">Output</span>
        <CopyButton text={output} />
      </div>
      <textarea
        readOnly
        value={output}
        className="w-full h-32 bg-bg-primary border border-border rounded-lg p-3 text-sm
          font-mono text-text-secondary resize-none focus:outline-none"
      />
    </div>
  );
}

function UrlTab() {
  const [input, setInput] = useState('');
  const [direction, setDirection] = useState<Direction>('encode');
  const [output, setOutput] = useState('');

  useEffect(() => {
    if (!input) { setOutput(''); return; }
    try {
      setOutput(
        direction === 'encode'
          ? encodeURIComponent(input)
          : decodeURIComponent(input),
      );
    } catch {
      setOutput('[Error: invalid input]');
    }
  }, [input, direction]);

  return (
    <div className="space-y-3">
      <DirectionToggle direction={direction} onChange={setDirection} />
      <textarea
        value={input}
        onChange={(e) => setInput(e.target.value)}
        placeholder="Enter text to URL encode or encoded string to decode..."
        className="w-full h-32 bg-bg-primary border border-border rounded-lg p-3 text-sm
          font-mono text-text-primary placeholder:text-text-muted resize-none
          focus:outline-none focus:border-white/20"
      />
      <div className="flex items-center justify-between">
        <span className="text-xs text-text-muted">Output</span>
        <CopyButton text={output} />
      </div>
      <textarea
        readOnly
        value={output}
        className="w-full h-32 bg-bg-primary border border-border rounded-lg p-3 text-sm
          font-mono text-text-secondary resize-none focus:outline-none"
      />
    </div>
  );
}

function JwtTab() {
  const [input, setInput] = useState('');
  const [header, setHeader] = useState('');
  const [payload, setPayload] = useState('');
  const [expiry, setExpiry] = useState<{ text: string; expired: boolean } | null>(null);
  const [error, setError] = useState('');

  useEffect(() => {
    if (!input.trim()) {
      setHeader('');
      setPayload('');
      setExpiry(null);
      setError('');
      return;
    }
    const parts = input.trim().split('.');
    if (parts.length !== 3) {
      setError('JWT must have 3 segments separated by dots');
      setHeader('');
      setPayload('');
      setExpiry(null);
      return;
    }
    try {
      const h = JSON.parse(base64UrlDecode(parts[0]));
      const p = JSON.parse(base64UrlDecode(parts[1]));
      setHeader(JSON.stringify(h, null, 2));
      setPayload(JSON.stringify(p, null, 2));
      setExpiry(typeof p.exp === 'number' ? formatExpiry(p.exp) : null);
      setError('');
    } catch {
      setError('Failed to decode JWT');
      setHeader('');
      setPayload('');
      setExpiry(null);
    }
  }, [input]);

  return (
    <div className="space-y-3">
      <textarea
        value={input}
        onChange={(e) => setInput(e.target.value)}
        placeholder="Paste a JWT token..."
        className="w-full h-20 bg-bg-primary border border-border rounded-lg p-3 text-sm
          font-mono text-text-primary placeholder:text-text-muted resize-none
          focus:outline-none focus:border-white/20"
      />
      {error && <p className="text-xs text-error">{error}</p>}
      {expiry && (
        <p className={`text-xs font-medium ${expiry.expired ? 'text-error' : 'text-success'}`}>
          {expiry.text}
        </p>
      )}
      {header && (
        <div className="space-y-1">
          <div className="flex items-center justify-between">
            <span className="text-xs text-text-muted">Header</span>
            <CopyButton text={header} />
          </div>
          <pre className="bg-bg-primary border border-border rounded-lg p-3 text-xs
            font-mono text-text-secondary overflow-auto max-h-40">
            {header}
          </pre>
        </div>
      )}
      {payload && (
        <div className="space-y-1">
          <div className="flex items-center justify-between">
            <span className="text-xs text-text-muted">Payload</span>
            <CopyButton text={payload} />
          </div>
          <pre className="bg-bg-primary border border-border rounded-lg p-3 text-xs
            font-mono text-text-secondary overflow-auto max-h-48">
            {payload}
          </pre>
        </div>
      )}
    </div>
  );
}

function HashTab() {
  const [input, setInput] = useState('');
  const [algo, setAlgo] = useState<HashAlgo>('SHA-256');
  const [output, setOutput] = useState('');
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (timerRef.current) clearTimeout(timerRef.current);
    if (!input) { setOutput(''); return; }

    timerRef.current = setTimeout(() => {
      computeHash(input, algo).then(setOutput).catch(() => setOutput('[Error]'));
    }, 100);

    return () => { if (timerRef.current) clearTimeout(timerRef.current); };
  }, [input, algo]);

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2">
        <span className="text-xs text-text-muted">Algorithm</span>
        <div className="flex items-center gap-1 bg-bg-primary rounded-md p-0.5 border border-border">
          {(['SHA-256', 'SHA-512'] as const).map((a) => (
            <button
              key={a}
              onClick={() => setAlgo(a)}
              className={`px-3 py-1 text-xs font-medium rounded transition-colors ${
                algo === a
                  ? 'bg-bg-tertiary text-white'
                  : 'text-text-muted hover:text-text-secondary'
              }`}
            >
              {a}
            </button>
          ))}
        </div>
      </div>
      <textarea
        value={input}
        onChange={(e) => setInput(e.target.value)}
        placeholder="Enter text to hash..."
        className="w-full h-32 bg-bg-primary border border-border rounded-lg p-3 text-sm
          font-mono text-text-primary placeholder:text-text-muted resize-none
          focus:outline-none focus:border-white/20"
      />
      <div className="flex items-center justify-between">
        <span className="text-xs text-text-muted">
          {algo} Hash{output ? ` (${output.length} hex chars)` : ''}
        </span>
        <CopyButton text={output} />
      </div>
      <div className="bg-bg-primary border border-border rounded-lg p-3 min-h-[3rem]
        break-all text-sm font-mono text-text-secondary">
        {output || <span className="text-text-muted">Hash output will appear here</span>}
      </div>
    </div>
  );
}

export default function EncoderDecoder() {
  const [activeTab, setActiveTab] = useState<SubTab>('base64');

  return (
    <div className="space-y-4">
      <nav className="flex items-center gap-1 bg-bg-secondary rounded-lg p-1 border border-border w-fit">
        {SUB_TABS.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={`px-4 py-1.5 text-xs font-medium rounded-md transition-colors ${
              activeTab === tab.id
                ? 'bg-bg-tertiary text-white border border-border'
                : 'text-text-muted hover:text-text-secondary border border-transparent'
            }`}
          >
            {tab.label}
          </button>
        ))}
      </nav>
      <div className="bg-bg-secondary border border-border rounded-lg p-4">
        {activeTab === 'base64' && <Base64Tab />}
        {activeTab === 'url' && <UrlTab />}
        {activeTab === 'jwt' && <JwtTab />}
        {activeTab === 'hash' && <HashTab />}
      </div>
    </div>
  );
}
