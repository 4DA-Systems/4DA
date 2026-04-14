/**
 * GAME Component: notif-card-high
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
 * `<fourda-notif-card-high>` Web Component
 *
 * A self-contained WebGPU/WebGL2 shader component.
 *
 * @example
 * ```html
 * <fourda-notif-card-high intensity="1" hover="0"></fourda-notif-card-high>
 * ```
 *
 * @example
 * ```typescript
 * const el = document.querySelector('fourda-notif-card-high')!;
   * el.intensity = 1;
   * el.hover = 0;
 * ```
 */
interface GameNotifCardHighElement extends HTMLElement {
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
  /** Default: 1 */
  intensity: number;
  /** Default: 0 */
  hover: number;
  /** Convenience alias for intensity. */
  health: number;
}

declare global {
  interface HTMLElementTagNameMap {
    'fourda-notif-card-high': GameNotifCardHighElement;
  }
}

export {};
