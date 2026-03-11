/**
 * GAME Component: organic-flow
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
 * `<game-organic-flow>` Web Component
 *
 * A self-contained WebGPU/WebGL2 shader component.
 *
 * @example
 * ```html
 * <game-organic-flow current="0" luminance="0" depth="0"></game-organic-flow>
 * ```
 *
 * @example
 * ```typescript
 * const el = document.querySelector('game-organic-flow')!;
   * el.current = 0;
   * el.luminance = 0;
 * ```
 */
interface GameOrganicFlowElement extends HTMLElement {
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
  current: number;
  /** Default: 0 */
  luminance: number;
  /** Default: 0 */
  depth: number;
}

declare global {
  interface HTMLElementTagNameMap {
    'game-organic-flow': GameOrganicFlowElement;
  }
}

export {};
