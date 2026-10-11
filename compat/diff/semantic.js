'use strict';
// Semantic random streams (spec P1c), mirrored exactly by battler-diff-runner/src/semantic.rs.
//
// Every draw category has its own stream of 32-bit values, seeded from the battle seed and the
// category name. A value x maps to int(n) = floor(x * n / 2^32) and chance(p / q) = int(q) < p.
// Certain outcomes (n <= 1, p <= 0, p >= q) draw nothing.

function fnv1a(text) {
  let hash = 0x811c9dc5;
  for (const byte of Buffer.from(text, 'utf8')) {
    hash ^= byte;
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash >>> 0;
}

// Mulberry32.
class Stream {
  constructor(seed, category) {
    this.state = fnv1a(`${seed}:${category}`) | 0;
  }
  next() {
    this.state = (this.state + 0x6D2B79F5) | 0;
    const a = this.state;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = ((t + Math.imul(t ^ (t >>> 7), 61 | t)) | 0) ^ t;
    return (t ^ (t >>> 14)) >>> 0;
  }
}

class SemanticRandom {
  constructor(seed) {
    this.seed = seed >>> 0;
    this.streams = new Map();
    this.draws = [];
    this.ties = [];
  }
  value(category) {
    let stream = this.streams.get(category);
    if (!stream) {
      stream = new Stream(this.seed, category);
      this.streams.set(category, stream);
    }
    return stream.next();
  }
  intValue(category, n) {
    // x < 2^32 and n < 2^21, so the product is exact in a double.
    return Math.floor(this.value(category) * n / 4294967296);
  }
  // Showdown's randomChance(p, q) is random(q) < p, which may have a fractional p.
  chance(category, numerator, denominator, site) {
    if (numerator <= 0) return false;
    if (numerator >= denominator) return true;
    const value = this.intValue(category, denominator);
    const result = value < numerator;
    this.draws.push({c: category, k: 'chance', n: denominator, p: numerator, r: result ? 1 : 0, site});
    return result;
  }
  int(category, n, site) {
    n = Math.floor(n);
    if (n <= 1) return 0;
    const value = this.intValue(category, n);
    this.draws.push({c: category, k: 'int', n, r: value, site});
    return value;
  }
  tie(owners) {
    const distinct = owners.some(o => o === null || o !== owners[0]);
    this.ties.push({owners, distinct});
  }
  take() {
    const out = {draws: this.draws, ties: this.ties};
    this.draws = [];
    this.ties = [];
    return out;
  }
}

module.exports = {fnv1a, Stream, SemanticRandom};
