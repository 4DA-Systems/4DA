#!/usr/bin/env node
/**
 * cleanup-orphaned-worktrees.cjs
 *
 * Detects and removes orphaned worktree-* branches created by subagent
 * parallelism. This script is SAFE BY DESIGN — it never removes a
 * worktree that has:
 *
 *   1. Uncommitted changes in its working tree
 *   2. Commits that are not already merged — established EITHER by ancestry
 *      (tip reachable from main) OR by a merged PR whose head OID is exactly
 *      the branch tip. The second criterion is required because this repo
 *      squash-merges, which never makes a branch an ancestor of main.
 *
 * Run modes:
 *
 *   node scripts/cleanup-orphaned-worktrees.cjs            # dry-run, shows what would go
 *   node scripts/cleanup-orphaned-worktrees.cjs --execute  # actually delete
 *   node scripts/cleanup-orphaned-worktrees.cjs --force    # include worktrees with only UNTRACKED files
 *
 * Background:
 *
 * When a Task subagent is spawned with `isolation: "worktree"`, Claude
 * Code creates a new worktree under `.claude/worktrees/agent-<hash>/`
 * and a matching branch `worktree-agent-<hash>`. After the subagent
 * commits and returns, the orchestrator merges or cherry-picks the
 * work onto main — but the worktree directory and branch remain.
 *
 * Over time these accumulate. Eleven of them at once caused the sentinel
 * alarm on 2026-04-12 ("41 unclaimed files accumulating"). This script
 * is the prevention.
 *
 * Suggested cadence: run via a pre-push hook or nightly cron.
 */

const { execSync } = require("node:child_process");
const path = require("node:path");
const fs = require("node:fs");

const args = process.argv.slice(2);
const execute = args.includes("--execute");
const force = args.includes("--force");

const sh = (cmd) => {
  try {
    return execSync(cmd, { encoding: "utf8", stdio: ["pipe", "pipe", "pipe"] }).trim();
  } catch (err) {
    return { error: err.stderr?.toString() ?? err.message };
  }
};

function listWorktrees() {
  // Parse `git worktree list --porcelain` into {path, branch, head}.
  // The porcelain format uses keys "worktree", "HEAD", "branch" — we
  // normalise "worktree" → "path" for cleaner downstream access.
  const out = sh("git worktree list --porcelain");
  if (typeof out !== "string") throw new Error(`git worktree list failed: ${JSON.stringify(out)}`);
  const entries = [];
  let cur = {};
  const flush = () => {
    if (cur.path || cur.worktree) {
      if (!cur.path && cur.worktree) cur.path = cur.worktree;
      entries.push(cur);
    }
  };
  for (const line of out.split(/\r?\n/)) {
    if (!line) {
      flush();
      cur = {};
      continue;
    }
    const [key, ...rest] = line.split(" ");
    cur[key] = rest.join(" ");
  }
  flush();
  return entries;
}

function listOrphanBranches() {
  // Every `worktree-*` branch, even ones without an active worktree
  const out = sh('git branch --list "worktree-*" --format "%(refname:short)"');
  if (typeof out !== "string") return [];
  return out.split(/\r?\n/).filter(Boolean);
}

function isReachableFromMain(ref) {
  const tip = sh(`git rev-parse ${ref}`);
  const mergeBase = sh(`git merge-base ${ref} main`);
  return typeof tip === "string" && typeof mergeBase === "string" && tip === mergeBase;
}

/**
 * Head refs of merged PRs, as Map<branchName, headRefOid>.
 *
 * WHY THIS EXISTS: this repo merges by SQUASH. A squash merge replays the
 * branch as a NEW commit on main, so the branch tip is never an ancestor of
 * main and `isReachableFromMain` is permanently FALSE for every successfully
 * merged PR. Ancestry alone therefore protects everything forever: measured
 * 2026-08-24 on this repo — 44 registered worktrees, 46 `worktree-*` branches,
 * and a plan of "0 dirs, 0 branches", with #488/#493/#427 (all MERGED) each
 * reported as "has unique commits".
 *
 * Returns null when `gh` is missing, unauthenticated, or returns junk — the
 * caller then falls back to ancestry-only, i.e. the original conservative
 * behaviour. This must never make the script LESS safe than before.
 */
function getMergedPrHeads() {
  const out = sh(
    "gh pr list --state merged --limit 500 --json number,headRefName,headRefOid"
  );
  if (typeof out !== "string" || out.length === 0) return null;
  let rows;
  try {
    rows = JSON.parse(out);
  } catch {
    return null;
  }
  if (!Array.isArray(rows)) return null;
  const heads = new Map();
  for (const row of rows) {
    if (row && row.headRefName && row.headRefOid) {
      heads.set(row.headRefName, row.headRefOid);
    }
  }
  return heads;
}

/**
 * True only when `ref`'s PR is merged AND the local tip is EXACTLY the commit
 * that was merged.
 *
 * The OID equality is what makes this as safe as the ancestry check: a branch
 * that received further commits after its PR merged will not match, so that
 * unmerged work stays protected. A branch older than the `--limit` window
 * simply will not be found, which is likewise conservative.
 */
function isMergedPrHead(ref, mergedHeads) {
  if (!mergedHeads) return false;
  const mergedOid = mergedHeads.get(ref);
  if (!mergedOid) return false;
  const tip = sh(`git rev-parse ${ref}`);
  return typeof tip === "string" && tip === mergedOid;
}

function hasUncommittedChanges(dirPath) {
  if (!fs.existsSync(dirPath)) return { empty: true, status: "" };
  const out = sh(`git -C "${dirPath}" status --short`);
  if (typeof out !== "string") return { empty: false, status: "(status failed)" };
  return { empty: out.length === 0, status: out };
}

function main() {
  const mainRoot = sh("git rev-parse --show-toplevel");
  if (typeof mainRoot !== "string") {
    console.error("Not inside a git repo.");
    process.exit(1);
  }

  const worktrees = listWorktrees();
  const orphanBranches = listOrphanBranches();
  const mergedHeads = getMergedPrHeads();

  // Split worktrees: main vs worktree-*
  // `git worktree list --porcelain` writes `branch refs/heads/<name>`, so
  // matching with or without the `refs/heads/` prefix keeps us robust.
  const isMainBranch = (w) =>
    w.branch === "main" || w.branch === "refs/heads/main";
  const isWorktreeBranch = (w) =>
    w.branch &&
    (w.branch.startsWith("worktree-") || w.branch.startsWith("refs/heads/worktree-"));
  const mainEntry = worktrees.find(isMainBranch);
  const worktreeWorktrees = worktrees.filter(isWorktreeBranch);

  console.log(`Main worktree:       ${mainEntry?.path ?? "(unknown)"}`);
  console.log(`Worktree-* dirs:     ${worktreeWorktrees.length}`);
  console.log(`Worktree-* branches: ${orphanBranches.length}`);
  console.log(
    mergedHeads
      ? `Merged PR heads:     ${mergedHeads.size} (squash-merge detection ACTIVE)`
      : "Merged PR heads:     unavailable — `gh` missing/unauthenticated;" +
        " falling back to ancestry-only (conservative, squash-merged branches stay protected)"
  );
  console.log("");

  const plan = {
    dirsToRemove: [],
    branchesToDelete: [],
    orphanedDirsOnDisk: [],
    unsafe: [],
  };

  // Phase 1: worktrees that git knows about
  for (const w of worktreeWorktrees) {
    const branchName = w.branch.replace(/^refs\/heads\//, "");
    const merged =
      isReachableFromMain(branchName) || isMergedPrHead(branchName, mergedHeads);
    const { empty, status } = hasUncommittedChanges(w.path);

    if (!merged) {
      plan.unsafe.push({
        kind: "worktree",
        path: w.path,
        branch: branchName,
        reason:
          "not merged — tip is not reachable from main and no merged PR has this tip",
      });
      continue;
    }
    if (!empty && !force) {
      plan.unsafe.push({
        kind: "worktree",
        path: w.path,
        branch: branchName,
        reason: `uncommitted changes present:\n${status}`,
      });
      continue;
    }
    plan.dirsToRemove.push(w.path);
    plan.branchesToDelete.push(branchName);
  }

  // Phase 2: dead branches with no active worktree
  const stillLiveBranches = new Set(
    worktreeWorktrees.map((w) => w.branch.replace(/^refs\/heads\//, ""))
  );
  for (const b of orphanBranches) {
    if (stillLiveBranches.has(b)) continue; // already handled above
    if (!isReachableFromMain(b) && !isMergedPrHead(b, mergedHeads)) {
      plan.unsafe.push({
        kind: "branch",
        branch: b,
        reason:
          "not merged — tip is not reachable from main and no merged PR has this tip",
      });
      continue;
    }
    plan.branchesToDelete.push(b);
  }

  // Phase 3: orphaned directories on disk that git forgot about
  const wtDir = path.join(mainRoot, ".claude", "worktrees");
  if (fs.existsSync(wtDir)) {
    for (const entry of fs.readdirSync(wtDir)) {
      const full = path.join(wtDir, entry);
      const stat = fs.statSync(full);
      if (!stat.isDirectory()) continue;
      const knownToGit = worktrees.some(
        (w) => w.path && path.resolve(w.path) === path.resolve(full)
      );
      if (knownToGit) continue;
      // Check for .git metadata
      const hasGitMarker = fs.existsSync(path.join(full, ".git"));
      if (hasGitMarker) {
        plan.unsafe.push({
          kind: "orphan-dir",
          path: full,
          reason: "has .git marker — git may still track it",
        });
        continue;
      }
      plan.orphanedDirsOnDisk.push(full);
    }
  }

  // Report
  console.log("=== Plan ===");
  console.log(`  Dirs to remove (git worktree remove):  ${plan.dirsToRemove.length}`);
  plan.dirsToRemove.forEach((p) => console.log(`    ${p}`));
  console.log(`  Branches to delete:                    ${plan.branchesToDelete.length}`);
  plan.branchesToDelete.forEach((b) => console.log(`    ${b}`));
  console.log(`  Orphaned dirs on disk (rm -rf):        ${plan.orphanedDirsOnDisk.length}`);
  plan.orphanedDirsOnDisk.forEach((p) => console.log(`    ${p}`));
  console.log("");

  if (plan.unsafe.length > 0) {
    console.log("=== NOT TOUCHING (unsafe) ===");
    for (const u of plan.unsafe) {
      console.log(`  ${u.kind}: ${u.path ?? u.branch}`);
      console.log(`    reason: ${u.reason}`);
    }
    console.log("");
  }

  if (!execute) {
    console.log("Dry-run mode. Rerun with --execute to apply.");
    return;
  }

  // Execute
  console.log("=== Executing ===");
  for (const dir of plan.dirsToRemove) {
    const r = sh(`git worktree remove "${dir}"`);
    console.log(`  removed worktree ${dir}${r?.error ? " — FAILED: " + r.error : ""}`);
  }
  sh("git worktree prune");
  for (const b of plan.branchesToDelete) {
    const r = sh(`git branch -D "${b}"`);
    console.log(`  deleted branch ${b}${r?.error ? " — FAILED: " + r.error : ""}`);
  }
  for (const dir of plan.orphanedDirsOnDisk) {
    try {
      fs.rmSync(dir, { recursive: true, force: true });
      console.log(`  removed orphaned dir ${dir}`);
    } catch (err) {
      console.log(`  FAILED to remove ${dir}: ${err.message}`);
    }
  }
  console.log("");
  console.log("Done. Reflog preserves everything for 90 days in case of mistakes.");
}

main();
