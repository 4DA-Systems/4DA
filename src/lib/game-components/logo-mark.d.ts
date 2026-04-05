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
  /** Default: 0.10 */
  rotation_speed: number;
  /** Default: 1.2 */
  glow_intensity: number;
}

declare global {
  interface HTMLElementTagNameMap {
    'game-logo-mark': GameLogoMarkElement;
  }
}

export {};
