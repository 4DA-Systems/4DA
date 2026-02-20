/**
 * Session transcript parsing and indexing.
 *
 * Reads JSONL transcript files, parses messages, and stores them
 * in the database for full-text search.
 */

import type Database from "better-sqlite3";
import { existsSync, readFileSync } from "fs";
import { join } from "path";

/** Shape of a single line in a JSONL transcript file. */
interface TranscriptMessage {
  type: string;
  role?: string;
  content?: string | Array<{ type: string; text?: string }>;
  message?: {
    role?: string;
    content?: string | Array<{ type: string; text?: string }>;
  };
}

/** Parsed message with normalized role and text content. */
export interface ParsedMessage {
  role: string;
  content: string;
}

/** Extract text from a content field that may be string or structured array. */
function extractText(
  content: string | Array<{ type: string; text?: string }> | undefined
): string {
  if (typeof content === "string") return content;
  if (Array.isArray(content)) {
    return content
      .filter((c) => c.type === "text")
      .map((c) => c.text || "")
      .join("\n");
  }
  return "";
}

/** Parse a JSONL transcript file into an array of messages. */
export function parseTranscript(filePath: string): ParsedMessage[] {
  const messages: ParsedMessage[] = [];

  try {
    const content = readFileSync(filePath, "utf-8");
    const lines = content.split("\n").filter((line) => line.trim());

    for (const line of lines) {
      try {
        const entry = JSON.parse(line) as TranscriptMessage;

        let role = "";
        let text = "";

        if (entry.type === "human" || entry.role === "user") {
          role = "user";
          text = extractText(entry.content);
        } else if (entry.type === "assistant" || entry.role === "assistant") {
          role = "assistant";
          text = extractText(entry.content);
        } else if (entry.message) {
          role = entry.message.role || "unknown";
          text = extractText(entry.message.content);
        }

        if (role && text) {
          messages.push({ role, content: text });
        }
      } catch {
        // Skip malformed lines
      }
    }
  } catch {
    // File read error
  }

  return messages;
}

/** Index a single session transcript into the database. */
export function indexSession(
  db: Database.Database,
  sessionsDir: string,
  sessionFile: string
): { indexed: boolean; messageCount: number } {
  const filePath = join(sessionsDir, sessionFile);

  if (!existsSync(filePath)) {
    return { indexed: false, messageCount: 0 };
  }

  const messages = parseTranscript(filePath);

  // Extract date from filename (session_YYYYMMDD_HHMMSS_id.jsonl)
  const dateMatch = sessionFile.match(/(\d{8})_(\d{6})/);
  const sessionDate = dateMatch
    ? `${dateMatch[1].slice(0, 4)}-${dateMatch[1].slice(4, 6)}-${dateMatch[1].slice(6, 8)} ` +
      `${dateMatch[2].slice(0, 2)}:${dateMatch[2].slice(2, 4)}:${dateMatch[2].slice(4, 6)}`
    : "unknown";

  // Prepared statements for session indexing
  const insertSessionIndex = db.prepare(
    `INSERT OR REPLACE INTO session_index (session_file, session_date, message_count)
     VALUES (?, ?, ?)`
  );
  const insertSessionMessage = db.prepare(
    `INSERT OR REPLACE INTO session_messages (session_file, message_index, role, content)
     VALUES (?, ?, ?, ?)`
  );
  const insertSessionFts = db.prepare(
    `INSERT INTO session_fts (session_file, content)
     VALUES (?, ?)`
  );

  insertSessionIndex.run(sessionFile, sessionDate, messages.length);

  const insertMany = db.transaction((msgs: ParsedMessage[]) => {
    for (let i = 0; i < msgs.length; i++) {
      insertSessionMessage.run(sessionFile, i, msgs[i].role, msgs[i].content);

      if (msgs[i].role === "user" || msgs[i].role === "assistant") {
        try {
          insertSessionFts.run(sessionFile, msgs[i].content);
        } catch {
          // Ignore FTS errors (duplicate entries)
        }
      }
    }
  });

  insertMany(messages);

  return { indexed: true, messageCount: messages.length };
}
