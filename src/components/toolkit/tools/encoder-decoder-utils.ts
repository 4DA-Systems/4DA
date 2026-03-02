export type SubTab = 'base64' | 'url' | 'jwt' | 'hash';
export type Direction = 'encode' | 'decode';
export type HashAlgo = 'SHA-256' | 'SHA-512';

export const SUB_TABS: { id: SubTab; label: string }[] = [
  { id: 'base64', label: 'Base64' },
  { id: 'url', label: 'URL Encode' },
  { id: 'jwt', label: 'JWT' },
  { id: 'hash', label: 'Hash' },
];

export function isBase64(str: string): boolean {
  if (str.length === 0) return false;
  try {
    return btoa(atob(str)) === str;
  } catch {
    return false;
  }
}

export function base64Encode(input: string): string {
  try {
    const bytes = new TextEncoder().encode(input);
    const binary = Array.from(bytes, (b) => String.fromCharCode(b)).join('');
    return btoa(binary);
  } catch {
    return '[Error: could not encode]';
  }
}

export function base64Decode(input: string): string {
  try {
    const binary = atob(input);
    const bytes = Uint8Array.from(binary, (c) => c.charCodeAt(0));
    return new TextDecoder().decode(bytes);
  } catch {
    return '[Error: invalid Base64]';
  }
}

export function base64UrlDecode(segment: string): string {
  let base64 = segment.replace(/-/g, '+').replace(/_/g, '/');
  while (base64.length % 4 !== 0) base64 += '=';
  return atob(base64);
}

export function formatExpiry(exp: number): { text: string; expired: boolean } {
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

export async function computeHash(input: string, algo: HashAlgo): Promise<string> {
  const data = new TextEncoder().encode(input);
  const buffer = await crypto.subtle.digest(algo, data);
  return Array.from(new Uint8Array(buffer))
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}
