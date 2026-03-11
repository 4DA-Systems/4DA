/**
 * GAME Component: glowing-cross
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
 * `<game-glowing-cross>` Web Component
 *
 * A self-contained WebGPU/WebGL2 shader component.
 *
 * @example
 * ```html
 * <game-glowing-cross power="0" breath="0" sanctity="0"></game-glowing-cross>
 * ```
 *
 * @example
 * ```typescript
 * const el = document.querySelector('game-glowing-cross')!;
   * el.power = 0;
   * el.breath = 0;
 * ```
 */
interface GameGlowingCrossElement extends HTMLElement {
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
  power: number;
  /** Default: 0 */
  breath: number;
  /** Default: 0 */
  sanctity: number;
}

declare global {
  interface HTMLElementTagNameMap {
    'game-glowing-cross': GameGlowingCrossElement;
  }
}

export {};
