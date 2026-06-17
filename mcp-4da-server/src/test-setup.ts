// SPDX-License-Identifier: Apache-2.0
/**
 * Vitest setup: make config resolution HERMETIC.
 *
 * getEmbeddingConfig()/getLLMConfig() otherwise walk up to the operator's real
 * data/settings.json, so enabling a provider there would silently flip test
 * behaviour (e.g. agent_memory.recall going async). Point FOURDA_SETTINGS_PATH at
 * an empty settings file so tests depend only on the env they set themselves.
 * Tests that exercise the semantic path set FOURDA_EMBED_PROVIDER explicitly,
 * which takes precedence over the settings file.
 */
import { writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const emptySettings = join(tmpdir(), `4da-test-empty-settings-${process.pid}.json`);
writeFileSync(emptySettings, "{}");
process.env.FOURDA_SETTINGS_PATH = emptySettings;
