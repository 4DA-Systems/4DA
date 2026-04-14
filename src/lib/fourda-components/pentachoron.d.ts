/**
 * GAME Component: pentachoron
 * Auto-generated TypeScript definitions — do not edit.
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

interface GamePentachoronElement extends HTMLElement {
  setParam(name: string, value: number): void;
  setAudioData(data: GameAudioData): void;
  setAudioSource(bridge: GameAudioBridge): void;
  getFrame(): ImageData | null;
  getFrameDataURL(type?: string): string | null;
  /** Default: 0.3 */
  rotation_speed: number;
  /** Default: 1.0 */
  glow_intensity: number;
  /** Default: 0.2 */
  w_rotation: number;
}

declare global {
  interface HTMLElementTagNameMap {
    'fourda-pentachoron': GamePentachoronElement;
  }
}

export {};
