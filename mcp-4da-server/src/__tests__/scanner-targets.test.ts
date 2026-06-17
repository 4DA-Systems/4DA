// SPDX-License-Identifier: Apache-2.0
/**
 * Verifies that the project scanner parses platform-gated dependency sections
 * ([target.'cfg(...)'.dependencies]) that the plain section parser skipped,
 * and records each gated dep's target spec.
 */

import { describe, it, expect, beforeEach, afterEach } from "vitest";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { scanCurrentProject } from "../project-scanner.js";

const CARGO_TOML = `
[package]
name = "demo-app"
version = "0.1.0"

[dependencies]
serde = "1"
tokio = { version = "1", features = ["full"] }

[target.'cfg(windows)'.dependencies]
winreg = "0.55"
windows-sys = { version = "0.59", features = ["Win32_System_Console"] }

[target.'cfg(not(windows))'.dependencies]
libc = "0.2"

[dev-dependencies]
tempfile = "3"
`;

const CARGO_LOCK = `
[[package]]
name = "serde"
version = "1.0.193"

[[package]]
name = "tokio"
version = "1.35.0"

[[package]]
name = "winreg"
version = "0.55.0"

[[package]]
name = "windows-sys"
version = "0.59.0"

[[package]]
name = "libc"
version = "0.2.151"
`;

describe("project scanner: platform-gated dependencies", () => {
  let dir: string;

  beforeEach(() => {
    dir = fs.mkdtempSync(path.join(os.tmpdir(), "4da-scan-"));
    fs.writeFileSync(path.join(dir, "Cargo.toml"), CARGO_TOML);
    fs.writeFileSync(path.join(dir, "Cargo.lock"), CARGO_LOCK);
  });

  afterEach(() => {
    fs.rmSync(dir, { recursive: true, force: true });
  });

  it("includes target-gated deps that were previously invisible", () => {
    const scan = scanCurrentProject(dir);
    const rust = scan.depsByEcosystem.rust;
    expect(rust).toBeDefined();
    // Unconditional + platform-gated direct deps are all present now.
    expect(rust.deps).toContain("serde");
    expect(rust.deps).toContain("winreg");
    expect(rust.deps).toContain("windows-sys");
    expect(rust.deps).toContain("libc");
  });

  it("records the target spec for each gated dep (and none for unconditional)", () => {
    const scan = scanCurrentProject(dir);
    expect(scan.depTargets["winreg"]).toBe("cfg(windows)");
    expect(scan.depTargets["windows-sys"]).toBe("cfg(windows)");
    expect(scan.depTargets["libc"]).toBe("cfg(not(windows))");
    // Unconditional deps carry no target gate.
    expect(scan.depTargets["serde"]).toBeUndefined();
    expect(scan.depTargets["tokio"]).toBeUndefined();
  });
});
