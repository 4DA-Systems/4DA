import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    globals: true,
    environment: "node",
    include: ["src/**/*.{test,spec}.{ts,tsx}"],
    exclude: ["node_modules", "dist"],
    setupFiles: ["./src/test-setup.ts"],
    // Several suites write SQLite files and lockfile trees to disk, and one
    // runs `cargo tree`. Each takes 0.1-0.6 s alone; on a machine compiling
    // Rust in parallel (15 cargo/rustc processes, 2026-10-02) the same tests
    // took 5-9 s and tripped the 5 s default. Contention, not a hang.
    testTimeout: 20_000,
  },
});
