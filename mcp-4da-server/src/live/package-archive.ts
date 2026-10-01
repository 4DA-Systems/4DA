// SPDX-License-Identifier: Apache-2.0
/**
 * Registry package archive reader (gzip tar), Node built-ins only.
 *
 * `upgrade_impact` reads the changelog a package ships INSIDE its own registry
 * archive (npm `.tgz`, crates.io `.crate`). That keeps the privacy contract
 * simple: the only host contacted is the package's own registry, which the
 * agent was going to download from anyway — never GitHub or a docs site.
 *
 * Why a hand-rolled tar reader: adding `tar` as a dependency for the one job
 * of pulling a few root-level text files would grow the published server's
 * supply chain for no gain. The subset handled here is what real registry
 * archives use: ustar headers with the `prefix` field, PAX `x` headers
 * (`path=` for names over 100 bytes — npm's packer emits these) and GNU `L`
 * long names (older cargo / GNU tar). Everything else is skipped, not guessed.
 *
 * Hard caps, because the archive is third-party input: compressed ≤ 20 MB
 * (Content-Length AND bytes actually read — a server can lie about the
 * former), uncompressed ≤ 80 MB (zlib `maxOutputLength`, so a gzip bomb stops
 * inflating at the cap instead of exhausting memory), single file ≤ 2 MB.
 */

import { gunzipSync } from "node:zlib";
import { fetchWithTimeout } from "./http-utils.js";

export const MAX_COMPRESSED_BYTES = 20 * 1024 * 1024;
export const MAX_UNCOMPRESSED_BYTES = 80 * 1024 * 1024;
export const MAX_FILE_BYTES = 2 * 1024 * 1024;

const BLOCK = 512;
const ARCHIVE_TIMEOUT_MS = 30_000;

/** The fetch signature archive and registry readers go through, so tests can inject one. */
export type FetchFn = (url: string, init?: RequestInit, timeoutMs?: number) => Promise<Response>;

export const defaultFetch: FetchFn = (url, init, timeoutMs) => fetchWithTimeout(url, init ?? {}, timeoutMs);

/** Raised for every refusal (cap exceeded, corrupt archive) with a message fit for tool output. */
export class ArchiveError extends Error {}

interface TarEntry {
  path: string;
  type: string;
  data: Buffer;
}

function readString(buf: Buffer, start: number, length: number): string {
  const slice = buf.subarray(start, start + length);
  const nul = slice.indexOf(0);
  return slice.subarray(0, nul === -1 ? slice.length : nul).toString("utf8");
}

function readOctal(buf: Buffer, start: number, length: number): number {
  if (buf[start] & 0x80) {
    // GNU base-256 size: only needed for entries ≥ 8 GB, far over every cap here.
    throw new ArchiveError("tar entry uses a base-256 size field (entry too large)");
  }
  const text = readString(buf, start, length).trim();
  if (text === "") return 0;
  if (!/^[0-7]+$/.test(text)) throw new ArchiveError(`corrupt tar header: size field "${text}"`);
  return parseInt(text, 8);
}

function checksumMatches(header: Buffer): boolean {
  const stored = readString(header, 148, 8).trim();
  if (!/^[0-7]+$/.test(stored)) return false;
  let sum = 0;
  for (let i = 0; i < BLOCK; i++) sum += i >= 148 && i < 156 ? 0x20 : header[i];
  return sum === parseInt(stored, 8);
}

/** PAX records are `"<len> <key>=<value>\n"`, where len counts the whole record in BYTES. */
export function parsePaxHeaders(data: Buffer): Record<string, string> {
  const out: Record<string, string> = {};
  let pos = 0;
  while (pos < data.length) {
    const space = data.indexOf(0x20, pos);
    if (space === -1) break;
    const len = Number(data.subarray(pos, space).toString("ascii"));
    if (!Number.isInteger(len) || len <= 0 || pos + len > data.length) break;
    const record = data.subarray(space + 1, pos + len).toString("utf8").replace(/\n$/, "");
    const eq = record.indexOf("=");
    if (eq > 0) out[record.slice(0, eq)] = record.slice(eq + 1);
    pos += len;
  }
  return out;
}

/** Walk an uncompressed tar buffer, yielding regular-file entries with their resolved names. */
export function* iterateTar(tar: Buffer): Generator<TarEntry> {
  let offset = 0;
  let paxPath: string | null = null;
  let gnuLongName: string | null = null;

  while (offset + BLOCK <= tar.length) {
    const header = tar.subarray(offset, offset + BLOCK);
    if (header.every((b) => b === 0)) return; // end-of-archive marker
    if (!checksumMatches(header)) throw new ArchiveError("corrupt tar header: checksum mismatch");

    const size = readOctal(header, 124, 12);
    const type = String.fromCharCode(header[156] || 0x30); // NUL typeflag = regular file
    const dataStart = offset + BLOCK;
    if (dataStart + size > tar.length) throw new ArchiveError("truncated tar entry");
    const data = tar.subarray(dataStart, dataStart + size);
    offset = dataStart + Math.ceil(size / BLOCK) * BLOCK;

    if (type === "x") {
      paxPath = parsePaxHeaders(data).path ?? null;
      continue;
    }
    if (type === "L") {
      gnuLongName = readString(data, 0, data.length);
      continue;
    }
    if (type === "g" || type === "K") continue; // global PAX / GNU long link: no effect on names here

    const name = readString(header, 0, 100);
    const magic = readString(header, 257, 6);
    const prefix = magic.startsWith("ustar") ? readString(header, 345, 155) : "";
    const path = paxPath ?? gnuLongName ?? (prefix ? `${prefix}/${name}` : name);
    paxPath = null;
    gnuLongName = null;
    yield { path, type, data };
  }
}

/** Path segments with `./` and empty segments removed. */
function segments(path: string): string[] {
  return path.split("/").filter((s) => s !== "" && s !== ".");
}

/**
 * Root-level text files from a gzip tar whose basename `accept` approves.
 * "Root-level" = at most one directory deep, because registry archives wrap
 * everything in a single top directory (`package/`, `<name>-<version>/`).
 * Keys are the archive paths as stored. Oversized or binary (NUL-containing)
 * files are skipped.
 */
export function readRootTextFiles(gz: Buffer, accept: (basename: string) => boolean): Map<string, string> {
  if (gz.length > MAX_COMPRESSED_BYTES) {
    throw new ArchiveError(`archive is ${gz.length} bytes compressed (cap ${MAX_COMPRESSED_BYTES})`);
  }
  let tar: Buffer;
  try {
    tar = gunzipSync(gz, { maxOutputLength: MAX_UNCOMPRESSED_BYTES });
  } catch (err) {
    if (err instanceof RangeError || (err as NodeJS.ErrnoException).code === "ERR_BUFFER_TOO_LARGE") {
      throw new ArchiveError(`archive exceeds ${MAX_UNCOMPRESSED_BYTES} bytes uncompressed`);
    }
    throw new ArchiveError(`archive is not valid gzip: ${(err as Error).message}`);
  }

  const files = new Map<string, string>();
  for (const entry of iterateTar(tar)) {
    if (entry.type !== "0" && entry.type !== "7") continue;
    const parts = segments(entry.path);
    if (parts.length === 0 || parts.length > 2) continue;
    if (!accept(parts[parts.length - 1])) continue;
    if (entry.data.length > MAX_FILE_BYTES || entry.data.includes(0)) continue;
    files.set(entry.path, entry.data.toString("utf8"));
  }
  return files;
}

/** Read a response body, refusing at `cap` bytes instead of buffering whatever the server sends. */
async function readCapped(response: Response, cap: number): Promise<Buffer> {
  const declared = Number(response.headers.get("content-length"));
  if (Number.isFinite(declared) && declared > cap) {
    throw new ArchiveError(`archive is ${declared} bytes compressed (cap ${cap})`);
  }
  if (!response.body) return Buffer.from(await response.arrayBuffer());
  const reader = response.body.getReader();
  const chunks: Buffer[] = [];
  let total = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    total += value.byteLength;
    if (total > cap) {
      await reader.cancel().catch(() => undefined);
      throw new ArchiveError(`archive exceeds ${cap} bytes compressed`);
    }
    chunks.push(Buffer.from(value));
  }
  return Buffer.concat(chunks);
}

/** Download a registry archive (size-capped) and return the accepted root-level text files. */
export async function fetchArchiveRootFiles(
  url: string,
  accept: (basename: string) => boolean,
  fetchFn: FetchFn = defaultFetch,
  headers: Record<string, string> = {},
): Promise<Map<string, string>> {
  const response = await fetchFn(url, { headers }, ARCHIVE_TIMEOUT_MS);
  if (!response.ok) throw new ArchiveError(`archive download failed: HTTP ${response.status}`);
  const gz = await readCapped(response, MAX_COMPRESSED_BYTES);
  return readRootTextFiles(gz, accept);
}
