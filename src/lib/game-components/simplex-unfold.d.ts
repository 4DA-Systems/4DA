/**
 * GAME Component: simplex-unfold
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

interface GameSimplexUnfoldElement extends HTMLElement {
  setParam(name: string, value: number): void;
  setAudioData(data: GameAudioData): void;
  setAudioSource(bridge: GameAudioBridge): void;
  getFrame(): ImageData | null;
  getFrameDataURL(type?: string): string | null;
  /** Default: 0.0 */
  phase_offset: number;
  /** Default: 0.12 */
  auto_speed: number;
  /** Default: 1.0 */
  glow_intensity: number;
}

declare global {
  interface HTMLElementTagNameMap {
    'fourda-simplex-unfold': GameSimplexUnfoldElement;
  }
}

export {};
