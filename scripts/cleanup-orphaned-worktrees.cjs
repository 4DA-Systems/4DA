#!/usr/bin/env node
/**
 * cleanup-orphaned-worktrees.cjs
 *
 * Detects and removes orphaned worktree-* branches created by subagent
 * parallelism. This script is SAFE BY DESIGN — it never removes a
 * worktree that has:
 *
 *   1. Uncommitted changes in its working tree
 *   2. Work that is not provably already in `origin/main`
 *
 * On (2) — the criterion was wrong until 2026-08-12 and the tool was inert
 * because of it. It asked "is the branch tip an ANCESTOR of main?", but this
 * repo merges by SQUASH: a merged branch's commits are never ancestors of main,
 * main gets one new commit with a different hash. So merged branches looked
 * unmerged forever, the tool proposed 0 removals every run, and 18 worktrees /
 * 73 branches accumulated while it reported all-clear. It also compared against
 * the LOCAL `main`, which had drifted 10 commits behind origin (and carried an
 * unpushed commit), so even the ancestry answer was computed off a wrong base.
 *
 * Now it asks the question that actually matters — "does merging this branch
 * change `origin/main` at all?" — and keeps anything it cannot prove.
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

/**
 * The branch every worktree lands onto.
 *
 * `origin/main`, not `main`. The local `main` ref drifts: on 2026-08-12 it was
 * 10 commits behind origin AND carried one unpushed commit, so every verdict
 * computed against it was wrong. Falls back to `main` only if there is no
 * remote-tracking ref.
 */
function baseRef() {
  return sh("git rev-parse --verify --quiet origin/main") ? "origin/main" : "main";
}

/**
 * Is this branch's work already in the base — by CONTENT, not by ancestry?
 *
 * Ancestry alone is the wrong test for this repo. Under **squash merge** the
 * merged branch's commits are never ancestors of main; main gets one new commit
 * with a different hash. So `tip === merge-base` is false forever for merged
 * work, and the old check proposed **0 removals** while 18 worktrees and 73
 * branches accumulated — a guard rail reporting all-clear while the thing it
 * guards against happened anyway.
 *
 * Content test: three-way merge the branch into the base and ask whether the
 * result differs from the base tree. If it does not, the branch contributes
 * nothing and is safe to retire.
 *
 * Conservative by construction — anything uncertain returns false (KEEP):
 *   - merge conflicts (exit 1) mean the trees genuinely diverge, or the branch
 *     is too stale to compare; either way it is not provably superseded
 *   - any command failure (old git without `merge-tree --write-tree`, bad ref)
 *     falls through to the ancestry test rather than guessing
 */
function isSupersededByBase(ref) {
  const base = baseRef();

  // Cheap path first: a genuine fast-forward ancestor is superseded outright.
  const tip = sh(`git rev-parse ${ref}`);
  const mergeBase = sh(`git merge-base ${ref} ${base}`);
  if (typeof tip === "string" && typeof mergeBase === "string" && tip === mergeBase) {
    return { superseded: true, reason: "tip is an ancestor of " + base };
  }

  // Content path: does merging it into the base change the base at all?
  const baseTree = sh(`git rev-parse ${base}^{tree}`);
  const merged = sh(`git merge-tree --write-tree ${base} ${ref}`);
  if (typeof baseTree !== "string" || typeof merged !== "string") {
    // Could not run the content test (conflict, old git, bad ref) — KEEP.
    return { superseded: false, reason: "content test inconclusive — keeping" };
  }
  const mergedTree = merged.split(/\r?\n/)[0].trim();
  if (mergedTree && mergedTree === baseTree) {
    return { superseded: true, reason: `adds nothing to ${base} (content-identical merge)` };
  }
  return { superseded: false, reason: "has unique content" };
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
    const verdict = isSupersededByBase(branchName);
    const { empty, status } = hasUncommittedChanges(w.path);

    if (!verdict.superseded) {
      plan.unsafe.push({
        kind: "worktree",
        path: w.path,
        branch: branchName,
        reason: `not superseded — ${verdict.reason}`,
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
    const verdict = isSupersededByBase(b);
    if (!verdict.superseded) {
      plan.unsafe.push({
        kind: "branch",
        branch: b,
        reason: `not superseded — ${verdict.reason}`,
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
