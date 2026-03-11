/**
 * GAME Component: crystal-vortex
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
 * `<game-crystal-vortex>` Web Component
 *
 * A self-contained WebGPU/WebGL2 shader component.
 *
 * @example
 * ```html
 * <game-crystal-vortex spin="0" refraction="0" depth="0"></game-crystal-vortex>
 * ```
 *
 * @example
 * ```typescript
 * const el = document.querySelector('game-crystal-vortex')!;
   * el.spin = 0;
   * el.refraction = 0;
 * ```
 */
interface GameCrystalVortexElement extends HTMLElement {
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
  spin: number;
  /** Default: 0 */
  refraction: number;
  /** Default: 0 */
  depth: number;
}

declare global {
  interface HTMLElementTagNameMap {
    'game-crystal-vortex': GameCrystalVortexElement;
  }
}

export {};
