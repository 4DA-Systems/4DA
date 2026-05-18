// SPDX-License-Identifier: FSL-1.1-Apache-2.0
// Validated technology vocabulary for frontend topic extraction.
// Mirrors the backend vocabulary in src-tauri/src/utils/topics.rs (SINGLE_WORD_TOPICS).
// Only words in this set are accepted as topics — prevents random title words
// from polluting learned preference facets.

const KNOWN_TECH = new Set([
  'rust', 'python', 'javascript', 'typescript', 'golang', 'java', 'kotlin', 'swift',
  'ruby', 'php', 'scala', 'elixir', 'haskell', 'dart', 'zig', 'lua', 'julia',
  'react', 'vue', 'angular', 'svelte', 'solid', 'htmx', 'alpine', 'preact',
  'nextjs', 'nuxt', 'remix', 'astro', 'sveltekit', 'fresh',
  'tauri', 'electron', 'flutter', 'expo', 'swiftui',
  'django', 'flask', 'fastapi', 'rails', 'spring', 'express', 'actix', 'axum', 'rocket',
  'tokio', 'serde', 'reqwest', 'sqlx', 'diesel', 'hyper', 'warp', 'tonic',
  'docker', 'kubernetes', 'terraform', 'ansible', 'nginx', 'caddy',
  'postgresql', 'postgres', 'mysql', 'mongodb', 'redis', 'sqlite', 'elasticsearch',
  'graphql', 'grpc', 'websocket', 'rest',
  'wasm', 'webassembly', 'webgpu', 'opengl', 'vulkan',
  'linux', 'macos', 'windows', 'android', 'ios',
  'git', 'github', 'gitlab', 'ci', 'testing', 'security', 'performance',
  'ai', 'ml', 'llm', 'gpt', 'claude', 'ollama', 'embedding', 'rag',
  'tensorflow', 'pytorch', 'numpy', 'pandas', 'scikit-learn',
  'vercel', 'netlify', 'cloudflare', 'aws', 'azure', 'gcp',
  'deno', 'bun', 'node', 'npm', 'pnpm', 'cargo', 'pip',
  'css', 'html', 'sass', 'tailwind',
  'vite', 'webpack', 'esbuild', 'rollup', 'turbopack',
  'eslint', 'prettier', 'biome',
  'prisma', 'drizzle', 'typeorm', 'sequelize', 'mongoose', 'knex',
  'trpc', 'zod', 'yup',
  'storybook', 'playwright', 'cypress', 'vitest', 'jest',
  'supabase', 'firebase', 'neon', 'planetscale', 'turso',
  'stripe', 'auth0', 'clerk', 'lucia',
  'openai', 'anthropic', 'huggingface', 'langchain',
  'nix', 'homebrew', 'apt',
  'wgsl', 'glsl', 'shader', 'compute',
  'crate', 'package', 'module', 'plugin', 'extension',
  'api', 'sdk', 'cli', 'tui', 'gui',
  'migration', 'deployment', 'monitoring', 'logging', 'caching',
  'authentication', 'authorization', 'encryption', 'oauth',
  'microservice', 'monolith', 'serverless', 'edge',
  'compiler', 'parser', 'lexer', 'interpreter', 'runtime',
  'concurrency', 'async', 'parallel', 'threading',
  'benchmark', 'profiling', 'optimization',
]);

/** Extract recognized technology topics from a title string. */
export function extractTechTopics(title: string, maxTopics = 5): string[] {
  return title.toLowerCase()
    .split(/[\s\-—:,.()/]+/)
    .filter(w => KNOWN_TECH.has(w))
    .slice(0, maxTopics);
}
