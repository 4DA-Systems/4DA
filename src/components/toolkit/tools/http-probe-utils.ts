// --- Types ---

export interface HttpProbeRequest {
  method: string;
  url: string;
  headers: [string, string][];
  body: string | null;
}

export interface HttpProbeResponse {
  status: number;
  status_text: string;
  headers: [string, string][];
  body: string;
  duration_ms: number;
  size_bytes: number;
}

export interface HttpHistoryEntry {
  id: number;
  method: string;
  url: string;
  status: number;
  duration_ms: number;
  created_at: string;
}

// --- Constants ---

export const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'] as const;
export const BODY_METHODS = new Set(['POST', 'PUT', 'PATCH']);

// --- Helpers ---

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1048576).toFixed(1)} MB`;
}

export function statusColor(status: number): string {
  if (status >= 200 && status < 300) return 'text-[#22C55E]';
  if (status >= 300 && status < 400) return 'text-blue-400';
  if (status >= 400 && status < 500) return 'text-orange-400';
  if (status >= 500) return 'text-[#EF4444]';
  return 'text-text-secondary';
}

export function statusBgColor(status: number): string {
  if (status >= 200 && status < 300) return 'bg-[#22C55E]/10';
  if (status >= 300 && status < 400) return 'bg-blue-400/10';
  if (status >= 400 && status < 500) return 'bg-orange-400/10';
  if (status >= 500) return 'bg-[#EF4444]/10';
  return 'bg-bg-tertiary';
}

export function methodColor(method: string): string {
  switch (method) {
    case 'GET': return 'bg-[#22C55E]/15 text-[#22C55E]';
    case 'POST': return 'bg-blue-400/15 text-blue-400';
    case 'PUT': return 'bg-orange-400/15 text-orange-400';
    case 'PATCH': return 'bg-orange-400/15 text-orange-400';
    case 'DELETE': return 'bg-[#EF4444]/15 text-[#EF4444]';
    default: return 'bg-bg-tertiary text-text-secondary';
  }
}
