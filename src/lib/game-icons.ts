/** Achievement icon map — shared between celebration overlay and achievements panel.
 *  Keys must match the `icon` field from the Rust game_engine achievement definitions. */
export const GAME_ICON_MAP: Record<string, string> = {
  // Rust game_engine icons
  telescope: '\uD83D\uDD2D',   // First Light
  satellite: '\uD83D\uDEF0\uFE0F', // Radar Operator
  lightbulb: '\uD83D\uDCA1',   // Eureka
  puzzle: '\uD83E\uDDE9',      // Context Builder
  newspaper: '\uD83D\uDCF0',   // Briefed
  antenna: '\uD83D\uDCE1',     // Multi-Source (same emoji as radar — they're related)
  radar: '\uD83D\uDCE1',       // Signal Hunter
  eye: '\uD83D\uDC41',         // Pattern Spotter
  brain: '\uD83E\uDDE0',       // Intelligence Analyst
  bookmark: '\uD83D\uDD16',    // Collector
  globe: '\uD83C\uDF10',       // Intel Network
  flame: '\uD83D\uDD25',       // Consistent (3-day streak)
  fire: '\uD83D\uDD25',        // Dedicated (7-day streak)
  // Legacy / generic
  sun: '\u2600',
  sparkle: '\u2728',
  gem: '\uD83D\uDC8E',
  archive: '\uD83D\uDCE6',
  scroll: '\uD83D\uDCDC',
  crown: '\uD83D\uDC51',
};

export function getGameIcon(key: string): string {
  return GAME_ICON_MAP[key] || '\u2B50';
}
