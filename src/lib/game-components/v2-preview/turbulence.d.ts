/**
 * GAME Component: turbulence
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
 * `<game-turbulence>` Web Component
 *
 * A self-contained WebGPU/WebGL2 shader component.
 *
 * @example
 * ```html
 * <game-turbulence storm="0" fury="0" lightning="0"></game-turbulence>
 * ```
 *
 * @example
 * ```typescript
 * const el = document.querySelector('game-turbulence')!;
   * el.storm = 0;
   * el.fury = 0;
 * ```
 */
interface GameTurbulenceElement extends HTMLElement {
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
  storm: number;
  /** Default: 0 */
  fury: number;
  /** Default: 0 */
  lightning: number;
}

declare global {
  interface HTMLElementTagNameMap {
    'game-turbulence': GameTurbulenceElement;
  }
}

export {};
