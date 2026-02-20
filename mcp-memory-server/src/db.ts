/**
 * Database initialization, schema, and prepared statement factory.
 *
 * The db instance is created once and accessed via getDb().
 * Prepared statements are created lazily via getStatements().
 */

import Database from "better-sqlite3";
import { existsSync, mkdirSync } from "fs";
import { dirname, join } from "path";
import { homedir } from "os";

// Paths -- platform-aware defaults, overridable via env vars
export const DB_PATH =
  process.env.MEMORY_DB_PATH || join(homedir(), ".claude", "memory.db");
export const SESSIONS_DIR =
  process.env.SESSIONS_DIR ||
  join(homedir(), ".claude", "sessions", "transcripts");

let db: Database.Database | null = null;

/** Get (or create) the singleton database instance. */
export function getDb(): Database.Database {
  if (db) return db;

  const dbDir = dirname(DB_PATH);
  if (!existsSync(dbDir)) {
    mkdirSync(dbDir, { recursive: true });
  }

  db = new Database(DB_PATH);
  initSchema(db);
  return db;
}

/** Close the database connection. */
export function closeDb(): void {
  if (db) {
    db.close();
    db = null;
  }
}

/** Initialize all tables, virtual tables, and indexes. */
function initSchema(database: Database.Database): void {
  database.exec(`
    CREATE TABLE IF NOT EXISTS decisions (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      key TEXT UNIQUE NOT NULL,
      decision TEXT NOT NULL,
      rationale TEXT,
      alternatives TEXT,
      created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
      updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
    );

    CREATE TABLE IF NOT EXISTS state (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      key TEXT UNIQUE NOT NULL,
      value TEXT NOT NULL,
      updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
    );

    CREATE TABLE IF NOT EXISTS learnings (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      topic TEXT NOT NULL,
      content TEXT NOT NULL,
      context TEXT,
      created_at DATETIME DEFAULT CURRENT_TIMESTAMP
    );

    CREATE TABLE IF NOT EXISTS code_locations (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      name TEXT NOT NULL,
      file_path TEXT NOT NULL,
      line_number INTEGER,
      purpose TEXT,
      created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
      updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
      UNIQUE(name, file_path)
    );

    CREATE TABLE IF NOT EXISTS session_index (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      session_file TEXT UNIQUE NOT NULL,
      session_date TEXT,
      message_count INTEGER,
      indexed_at DATETIME DEFAULT CURRENT_TIMESTAMP
    );

    CREATE TABLE IF NOT EXISTS session_messages (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      session_file TEXT NOT NULL,
      message_index INTEGER NOT NULL,
      role TEXT,
      content TEXT,
      UNIQUE(session_file, message_index)
    );

    CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
      source,
      key,
      content,
      tokenize='porter'
    );

    CREATE VIRTUAL TABLE IF NOT EXISTS session_fts USING fts5(
      session_file,
      content,
      tokenize='porter'
    );

    -- CADE Quality Metrics table
    CREATE TABLE IF NOT EXISTS quality_metrics (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
      metric_type TEXT NOT NULL,
      value REAL NOT NULL,
      context TEXT,
      task_id TEXT,
      session_id TEXT
    );

    CREATE INDEX IF NOT EXISTS idx_metrics_type ON quality_metrics(metric_type);
    CREATE INDEX IF NOT EXISTS idx_metrics_timestamp ON quality_metrics(timestamp);
  `);
}
