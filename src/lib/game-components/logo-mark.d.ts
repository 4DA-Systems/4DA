/**
 * GAME Component: logo-mark
 * 4DA brand mark — pentachoron at logo grade.
 */

interface GameAudioData {
  bass: number;
  mid: number;
  treble: number;
  energy: number;
  beat: number;
}

interface GameAudioBridge {
  subscribe(callback: (data: GameAudioData) => void): void;
}

interface GameLogoMarkElement extends HTMLElement {
  setParam(name: string, value: number): void;
  setAudioData(data: GameAudioData): void;
  setAudioSource(bridge: GameAudioBridge): void;
  /** Default: 0.08 */
  rotation_speed: number;
  /** Default: 1.3 */
  glow_intensity: number;
  /** Default: 0.05 */
  w_rotation: number;
}

declare global {
  interface HTMLElementTagNameMap {
    'game-logo-mark': GameLogoMarkElement;
  }
}

export {};
