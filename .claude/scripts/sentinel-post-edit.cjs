#!/usr/bin/env node
/**
 * sentinel-post-edit.cjs — Incremental bug detection after file edits
 *
 * PostToolUse hook for Edit/Write on Rust files.
 * Runs a fast compilation check (~5s) to catch errors immediately
 * rather than waiting until commit time.
 *
 * Hook type: PostToolUse (matcher: Edit, Write)
 * Behavior: Never blocks. Warns on compilation failure.
 */

let input = "";
process.stdin.setEncoding("utf8");
process.stdin.on("readable", () => {
  let chunk;
  while ((chunk = process.stdin.read()) !== null) input += chunk;
});

process.stdin.on("end", () => {
  try {
    const hookData = JSON.parse(input);
    const toolName = hookData.tool_name || "";
    const toolInput = hookData.tool_input || {};

    // Only trigger on Edit/Write of Rust files
    if (toolName !== "Edit" && toolName !== "Write") {
      console.log("PASS");
      return;
    }

    const filePath = (toolInput.file_path || "").replace(/\\/g, "/");
    if (!filePath.endsWith(".rs")) {
      console.log("PASS");
      return;
    }

    // Skip test files — compilation errors in tests are less urgent
    if (/_tests?\.rs$|test_/.test(filePath)) {
      console.log("PASS");
      return;
    }

    // Debounce: don't check more than once per 30 seconds
    const fs = require("fs");
    const path = require("path");
    const ROOT = path.resolve(__dirname, "..", "..");
    const DEBOUNCE_FILE = path.join(
      ROOT,
      ".claude",
      "wisdom",
      "sentinel-last-check.txt"
    );

    try {
      const lastCheck = parseInt(
        fs.readFileSync(DEBOUNCE_FILE, "utf-8").trim()
      );
      if (Date.now() - lastCheck < 30000) {
        console.log("PASS");
        return;
      }
    } catch {}

    // Record this check time
    try {
      fs.writeFileSync(DEBOUNCE_FILE, String(Date.now()));
    } catch {}

    // GAP 5 fix: skip if cargo/dev server is already running
    const { execSync } = require("child_process");
    try {
      const tasklist = execSync(
        'tasklist //FI "IMAGENAME eq cargo.exe" 2>NUL',
        { encoding: "utf-8", timeout: 3000, stdio: ["pipe", "pipe", "pipe"] }
      );
      if (/cargo\.exe/i.test(tasklist)) {
        console.log("PASS");
        return;
      }
    } catch {}

    // Run fast cargo check
    try {
      execSync("cargo check --lib --message-format=short 2>&1", {
        cwd: path.join(ROOT, "src-tauri"),
        encoding: "utf-8",
        timeout: 60000,
        stdio: ["pipe", "pipe", "pipe"],
      });
      console.log("PASS");
    } catch (err) {
      const output = (err.stdout || "") + (err.stderr || "");
      const errors = output
        .split("\n")
        .filter((l) => /^error/.test(l))
        .slice(0, 3);

      if (errors.length > 0) {
        console.log(
          "PASS\n" +
            "SENTINEL: Rust compilation error detected after edit.\n" +
            errors.map((e) => `  ${e}`).join("\n") +
            "\nFix the compilation error before proceeding."
        );
      } else {
        console.log("PASS");
      }
    }
  } catch {
    console.log("PASS");
  }
});
