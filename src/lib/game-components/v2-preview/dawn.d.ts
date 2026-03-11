/**
 * GAME Component: dawn
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
 * `<game-dawn>` Web Component
 *
 * A self-contained WebGPU/WebGL2 shader component.
 *
 * @example
 * ```html
 * <game-dawn emergence="0" warmth="0" sky_glow="0"></game-dawn>
 * ```
 *
 * @example
 * ```typescript
 * const el = document.querySelector('game-dawn')!;
   * el.emergence = 0;
   * el.warmth = 0;
 * ```
 */
interface GameDawnElement extends HTMLElement {
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
  emergence: number;
  /** Default: 0 */
  warmth: number;
  /** Default: 0 */
  sky_glow: number;
}

declare global {
  interface HTMLElementTagNameMap {
    'game-dawn': GameDawnElement;
  }
}

export {};
