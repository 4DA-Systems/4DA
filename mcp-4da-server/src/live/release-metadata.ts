// SPDX-License-Identifier: Apache-2.0
/**
 * Per-release registry metadata, normalised across npm and crates.io.
 *
 * `dependency_check` judges ONE proposed version against the one installed
 * (or, for a new dependency, against the previous release on the same line).
 * That needs facts the version-freshness readers never kept: when each release
 * was published, who/what published it, whether it runs install scripts, and
 * which runtime dependencies it pulls in. Both registry readers project their
 * raw responses into this shape so the signal logic never sees a registry's
 * wire format.
 *
 * Privacy: every request behind this shape carries the package NAME only.
 * Versions are selected locally from the full release list.
 */

/**
 * Publish trust of one release.
 * 2 = published by a registry-verified trusted publisher (npm
 *     `_npmUser.trustedPublisher`, crates.io `trustpub_data`).
 * 1 = carries a provenance attestation (npm `dist.attestations`).
 * 0 = neither: a token / hand publish.
 */
export type TrustLevel = 0 | 1 | 2;

export interface ReleaseRecord {
  version: string;
  /** ISO publish time, or null when the registry does not say. */
  publishedAt: string | null;
  trust: TrustLevel;
  /** Human label for the trust evidence, e.g. "trusted publisher (github)". */
  trustEvidence: string;
  /** npm: install-time lifecycle scripts declared (preinstall/install/postinstall, or a gypfile). Null for crates. */
  installScripts: string[] | null;
  /** npm deprecation message, or null. */
  deprecated: string | null;
  /** crates.io: yanked. npm: unpublished (listed in `time` but gone from `versions`). */
  withdrawn: "yanked" | "unpublished" | null;
  /** Runtime dependency NAMES (npm dependencies + optionalDependencies; crates normal + build). Null when unknown. */
  dependencies: string[] | null;
}

export interface PackageReleaseIndex {
  ecosystem: "npm" | "crates.io";
  name: string;
  /** When the package itself was first published, or null. */
  createdAt: string | null;
  releases: Record<string, ReleaseRecord>;
}

/** The outcome of one registry read: data, a definite "no such package", or "could not reach it". */
export type RegistryLookup<T> =
  | { status: "ok"; data: T }
  | { status: "not_found"; detail: string }
  | { status: "unreachable"; detail: string };

/** What dependency_check needs from a registry. Live readers and test fakes both implement it. */
export interface ReleaseMetadataSource {
  /**
   * The package's release index. `wanted` names the versions the caller is
   * about to read; a reader that pages (crates.io) may stop once it has them
   * and the release just below `wanted[0]` on its line.
   */
  getReleases(name: string, wanted: string[]): Promise<RegistryLookup<PackageReleaseIndex>>;
  /** When a package was first published (for a newly introduced transitive dependency). */
  getCreatedAt(name: string): Promise<RegistryLookup<string | null>>;
}
