/* eslint-disable no-undef */
/**
 * GameAudioBridge — FFT analysis + beat detection for GAME Web Components.
 *
 * Captures microphone input via Web Audio API, splits frequency data into
 * bass/mid/treble bands, derives energy and beat detection, and pushes
 * AudioAnalysis frames to subscribers at requestAnimationFrame rate.
 *
 * Privacy: microphone is only activated on explicit start() call, which
 * requires a user gesture. No audio data leaves the process.
 *
 * Usage:
 *   const bridge = getAudioBridge();
 *   await bridge.start();              // must be inside a click handler
 *   gameComponent.setAudioSource(bridge);
 *   // later:
 *   bridge.stop();
 */

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/** Normalized audio analysis frame pushed to subscribers each animation frame. */
export interface AudioAnalysis {
  /** Low-frequency energy (0-250 Hz), normalized 0-1. */
  readonly bass: number;
  /** Mid-frequency energy (250-4000 Hz), normalized 0-1. */
  readonly mid: number;
  /** High-frequency energy (4000 Hz+), normalized 0-1. */
  readonly treble: number;
  /** Overall spectral energy, normalized 0-1. */
  readonly energy: number;
  /** Beat pulse: spikes to 1.0 on transient, exponential decay per frame. */
  readonly beat: number;
}

/** Callback signature for audio analysis subscribers. */
export type AudioSubscriber = (analysis: AudioAnalysis) => void;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** FFT window size. 512 gives 256 frequency bins — sufficient resolution
 *  for band splitting without excessive per-frame cost. */
const FFT_SIZE = 512;

/** Number of frequency bins (FFT_SIZE / 2). */
const BIN_COUNT = FFT_SIZE / 2;

/** Sample rate assumed for bin-to-Hz mapping (Web Audio default). */
const SAMPLE_RATE = 48_000;

/** Hz per frequency bin: sampleRate / fftSize. */
const HZ_PER_BIN = SAMPLE_RATE / FFT_SIZE;

/**
 * Bin index boundaries derived from frequency cutoffs.
 * bass:   0 Hz  – 250 Hz   -> bins 0..(bassEnd-1)
 * mid:  250 Hz  – 4000 Hz  -> bins bassEnd..(midEnd-1)
 * treble: 4000 Hz+         -> bins midEnd..255
 */
const BASS_END = Math.ceil(250 / HZ_PER_BIN);
const MID_END = Math.ceil(4_000 / HZ_PER_BIN);

/** Energy derivative threshold that triggers a beat. */
const BEAT_THRESHOLD = 0.15;

/** Per-frame multiplicative decay applied to the beat value. */
const BEAT_DECAY = 0.9;

// ---------------------------------------------------------------------------
// Singleton
// ---------------------------------------------------------------------------

/**
 * Singleton bridge between a microphone MediaStream and GAME Web Components.
 *
 * The class owns the full Web Audio graph (MediaStreamSource -> AnalyserNode)
 * and drives analysis via requestAnimationFrame. Subscribers receive an
 * immutable AudioAnalysis snapshot each frame.
 */
export class GameAudioBridge {
  // -- Singleton --------------------------------------------------------
  private static instance: GameAudioBridge | undefined;

  /** Return the singleton instance, creating it on first access. */
  static get(): GameAudioBridge {
    if (!GameAudioBridge.instance) {
      GameAudioBridge.instance = new GameAudioBridge();
    }
    return GameAudioBridge.instance;
  }

  // -- Private state ----------------------------------------------------
  private context: AudioContext | null = null;
  private analyser: AnalyserNode | null = null;
  private source: MediaStreamAudioSourceNode | null = null;
  private stream: MediaStream | null = null;
  private freqData: Uint8Array = new Uint8Array(BIN_COUNT);
  private rafId: number | null = null;

  private subscribers: Set<AudioSubscriber> = new Set();

  /** Previous frame energy for derivative-based beat detection. */
  private prevEnergy = 0;
  /** Current beat value, decayed each frame. */
  private beatValue = 0;

  /** Prevent external instantiation. */
  private constructor() {}

  // -- Public API -------------------------------------------------------

  /**
   * Start capturing microphone audio and analysing it.
   *
   * Must be called from a user-gesture context (click / keydown) so that
   * the browser permits microphone access and AudioContext creation.
   *
   * @returns `true` if the pipeline started successfully, `false` on error.
   */
  async start(): Promise<boolean> {
    // Already running — no-op.
    if (this.rafId !== null) return true;

    try {
      this.stream = await navigator.mediaDevices.getUserMedia({ audio: true });

      this.context = new AudioContext();
      this.analyser = this.context.createAnalyser();
      this.analyser.fftSize = FFT_SIZE;
      this.analyser.smoothingTimeConstant = 0.8;

      this.source = this.context.createMediaStreamSource(this.stream);
      this.source.connect(this.analyser);

      // Allocate typed array to match analyser output length.
      this.freqData = new Uint8Array(this.analyser.frequencyBinCount);

      // Reset beat detection state.
      this.prevEnergy = 0;
      this.beatValue = 0;

      // Kick off the analysis loop.
      this.loop();

      return true;
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : String(err);
      console.warn('[GameAudioBridge] Failed to start:', message);
      this.cleanup();
      return false;
    }
  }

  /**
   * Stop capturing, disconnect the audio graph, and release the microphone.
   * Subscribers remain registered so a subsequent start() resumes delivery.
   */
  stop(): void {
    this.cleanup();
  }

  /**
   * Register a subscriber to receive AudioAnalysis frames.
   *
   * @param fn — callback invoked once per animation frame while active.
   * @returns An unsubscribe function. Call it to remove the listener.
   */
  subscribe(fn: AudioSubscriber): () => void {
    this.subscribers.add(fn);
    return () => {
      this.subscribers.delete(fn);
    };
  }

  /** Whether the bridge is currently capturing and analysing audio. */
  get active(): boolean {
    return this.rafId !== null;
  }

  // -- Private ----------------------------------------------------------

  /** requestAnimationFrame loop: sample analyser, compute bands, notify. */
  private loop = (): void => {
    if (!this.analyser) return;

    this.analyser.getByteFrequencyData(this.freqData);
    const analysis = this.analyze(this.freqData);
    this.notify(analysis);

    this.rafId = requestAnimationFrame(this.loop);
  };

  /**
   * Split frequency bins into bass / mid / treble bands, compute overall
   * energy, and run beat detection.
   */
  private analyze(freq: Uint8Array): AudioAnalysis {
    const len = freq.length;
    const bassEnd = Math.min(BASS_END, len);
    const midEnd = Math.min(MID_END, len);

    let bassSum = 0;
    let midSum = 0;
    let trebleSum = 0;
    let totalSum = 0;

    for (let i = 0; i < len; i++) {
      const v = freq[i];
      totalSum += v;
      if (i < bassEnd) {
        bassSum += v;
      } else if (i < midEnd) {
        midSum += v;
      } else {
        trebleSum += v;
      }
    }

    const bassCount = bassEnd;
    const midCount = midEnd - bassEnd;
    const trebleCount = len - midEnd;

    // Normalize each band average from 0-255 to 0-1.
    const bass = bassCount > 0 ? bassSum / (bassCount * 255) : 0;
    const mid = midCount > 0 ? midSum / (midCount * 255) : 0;
    const treble = trebleCount > 0 ? trebleSum / (trebleCount * 255) : 0;
    const energy = len > 0 ? totalSum / (len * 255) : 0;

    // Beat detection: positive energy derivative exceeding threshold.
    const delta = energy - this.prevEnergy;
    this.prevEnergy = energy;

    if (delta > BEAT_THRESHOLD) {
      this.beatValue = 1.0;
    } else {
      this.beatValue *= BEAT_DECAY;
    }

    // Clamp tiny residuals to zero to avoid floating-point dust.
    const beat = this.beatValue < 0.001 ? 0 : this.beatValue;

    return { bass, mid, treble, energy, beat };
  }

  /** Push the latest analysis to all subscribers. */
  private notify(analysis: AudioAnalysis): void {
    for (const fn of this.subscribers) {
      try {
        fn(analysis);
      } catch (err: unknown) {
        const message = err instanceof Error ? err.message : String(err);
        console.warn('[GameAudioBridge] Subscriber error:', message);
      }
    }
  }

  /** Tear down the audio graph and cancel the animation frame. */
  private cleanup(): void {
    if (this.rafId !== null) {
      cancelAnimationFrame(this.rafId);
      this.rafId = null;
    }

    this.source?.disconnect();
    this.source = null;

    this.analyser?.disconnect();
    this.analyser = null;

    // Stop all tracks to release the microphone indicator.
    if (this.stream) {
      for (const track of this.stream.getTracks()) {
        track.stop();
      }
      this.stream = null;
    }

    if (this.context) {
      // close() returns a promise but we intentionally fire-and-forget here
      // since cleanup should not block the caller.
      void this.context.close();
      this.context = null;
    }

    this.prevEnergy = 0;
    this.beatValue = 0;
  }
}

// ---------------------------------------------------------------------------
// Convenience accessor
// ---------------------------------------------------------------------------

/** Return the GameAudioBridge singleton. */
export const getAudioBridge = (): GameAudioBridge => GameAudioBridge.get();
