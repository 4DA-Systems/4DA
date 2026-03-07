// GAME Scene Component: day-cycle — auto-generated, do not edit.
(function(){

class GameSceneTimeline_day_cycle {
  constructor() {
    this._startTime = null;
    this._entries = [
      { type: 'play', cinematic: 'dawn', duration: 8 },
      { type: 'transition', kind: 'dissolve', duration: 3 },
      { type: 'play', cinematic: 'noon', duration: 8 },
      { type: 'transition', kind: 'fade', duration: 3 },
      { type: 'play', cinematic: 'dusk', duration: 8 },
    ];
    this._totalDuration = this._entries.reduce((s, e) => s + e.duration, 0);
  }

  evaluate(elapsedSec) {
    if (this._startTime === null) this._startTime = elapsedSec;
    const t = elapsedSec - this._startTime;
    let acc = 0;
    let lastCinematic = null;

    for (const e of this._entries) {
      if (t < acc + e.duration) {
        const progress = (t - acc) / e.duration;
        if (e.type === 'play') {
          return { current: e.cinematic, next: null, blend: 0, kind: null };
        } else {
          // transition: find next play entry
          const idx = this._entries.indexOf(e);
          const nextEntry = this._entries.slice(idx + 1).find(x => x.type === 'play');
          return {
            current: lastCinematic,
            next: nextEntry ? nextEntry.cinematic : null,
            blend: progress,
            kind: e.kind
          };
        }
      }
      if (e.type === 'play') lastCinematic = e.cinematic;
      acc += e.duration;
    }

    return { current: lastCinematic, next: null, blend: 0, kind: null };
  }

  isComplete(elapsedSec) {
    if (this._startTime === null) return false;
    return (elapsedSec - this._startTime) >= this._totalDuration;
  }

  progress(elapsedSec) {
    if (this._startTime === null) return 0;
    return Math.min((elapsedSec - this._startTime) / this._totalDuration, 1.0);
  }

  reset() { this._startTime = null; }
}

const SCENE_CINEMATICS = ['dawn', 'noon', 'dusk'];

class GameSceneDayCycle extends HTMLElement {
  constructor() {
    super();
    this.attachShadow({ mode: 'open' });
    this._timeline = new GameSceneTimeline_day_cycle();
    this._children = {};
    this._animFrame = null;
    this._startTime = null;
  }

  connectedCallback() {
    const style = document.createElement('style');
    style.textContent = `
      :host { display: block; width: 100%; height: 100%; position: relative; overflow: hidden; }
      .scene-layer { position: absolute; top: 0; left: 0; width: 100%; height: 100%; opacity: 0; }
    `;
    this.shadowRoot.appendChild(style);

    for (const tag of SCENE_CINEMATICS) {
      const el = document.createElement('game-' + tag);
      el.classList.add('scene-layer');
      this.shadowRoot.appendChild(el);
      this._children[tag] = el;
    }

    this._startTime = performance.now() / 1000;
    this._tick();
  }

  disconnectedCallback() {
    if (this._animFrame) cancelAnimationFrame(this._animFrame);
  }

  _tick() {
    const elapsed = performance.now() / 1000 - this._startTime;
    const state = this._timeline.evaluate(elapsed);

    const toTag = n => n ? n.replace(/[^a-zA-Z0-9]/g, '-').toLowerCase() : null;
    const curTag = toTag(state.current);
    const nextTag = toTag(state.next);

    for (const [tag, el] of Object.entries(this._children)) {
      if (tag === curTag && !nextTag) {
        el.style.opacity = '1';
      } else if (tag === curTag && nextTag) {
        el.style.opacity = String(1 - state.blend);
      } else if (tag === nextTag) {
        el.style.opacity = String(state.blend);
      } else {
        el.style.opacity = '0';
      }
    }

    if (!this._timeline.isComplete(elapsed)) {
      this._animFrame = requestAnimationFrame(() => this._tick());
    }
  }

  reset() { this._timeline.reset(); this._startTime = performance.now() / 1000; this._tick(); }
  get progress() { return this._timeline.progress(performance.now() / 1000 - this._startTime); }
}

customElements.define('game-scene-day-cycle', GameSceneDayCycle);
})();
