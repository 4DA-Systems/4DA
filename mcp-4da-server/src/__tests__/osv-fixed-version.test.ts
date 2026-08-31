// SPDX-License-Identifier: Apache-2.0
/**
 * Regression tests for extractFixedVersion — OSV fixed-version branch selection.
 *
 * Live incident (2026-08-23 adversarial scoring audit): vulnerability_scan
 * recommended "Upgrade undici to 6.28.0" while the installed version was
 * 7.28.0 — a DOWNGRADE. Root cause: extractFixedVersion returned the FIRST
 * range's `fixed` event instead of the fixed event from the range containing
 * the installed version.
 */
import { describe, it, expect } from "vitest";
import { extractFixedVersion } from "../live/osv-scanner.js";
import type { OsvVulnerability } from "../live/types.js";

type Affected = NonNullable<OsvVulnerability["affected"]>;
type RangeEvents = Array<{ introduced?: string; fixed?: string }>;

/** Build a one-package `affected` block with the given SEMVER ranges. */
function affected(
  ranges: RangeEvents[],
  pkg = "undici",
  ecosystem = "npm",
  type = "SEMVER",
): Affected {
  return [
    {
      package: { name: pkg, ecosystem },
      ranges: ranges.map((events) => ({ type, events })),
    },
  ];
}

describe("extractFixedVersion — picks the fix from the installed version's range", () => {
  // The undici shape: 6.x line fixed at 6.28.0, 7.x line fixed at 7.28.1.
  const undiciTwoLines = affected([
    [{ introduced: "0" }, { fixed: "6.28.0" }],
    [{ introduced: "7.0.0" }, { fixed: "7.28.1" }],
  ]);

  it("installed on the SECOND line gets that line's fix, not the first range's (the downgrade bug)", () => {
    expect(extractFixedVersion(undiciTwoLines, "undici", "npm", "7.28.0")).toBe("7.28.1");
  });

  it("installed on the FIRST line gets the first line's fix, not a forced major jump", () => {
    expect(extractFixedVersion(undiciTwoLines, "undici", "npm", "6.5.0")).toBe("6.28.0");
  });

  it("single-range normal case returns the range's fix", () => {
    const single = affected([[{ introduced: "0" }, { fixed: "4.17.21" }]], "lodash");
    expect(extractFixedVersion(single, "lodash", "npm", "4.17.12")).toBe("4.17.21");
  });

  it("installed already past every fix returns null — never the live-bug downgrade", () => {
    // Exact live shape: installed 7.28.0, ranges fixed at 6.28.0 and 7.16.0.
    // The old code returned "6.28.0" here.
    const alreadyFixed = affected([
      [{ introduced: "0" }, { fixed: "6.28.0" }],
      [{ introduced: "7.0.0" }, { fixed: "7.16.0" }],
    ]);
    expect(extractFixedVersion(alreadyFixed, "undici", "npm", "7.28.0")).toBeNull();
  });

  it("installed exactly AT the fix version is not vulnerable on that line — null, not a re-recommendation", () => {
    const single = affected([[{ introduced: "0" }, { fixed: "6.28.0" }]]);
    expect(extractFixedVersion(single, "undici", "npm", "6.28.0")).toBeNull();
  });
});

describe("extractFixedVersion — hard guard: never a version below installed", () => {
  it("no fix on the installed line falls back to the smallest fix ABOVE installed", () => {
    // 4.x line has no fix; safety requires crossing to the 5.x line.
    const openLine = affected([
      [{ introduced: "4.5.0" }],
      [{ introduced: "5.0.0" }, { fixed: "5.29.0" }],
    ]);
    expect(extractFixedVersion(openLine, "undici", "npm", "4.8.0")).toBe("5.29.0");
  });

  it("picks the SMALLEST fix above installed, not an arbitrary range's", () => {
    const threeLines = affected([
      [{ introduced: "0" }, { fixed: "5.1.0" }],
      [{ introduced: "6.0.0" }, { fixed: "6.4.0" }],
      [{ introduced: "7.0.0" }, { fixed: "7.2.0" }],
    ]);
    // Installed 5.9.0 sits between lines (5.1.0 <= 5.9.0 < 6.0.0): no containing
    // interval, so the guard path picks 6.4.0 — the nearest safe version up.
    expect(extractFixedVersion(threeLines, "undici", "npm", "5.9.0")).toBe("6.4.0");
  });

  it("every emitted recommendation is strictly above installed across adversarial shapes", () => {
    const shapes: Array<{ aff: Affected; installed: string }> = [
      { aff: affected([[{ introduced: "0" }, { fixed: "1.0.0" }]]), installed: "2.0.0" },
      { aff: affected([[{ fixed: "3.0.0" }]]), installed: "9.9.9" },
      {
        aff: affected([
          [{ introduced: "0" }, { fixed: "6.28.0" }],
          [{ introduced: "7.0.0" }, { fixed: "7.16.0" }],
        ]),
        installed: "7.28.0",
      },
    ];
    for (const { aff, installed } of shapes) {
      const rec = extractFixedVersion(aff, "undici", "npm", installed);
      expect(rec).toBeNull(); // nothing above installed exists in any shape here
    }
  });
});

describe("extractFixedVersion — edge shapes", () => {
  it("a lone fixed event with no introduced is treated as vulnerable since inception", () => {
    const bare = affected([[{ fixed: "2.1.0" }]]);
    expect(extractFixedVersion(bare, "undici", "npm", "2.0.0")).toBe("2.1.0");
    expect(extractFixedVersion(bare, "undici", "npm", "2.1.0")).toBeNull();
  });

  it("multiple intervals inside ONE range resolve per-interval", () => {
    // introduced/fixed/introduced/fixed in a single events list.
    const multiInterval = affected([
      [
        { introduced: "0" },
        { fixed: "1.2.0" },
        { introduced: "2.0.0" },
        { fixed: "2.5.0" },
      ],
    ]);
    expect(extractFixedVersion(multiInterval, "undici", "npm", "1.0.0")).toBe("1.2.0");
    expect(extractFixedVersion(multiInterval, "undici", "npm", "2.2.0")).toBe("2.5.0");
  });

  it("unparseable installed version falls back to the HIGHEST published fix", () => {
    expect(
      extractFixedVersion(
        affected([
          [{ introduced: "0" }, { fixed: "6.28.0" }],
          [{ introduced: "7.0.0" }, { fixed: "7.28.1" }],
        ]),
        "undici",
        "npm",
        "git+deadbeef",
      ),
    ).toBe("7.28.1");
  });

  it("null installed version falls back to the highest published fix", () => {
    expect(
      extractFixedVersion(
        affected([
          [{ introduced: "0" }, { fixed: "1.1.0" }],
          [{ introduced: "2.0.0" }, { fixed: "2.3.0" }],
        ]),
        "undici",
        "npm",
        null,
      ),
    ).toBe("2.3.0");
  });

  it("GIT ranges (commit hashes) are never recommended as versions", () => {
    const gitOnly = affected(
      [[{ introduced: "9a1b2c3d" }, { fixed: "d4e5f6a7" }]],
      "undici",
      "npm",
      "GIT",
    );
    expect(extractFixedVersion(gitOnly, "undici", "npm", "1.0.0")).toBeNull();
  });

  it("returns null for missing affected, empty ranges, or no fixed events", () => {
    expect(extractFixedVersion(undefined, "undici", "npm", "1.0.0")).toBeNull();
    expect(extractFixedVersion(affected([]), "undici", "npm", "1.0.0")).toBeNull();
    expect(
      extractFixedVersion(affected([[{ introduced: "0" }]]), "undici", "npm", "1.0.0"),
    ).toBeNull();
  });

  it("ignores affected blocks for other packages or ecosystems", () => {
    const other = affected([[{ introduced: "0" }, { fixed: "9.9.9" }]], "not-undici");
    expect(extractFixedVersion(other, "undici", "npm", "1.0.0")).toBeNull();
    const otherEco = affected([[{ introduced: "0" }, { fixed: "9.9.9" }]], "undici", "PyPI");
    expect(extractFixedVersion(otherEco, "undici", "npm", "1.0.0")).toBeNull();
  });
});
