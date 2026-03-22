/**
 * GAME Component: notif-high
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
 * `<game-notif-high>` Web Component
 *
 * A self-contained WebGPU/WebGL2 shader component.
 *
 * @example
 * ```html
 * <game-notif-high intensity="0" hover="0"></game-notif-high>
 * ```
 *
 * @example
 * ```typescript
 * const el = document.querySelector('game-notif-high')!;
   * el.intensity = 0;
   * el.hover = 0;
 * ```
 */
interface GameNotifHighElement extends HTMLElement {
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
  intensity: number;
  /** Default: 0 */
  hover: number;
  /** Convenience alias for intensity. */
  health: number;
}

declare global {
  interface HTMLElementTagNameMap {
    'game-notif-high': GameNotifHighElement;
  }
}

export {};
