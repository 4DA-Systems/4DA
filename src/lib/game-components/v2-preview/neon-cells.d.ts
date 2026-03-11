/**
 * GAME Component: neon-cells
 * Auto-generated TypeScript definitions — do not edit.
 */

/** Audio data for reactive components. */
interface GameAudioData {
  bass: number;
  mid: number;
  treble: number;
  energy: number;
  beat: number;
}

/** Audio bridge for subscribable audio sources. */
interface GameAudioBridge {
  subscribe(callback: (data: GameAudioData) => void): void;
}

/**
 * `<game-neon-cells>` Web Component
 *
 * A self-contained WebGPU/WebGL2 shader component.
 *
 * @example
 * ```html
 * <game-neon-cells pulse="0" division="0" glow_energy="0"></game-neon-cells>
 * ```
 *
 * @example
 * ```typescript
 * const el = document.querySelector('game-neon-cells')!;
   * el.pulse = 0;
   * el.division = 0;
 * ```
 */
interface GameNeonCellsElement extends HTMLElement {
  /** Set a uniform parameter by name. */
  setParam(name: string, value: number): void;

  /** Feed audio frequency data for reactive components. */
  setAudioData(data: GameAudioData): void;

  /** Connect an audio bridge for automatic audio feeding. */
  setAudioSource(bridge: GameAudioBridge): void;

  /** Capture the current frame as ImageData. */
  getFrame(): ImageData | null;

  /** Capture the current frame as a data URL (PNG). */
  getFrameDataURL(type?: string): string | null;

  // Uniform properties
  /** Default: 0 */
  pulse: number;
  /** Default: 0 */
  division: number;
  /** Default: 0 */
  glow_energy: number;
}

declare global {
  interface HTMLElementTagNameMap {
    'game-neon-cells': GameNeonCellsElement;
  }
}

export {};
