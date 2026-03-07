class GameBreedMerger {
  constructor() { this.name = 'child'; this.parents = ['fire', 'ice']; }

  merge(parentA, parentB) {
    const result = {};
    // inherit layers: mix(0.6)
    for (const k of Object.keys(parentA.layers || {})) {
      const a = parentA.layers[k] || 0;
      const b = parentB.layers[k] || 0;
      result[k] = a * 0.6 + b * 0.4;
    }
    // inherit params: pick(0.5)
    for (const k of Object.keys(parentA.params || {})) {
      result[k] = Math.random() < 0.5 ? parentA.params[k] : parentB.params[k];
    }
    // mutate scale: +/-0.3
    if (result['scale'] !== undefined) result['scale'] += (Math.random() * 2 - 1) * 0.3;
    // mutate speed: +/-0.1
    if (result['speed'] !== undefined) result['speed'] += (Math.random() * 2 - 1) * 0.1;
    return result;
  }
}
