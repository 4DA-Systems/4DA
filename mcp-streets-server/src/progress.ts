// SPDX-License-Identifier: FSL-1.1-Apache-2.0
/**
 * SQLite-backed progress tracking for STREETS MCP Server
 *
 * Stores lesson completion state in a local SQLite database.
 * DB path: ~/.local/share/streets/progress.db
 */

import Database from "better-sqlite3";
import { existsSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import { homedir } from "node:os";

import type { ModuleProgress, ProgressReport, MarkCompleteResult, NextStepResult, ModuleId } from "./types.js";
import type { ContentLoader } from "./content.js";

// =============================================================================
// Module ordering (for next-step logic)
// =============================================================================

const MODULE_ORDER: ModuleId[] = ["S", "T", "R", "E1", "E2", "T2", "S2"];

// =============================================================================
// ProgressStore class
// =============================================================================

export class ProgressStore {
  private db: Database.Database;

  constructor() {
    const dbDir = this.getDbDir();
    if (!existsSync(dbDir)) {
      mkdirSync(dbDir, { recursive: true });
    }

    const dbPath = join(dbDir, "progress.db");
    this.db = new Database(dbPath);

    // Enable WAL mode for better concurrent access
    this.db.pragma("journal_mode = WAL");

    // Create table if not exists
    this.db.exec(`
      CREATE TABLE IF NOT EXISTS progress (
        module_id TEXT NOT NULL,
        lesson_idx INTEGER NOT NULL,
        completed_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        PRIMARY KEY (module_id, lesson_idx)
      );
    `);
  }

  /**
   * Get the database directory path
   */
  private getDbDir(): string {
    if (process.platform === "win32") {
      // On Windows, use %LOCALAPPDATA%/streets or fallback to ~/.local/share/streets
      const localAppData = process.env.LOCALAPPDATA;
      if (localAppData) {
        return join(localAppData, "streets");
      }
    }
    return join(homedir(), ".local", "share", "streets");
  }

  /**
   * Get progress for all modules
   */
  getProgress(content: ContentLoader): ProgressReport {
    const modules: ModuleProgress[] = [];
    let totalCompleted = 0;
    let totalLessons = 0;

    for (const moduleId of content.getModuleIds()) {
      try {
        const lessonCount = content.getLessonCount(moduleId);
        const completed = this.getCompletedCount(moduleId);
        const percentage = lessonCount > 0 ? Math.round((completed / lessonCount) * 100) : 0;

        modules.push({
          module_id: moduleId,
          completed_lessons: completed,
          total_lessons: lessonCount,
          percentage,
        });

        totalCompleted += completed;
        totalLessons += lessonCount;
      } catch {
        // Module failed to load, skip
        continue;
      }
    }

    const overall_percentage = totalLessons > 0
      ? Math.round((totalCompleted / totalLessons) * 100)
      : 0;

    return { modules, overall_percentage };
  }

  /**
   * Mark a lesson as complete
   */
  markComplete(content: ContentLoader, moduleId: string, lessonIdx: number): MarkCompleteResult {
    // Validate module ID
    if (!content.isValidModuleId(moduleId)) {
      throw new Error(`Invalid module_id: "${moduleId}". Valid IDs: ${content.getModuleIds().join(", ")}`);
    }

    // Validate lesson index
    const lessonCount = content.getLessonCount(moduleId);
    if (lessonIdx < 0 || lessonIdx >= lessonCount) {
      throw new Error(
        `Invalid lesson_idx: ${lessonIdx} for module ${moduleId}. Valid range: 0-${lessonCount - 1}`
      );
    }

    // Insert or ignore (idempotent)
    const stmt = this.db.prepare(
      "INSERT OR IGNORE INTO progress (module_id, lesson_idx) VALUES (?, ?)"
    );
    stmt.run(moduleId, lessonIdx);

    // Return updated progress for this module
    const completed = this.getCompletedCount(moduleId);
    const percentage = lessonCount > 0 ? Math.round((completed / lessonCount) * 100) : 0;

    return {
      success: true,
      module_id: moduleId,
      lesson_idx: lessonIdx,
      module_progress: {
        module_id: moduleId,
        completed_lessons: completed,
        total_lessons: lessonCount,
        percentage,
      },
    };
  }

  /**
   * Get the recommended next step based on current progress
   */
  getNextStep(content: ContentLoader): NextStepResult {
    // Walk modules in order, find the first incomplete lesson
    for (const moduleId of MODULE_ORDER) {
      try {
        const lessonCount = content.getLessonCount(moduleId);
        const completedLessons = this.getCompletedLessons(moduleId);

        for (let i = 0; i < lessonCount; i++) {
          if (!completedLessons.has(i)) {
            const mod = content.getModule(moduleId);
            const lesson = mod.lessons[i];
            const completedInModule = completedLessons.size;

            let reason: string;
            if (completedInModule === 0) {
              reason = `Start Module ${moduleId}: ${mod.title}. This is the next module in the STREETS sequence.`;
            } else {
              reason = `Continue Module ${moduleId}: ${mod.title}. You've completed ${completedInModule}/${lessonCount} lessons.`;
            }

            // Build context about what comes before/after
            const moduleIdx = MODULE_ORDER.indexOf(moduleId);
            let context = "";
            if (moduleIdx > 0) {
              const prevId = MODULE_ORDER[moduleIdx - 1];
              const prevProgress = this.getCompletedCount(prevId);
              const prevTotal = content.getLessonCount(prevId);
              context += `Previous module (${prevId}): ${prevProgress}/${prevTotal} complete. `;
            }
            if (moduleIdx < MODULE_ORDER.length - 1) {
              const nextId = MODULE_ORDER[moduleIdx + 1];
              context += `Next module after this: ${nextId} (${content.getModule(nextId).title}).`;
            }

            return {
              next_module_id: moduleId,
              next_lesson_idx: i,
              reason,
              context: context.trim() || `Lesson: ${lesson.title}`,
            };
          }
        }
      } catch {
        // Module failed to load, skip
        continue;
      }
    }

    // All modules complete
    return {
      next_module_id: "S2",
      next_lesson_idx: 0,
      reason: "Congratulations! You have completed all STREETS modules. Review Module S2 (Stacking Streams) to optimize your income portfolio.",
      context: "All 7 modules complete. Focus on executing your Stream Stack plan.",
    };
  }

  /**
   * Close the database connection
   */
  close(): void {
    this.db.close();
  }

  // ===========================================================================
  // Private helpers
  // ===========================================================================

  /**
   * Count completed lessons for a module
   */
  private getCompletedCount(moduleId: string): number {
    const row = this.db.prepare(
      "SELECT COUNT(*) as cnt FROM progress WHERE module_id = ?"
    ).get(moduleId) as { cnt: number } | undefined;
    return row?.cnt ?? 0;
  }

  /**
   * Get the set of completed lesson indices for a module
   */
  private getCompletedLessons(moduleId: string): Set<number> {
    const rows = this.db.prepare(
      "SELECT lesson_idx FROM progress WHERE module_id = ?"
    ).all(moduleId) as Array<{ lesson_idx: number }>;
    return new Set(rows.map((r) => r.lesson_idx));
  }
}
