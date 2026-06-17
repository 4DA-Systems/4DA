// SPDX-License-Identifier: Apache-2.0
/**
 * Unit tests for target/platform relevance evaluation.
 * Evaluated against explicit hosts so the tests are deterministic on any CI OS.
 */

import { describe, it, expect } from "vitest";
import { targetActiveOnHost, evalCfg, hostTarget, type HostTarget } from "../live/platform.js";

const WIN: HostTarget = { os: "windows", family: "windows", arch: "x86_64" };
const LINUX_ARM: HostTarget = { os: "linux", family: "unix", arch: "aarch64" };

describe("targetActiveOnHost", () => {
  it("treats unconditional (null) deps as always active", () => {
    expect(targetActiveOnHost(null, WIN)).toBe(true);
    expect(targetActiveOnHost(undefined, LINUX_ARM)).toBe(true);
    expect(targetActiveOnHost("", WIN)).toBe(true);
  });

  it("evaluates bare windows / unix predicates", () => {
    expect(targetActiveOnHost("cfg(windows)", WIN)).toBe(true);
    expect(targetActiveOnHost("cfg(windows)", LINUX_ARM)).toBe(false);
    expect(targetActiveOnHost("cfg(unix)", WIN)).toBe(false);
    expect(targetActiveOnHost("cfg(unix)", LINUX_ARM)).toBe(true);
  });

  it("handles not()", () => {
    expect(targetActiveOnHost("cfg(not(windows))", WIN)).toBe(false);
    expect(targetActiveOnHost("cfg(not(windows))", LINUX_ARM)).toBe(true);
  });

  it("evaluates key = value predicates", () => {
    expect(targetActiveOnHost('cfg(target_os = "windows")', WIN)).toBe(true);
    expect(targetActiveOnHost('cfg(target_os = "linux")', WIN)).toBe(false);
    expect(targetActiveOnHost('cfg(target_family = "unix")', LINUX_ARM)).toBe(true);
    expect(targetActiveOnHost('cfg(target_arch = "aarch64")', LINUX_ARM)).toBe(true);
    expect(targetActiveOnHost('cfg(target_arch = "x86_64")', LINUX_ARM)).toBe(false);
  });

  it("handles all() and any()", () => {
    expect(targetActiveOnHost('cfg(all(windows, target_arch = "x86_64"))', WIN)).toBe(true);
    expect(targetActiveOnHost('cfg(all(windows, target_arch = "aarch64"))', WIN)).toBe(false);
    expect(targetActiveOnHost("cfg(any(unix, windows))", WIN)).toBe(true);
    expect(targetActiveOnHost("cfg(any(unix, windows))", LINUX_ARM)).toBe(true);
    expect(targetActiveOnHost('cfg(any(target_os = "macos", target_os = "ios"))', WIN)).toBe(false);
  });

  it("evaluates explicit target triples", () => {
    expect(targetActiveOnHost("x86_64-pc-windows-msvc", WIN)).toBe(true);
    expect(targetActiveOnHost("x86_64-unknown-linux-gnu", WIN)).toBe(false);
    expect(targetActiveOnHost("aarch64-apple-darwin", LINUX_ARM)).toBe(false);
    expect(targetActiveOnHost("aarch64-unknown-linux-gnu", LINUX_ARM)).toBe(true);
  });

  it("is conservative: unknown predicates are treated as active (never hide a real advisory)", () => {
    expect(evalCfg("feature = \"some_feature\"", WIN)).toBe(true);
    expect(evalCfg("mystery_predicate", WIN)).toBe(true);
  });

  it("hostTarget reflects the running process", () => {
    const h = hostTarget();
    expect(["windows", "macos", "linux"]).toContain(h.os);
    expect(["windows", "unix"]).toContain(h.family);
  });
});
