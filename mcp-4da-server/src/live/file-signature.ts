// SPDX-License-Identifier: Apache-2.0
/**
 * A cheap identity for a file at one moment, so the live layer can notice a
 * change with stat calls alone: "absent", or mtime + size + inode.
 *
 * The inode matters for node_modules. npm packs every tarball file with one
 * fixed mtime, and pnpm swaps a package version by re-pointing a link at a
 * different store file, so two versions can share an mtime. They cannot share
 * an inode.
 */

import * as fs from "node:fs";

export type FileSignature = string;

export const ABSENT: FileSignature = "absent";

export function fileSignature(filePath: string): FileSignature {
  try {
    const s = fs.statSync(filePath);
    return `${s.mtimeMs}:${s.size}:${s.ino}`;
  } catch {
    return ABSENT;
  }
}

/** The modification time a signature recorded, or null for an absent file. */
export function signatureMtime(signature: FileSignature | undefined): number | null {
  if (!signature || signature === ABSENT) return null;
  const mtime = Number(signature.split(":")[0]);
  return Number.isFinite(mtime) ? mtime : null;
}

/** Signatures for a set of paths, taken now. */
export function signatureMap(paths: Iterable<string>): Map<string, FileSignature> {
  const out = new Map<string, FileSignature>();
  for (const p of paths) out.set(p, fileSignature(p));
  return out;
}

/** True when any recorded file changed, appeared, or disappeared since its signature was taken. */
export function anySignatureChanged(snapshot: Map<string, FileSignature>): boolean {
  for (const [p, recorded] of snapshot) {
    if (fileSignature(p) !== recorded) return true;
  }
  return false;
}
