/**
 * GAME Component: momentum-field
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
 * `<fourda-momentum-field>` Web Component
 *
 * A self-contained WebGPU/WebGL2 shader component.
 *
 * @example
 * ```html
 * <fourda-momentum-field trend_warm_r="0" trend_warm_g="0" advantage="0.5"></fourda-momentum-field>
 * ```
 *
 * @example
 * ```typescript
 * const el = document.querySelector('fourda-momentum-field')!;
   * el.trend_warm_r = 0;
   * el.trend_warm_g = 0;
 * ```
 */
interface GameMomentumFieldElement extends HTMLElement {
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
  trend_warm_r: number;
  /** Default: 0 */
  trend_warm_g: number;
  /** Default: 0.5 */
  advantage: number;
  /** Default: 0.5 */
  metabolism: number;
  /** Default: 0 */
  urgency: number;
  /** Default: 0.7 */
  confidence: number;
}

declare global {
  interface HTMLElementTagNameMap {
    'fourda-momentum-field': GameMomentumFieldElement;
  }
}

export {};
