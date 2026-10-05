import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// Paths normalised to forward slashes and lower case: the watcher reports
// Windows paths with either separator and drive-letter case varies.
const norm = (p: string) => p.replace(/\\/g, "/").replace(/\/+$/, "").toLowerCase();
const claudeDir = norm(
  decodeURIComponent(new URL("./.claude", import.meta.url).pathname).replace(/^\/([a-zA-Z]:)/, "$1"),
);
const isInside = (file: string, dir: string) => {
  const f = norm(file);
  return f === dir || f.startsWith(`${dir}/`);
};

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],

  define: {
    __APP_VERSION__: JSON.stringify(process.env.npm_package_version || '1.0.0'),
  },

  build: {
    // No source maps in production (smaller bundle)
    sourcemap: false,
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes('node_modules/react-dom') || id.includes('node_modules/react/')) return 'vendor-react';
          if (id.includes('@tauri-apps/')) return 'vendor-tauri';
          if (id.includes('i18next') || id.includes('react-i18next')) return 'vendor-i18n';
          if (id.includes('zustand')) return 'vendor-store';
          if (id.includes('@tanstack/react-virtual')) return 'vendor-virtual';
        },
      },
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 4444,
    strictPort: true,
    host: host || "0.0.0.0",
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 4445,
        }
      : undefined,
    watch: {
      // Only source files (src/, public/, index.html) should trigger HMR.
      // Any dir with runtime writes MUST be listed here or it creates an
      // infinite reload loop: write → Vite reload → React mount → write.
      ignored: [
        "**/src-tauri/**",
        "**/data/**",
        "**/temp/**",
        // THIS project's .claude/ only. The glob "**/.claude/**" also matched
        // the project root itself when the tree lives under .claude/ (every
        // agent worktree: D:/4DA/.claude/worktrees/<name>/), so a dev server
        // started in a worktree ignored ALL edits and kept serving stale code.
        (file: string) => isInside(file, claudeDir),
        "**/4da-stderr.txt",
      ],
    },
  },
}));
