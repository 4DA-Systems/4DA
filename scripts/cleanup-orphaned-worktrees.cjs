#!/usr/bin/env node
/**
 * cleanup-orphaned-worktrees.cjs
 *
 * Detects and removes orphaned worktree-* branches created by subagent
 * parallelism. This script is SAFE BY DESIGN — it never removes a
 * worktree that has:
 *
 *   1. Uncommitted changes in its working tree
 *   2. Commits whose content is not already in the base. "In the base" is
 *      established by ANY of four independent proofs — this repo squash-merges
 *      every PR, so a merged branch's commits are NEVER reachable from main,
 *      and ancestry alone refused every merged lane forever while 40+ of them
 *      accumulated (2026-08-24 audit):
 *
 *        a. ANCESTRY — tip reachable from the base (non-squash flows).
 *        b. TREE IDENTITY — the branch tip's tree object appears in recent
 *           base history, i.e. the base once held this branch's exact final
 *           state. Offline, needs no API.
 *        c. MERGED PR HEAD — a merged PR's headRefOid is exactly the tip.
 *        d. CONTENT-IDENTICAL MERGE — three-way merging the branch into the
 *           base leaves the base tree unchanged (`git merge-tree
 *           --write-tree`), i.e. the branch adds nothing. Catches work that
 *           landed on the base outside its own PR (harvested/cherry-picked
 *           lanes) that (b) and (c) cannot see.
 *
 *      (b), (c) and (d) are COMPLEMENTARY, not redundant. Tree identity only
 *      matches when the branch was up to date with main at merge time; that
 *      is guaranteed today by the strict "branches must be up to date"
 *      ruleset, but was NOT for older merges, and it is bounded by
 *      MAIN_TREE_DEPTH. Measured 2026-08-24 on this tree: (a)+(b) reclaimed
 *      2 branches, adding (c) reclaimed 30 — the extra 28 were verified
 *      MERGED with exact OID match (e.g. #357, #414, #465, #471).
 *
 *      Every proof is content-level and conservative: anything unproven stays
 *      protected, and reflog keeps 90 days regardless.
 *
 *      The BASE is `origin/main`, not the local `main` ref. The local ref
 *      drifts: on 2026-08-12 it was 10 commits behind origin AND carried one
 *      unpushed commit, so every verdict computed against it was wrong.
 *      Falls back to `main` only when no remote-tracking ref exists.
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
let baseRefCache = null;
function baseRef() {
  if (!baseRefCache) {
    const remote = sh("git rev-parse --verify --quiet origin/main");
    baseRefCache = typeof remote === "string" && remote ? "origin/main" : "main";
  }
  return baseRefCache;
}

// Proof (a): a genuine fast-forward ancestor is in the base outright.
function isReachableFromBase(ref, base) {
  const tip = sh(`git rev-parse ${ref}`);
  const mergeBase = sh(`git merge-base ${ref} ${base}`);
  return typeof tip === "string" && typeof mergeBase === "string" && tip === mergeBase;
}

// Squash-merge awareness (proof b): how far back in base history to look for
// a branch's tree. Main lands ~5-15 commits/day, so 500 covers a month-plus;
// a lane older than the window simply stays protected (fail-safe).
const MAIN_TREE_DEPTH = 500;
let baseTreesCache = null;
function baseTrees(base) {
  if (!baseTreesCache) {
    const out = sh(`git log --format=%T -${MAIN_TREE_DEPTH} ${base}`);
    baseTreesCache = new Set(
      typeof out === "string" ? out.split(/\r?\n/).filter(Boolean) : []
    );
  }
  return baseTreesCache;
}

// True when the branch tip's TREE object is byte-identical to the tree of
// some commit in recent base history — i.e. the base has held this branch's
// exact final state (the squash-merge signature). Content-level proof:
// deleting the branch cannot lose anything the merge flow kept.
function isSquashMergedIntoBase(ref, base) {
  const tree = sh(`git rev-parse "${ref}^{tree}"`);
  return typeof tree === "string" && baseTrees(base).has(tree);
}

/**
 * Proof (c): head refs of merged PRs, as Map<branchName, headRefOid>.
 *
 * Tree identity (b) only matches when the branch was up to date with main at
 * merge time — guaranteed by today's strict ruleset, but NOT for older merges,
 * where main had moved on and the squash commit's tree therefore differs from
 * the branch tip's. Those lanes are still fully merged; the PR record proves it
 * when the tree cannot. Measured 2026-08-24: (a)+(b) alone reclaimed 2 lanes,
 * (a)+(b)+(c) reclaimed 30.
 *
 * Returns null when `gh` is missing, unauthenticated, or returns junk — the
 * caller then simply relies on the offline proofs. This must never make the
 * script LESS safe than before.
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
 * The OID equality is what makes this as strict as ancestry: a branch that
 * received further commits after its PR merged will not match, so that unmerged
 * work stays protected. A branch older than the `--limit` window simply is not
 * found, which is likewise conservative.
 */
function isMergedPrHead(ref, mergedHeads) {
  if (!mergedHeads) return false;
  const mergedOid = mergedHeads.get(ref);
  if (!mergedOid) return false;
  const tip = sh(`git rev-parse ${ref}`);
  return typeof tip === "string" && tip === mergedOid;
}

/**
 * Proof (d): does merging this branch into the base change the base AT ALL?
 *
 * Three-way merge via `git merge-tree --write-tree` compared to the base tree.
 * If the merged tree equals the base tree, the branch contributes nothing —
 * its work already landed, even if it landed through someone else's commit
 * (harvested lanes, cherry-picks) that proofs (b)/(c) cannot see.
 *
 * Three-valued and conservative by construction:
 *   - true  → provably adds nothing
 *   - false → has unique content
 *   - null  → INCONCLUSIVE (merge conflict, old git without `merge-tree
 *             --write-tree`, bad ref) — the caller must KEEP, never guess
 */
function addsNothingToBase(ref, base) {
  const baseTree = sh(`git rev-parse ${base}^{tree}`);
  const merged = sh(`git merge-tree --write-tree ${base} ${ref}`);
  if (typeof baseTree !== "string" || typeof merged !== "string") return null;
  const mergedTree = merged.split(/\r?\n/)[0].trim();
  if (mergedTree && mergedTree === baseTree) return true;
  return false;
}

// The combined safety predicate: content is in the base by ancestry (a), by
// squash-merge tree identity (b), by merged-PR head OID (c), or by
// content-identical merge (d). Returns a verdict with the WINNING PROOF named,
// so the report explains itself instead of printing a flat boolean.
function supersededVerdict(ref, mergedHeads, base) {
  if (isReachableFromBase(ref, base)) {
    return { superseded: true, reason: `tip is an ancestor of ${base}` };
  }
  if (isSquashMergedIntoBase(ref, base)) {
    return {
      superseded: true,
      reason: `tree matches a recent ${base} commit (squash-merge signature)`,
    };
  }
  if (isMergedPrHead(ref, mergedHeads)) {
    return { superseded: true, reason: "merged PR head OID matches tip exactly" };
  }
  const contentVerdict = addsNothingToBase(ref, base);
  if (contentVerdict === true) {
    return { superseded: true, reason: `adds nothing to ${base} (content-identical merge)` };
  }
  if (contentVerdict === null) {
    return {
      superseded: false,
      reason: "no proof, and content test inconclusive (conflict/old git) — keeping",
    };
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
  const mergedHeads = getMergedPrHeads();
  const base = baseRef();

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
  console.log(`Base ref:            ${base}`);
  console.log(`Worktree-* dirs:     ${worktreeWorktrees.length}`);
  console.log(`Worktree-* branches: ${orphanBranches.length}`);
  console.log(
    mergedHeads
      ? `Merged PR heads:     ${mergedHeads.size} (proof c ACTIVE, alongside ancestry + tree identity + content-identical merge)`
      : "Merged PR heads:     unavailable — `gh` missing/unauthenticated; using" +
        " ancestry + tree identity + content-identical merge only (conservative:" +
        " lanes only a PR record can prove stay protected)"
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
    const verdict = supersededVerdict(branchName, mergedHeads, base);
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
    const verdict = supersededVerdict(b, mergedHeads, base);
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
