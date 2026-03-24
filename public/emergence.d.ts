/**
 * GAME Component: emergence
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
 * `<game-emergence>` Web Component
 *
 * A self-contained WebGPU/WebGL2 shader component.
 *
 * @example
 * ```html
 * <game-emergence metabolism="0.5" signal_count="0" awareness="0.3"></game-emergence>
 * ```
 *
 * @example
 * ```typescript
 * const el = document.querySelector('game-emergence')!;
   * el.metabolism = 0.5;
   * el.signal_count = 0;
 * ```
 */
interface GameEmergenceElement extends HTMLElement {
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
  /** Default: 0.5 */
  metabolism: number;
  /** Default: 0 */
  signal_count: number;
  /** Default: 0.3 */
  awareness: number;
}

declare global {
  interface HTMLElementTagNameMap {
    'game-emergence': GameEmergenceElement;
  }
}

export {};
