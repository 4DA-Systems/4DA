// SPDX-License-Identifier: Apache-2.0
import { describe, it, expect } from "vitest";
import { cvssBaseScore } from "../live/cvss.js";
import { compareSemver, maxSemver } from "../live/semver-utils.js";

describe("cvssBaseScore — CVSS v3.x vector → base score", () => {
  it("scores canonical v3.1 vectors", () => {
    // C:H/I:H/A:H, network, no privs, scope unchanged → 9.8 (critical)
    expect(cvssBaseScore("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H")).toBe(9.8);
    // same but scope changed → 10.0
    expect(cvssBaseScore("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:C/C:H/I:H/A:H")).toBe(10.0);
    // js-yaml DoS vector (A:L only) → 5.3 (medium)
    expect(cvssBaseScore("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:N/A:L")).toBe(5.3);
    // Marvin (rsa) AC:H/C:H → 5.9 (medium)
    expect(cvssBaseScore("CVSS:3.1/AV:N/AC:H/PR:N/UI:N/S:U/C:H/I:N/A:N")).toBe(5.9);
  });

  it("accepts the v3.0 prefix and lowercase metrics", () => {
    expect(cvssBaseScore("CVSS:3.0/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H")).toBe(9.8);
    expect(cvssBaseScore("cvss:3.1/av:n/ac:l/pr:n/ui:n/s:u/c:h/i:h/a:h")).toBe(9.8);
  });

  it("returns 0 when impact is zero (no C/I/A)", () => {
    expect(cvssBaseScore("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:N/A:N")).toBe(0);
  });

  it("returns null for non-v3 / malformed / partial vectors (never a fabricated score)", () => {
    expect(cvssBaseScore("AV:N/AC:L/Au:N/C:P/I:P/A:P")).toBeNull(); // CVSS v2
    expect(
      cvssBaseScore("CVSS:4.0/AV:N/AC:L/AT:N/PR:N/UI:N/VC:H/VI:H/VA:H/SC:N/SI:N/SA:N"),
    ).toBeNull(); // CVSS v4 (different metric names)
    expect(cvssBaseScore("CVSS:3.1/AV:N")).toBeNull(); // missing mandatory metrics
    expect(cvssBaseScore("CVSS:3.1/AV:Z/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H")).toBeNull(); // bad value
    expect(cvssBaseScore("not-a-vector")).toBeNull();
    expect(cvssBaseScore("")).toBeNull();
  });

  it("never throws on adversarial input", () => {
    for (const bad of ["/////", "CVSS:3.1/", "AV:", ":::", "CVSS:3.1/AV:N/AC", "🙂"]) {
      expect(() => cvssBaseScore(bad)).not.toThrow();
    }
  });
});

describe("semver compare / max — picks the highest fix version", () => {
  it("compareSemver orders by major.minor.patch", () => {
    expect(compareSemver("4.17.21", "4.17.12")).toBe(1);
    expect(compareSemver("4.17.12", "4.17.21")).toBe(-1);
    expect(compareSemver("1.2.3", "1.2.3")).toBe(0);
    expect(compareSemver("2.0.0", "1.99.99")).toBe(1);
    expect(compareSemver("garbage", "1.0.0")).toBe(0); // unparseable → 0, never throws
  });

  it("maxSemver returns the highest fix so an upgrade resolves every fixable CVE", () => {
    // lodash CVEs are fixed across 4.17.11/4.17.12/4.17.19/4.17.21/4.18.0 — the
    // recommendation must target 4.18.0, not the first (4.17.11), or it under-fixes.
    expect(maxSemver(["4.17.11", "4.17.12", "4.17.19", "4.17.21", "4.18.0"])).toBe("4.18.0");
    expect(maxSemver([])).toBeNull();
    expect(maxSemver(["1.2.6"])).toBe("1.2.6");
  });
});
