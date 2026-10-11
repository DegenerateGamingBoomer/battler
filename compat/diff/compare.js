'use strict';
// Comparison helpers for the differential harness: draw logs, JSON state, protocol lines.

// Categories that are logged but not compared: `display-target` is the printed target of spread and
// side moves (allowlist R1); `speed-tie` draws only happen around ties, which end the comparison
// when they are between different owners.
const UNCOMPARED = new Set(['display-target', 'speed-tie']);

function gcd(a, b) {
  while (b) [a, b] = [b, a % b];
  return a;
}

// A chance draw is compared as a reduced fraction: floor(x * q) < p only depends on p / q for an
// integer p, so 3/10 and 30/100 are the same draw.
function drawKey(d) {
  if (d.k !== 'chance') return `int ${d.n}`;
  if (!Number.isInteger(d.p)) return `chance ${d.p}/${d.n}`;
  const g = gcd(d.p, d.n);
  return `chance ${d.p / g}/${d.n / g}`;
}

function byCategory(draws) {
  const out = new Map();
  for (const d of draws) {
    if (UNCOMPARED.has(d.c)) continue;
    if (!out.has(d.c)) out.set(d.c, []);
    out.get(d.c).push(drawKey(d));
  }
  return out;
}

/** Per-category differences between two draw logs, as readable strings. */
function diffDraws(showdown, battler) {
  const a = byCategory(showdown);
  const b = byCategory(battler);
  const out = [];
  for (const c of new Set([...a.keys(), ...b.keys()])) {
    const x = a.get(c) || [];
    const y = b.get(c) || [];
    if (x.join('|') !== y.join('|')) out.push({category: c, showdown: x, battler: y});
  }
  return out;
}

/** Paths at which two JSON values differ. */
function diffJson(a, b, path = '', out = []) {
  if (a && b && typeof a === 'object' && typeof b === 'object' && Array.isArray(a) === Array.isArray(b)) {
    if (Array.isArray(a) && a.length !== b.length) {
      out.push({path: `${path}.length`, showdown: a.length, battler: b.length});
    }
    const keys = Array.isArray(a) ? [...Array(Math.min(a.length, b.length)).keys()] : new Set([...Object.keys(a), ...Object.keys(b)]);
    for (const k of keys) diffJson(a[k], b[k], `${path}.${k}`, out);
    return out;
  }
  const norm = v => (v === undefined ? null : v);
  if (JSON.stringify(norm(a)) !== JSON.stringify(norm(b))) out.push({path, showdown: norm(a), battler: norm(b)});
  return out;
}

// Same rules as battler-showdown-compat/tests/common/mod.rs `apply_allowlist` (compat/allowlist.md).
const SIDE_TARGETING = ['Spikes', 'Stealth Rock', 'Toxic Spikes', 'Sticky Web'];
function applyAllowlist(lines) {
  return lines.filter(line => !line.startsWith('|-hint|')).map(line => {
    const parts = line.split('|');
    if (parts[1] === 'move' && parts.length > 4 &&
      (line.includes('|[spread] ') || line.endsWith('|[notarget]') || SIDE_TARGETING.includes(parts[3]))) {
      parts[4] = '*';
      return parts.join('|');
    }
    return line;
  });
}

// Lines without battle semantics, dropped from Showdown's log as in compat/gen-fixtures.js.
const META = /^\|(t:|gametype|player|gen|tier|rule|clearpoke|poke|teampreview|uhtml|uhtmlchange|request|inactive|rated)(\||$)/;
function normalizeLog(log) {
  const out = [];
  for (let i = 0; i < log.length; i++) {
    const line = log[i];
    if (line === '|' || line === '') continue;
    if (line.startsWith('|split|') && META.test(log[i + 1])) { i += 2; continue; }
    if (META.test(line)) continue;
    out.push(line.replace(/\n+$/, ''));
  }
  return out;
}

/** The first differing protocol line, or null. */
function diffProtocol(showdown, battler) {
  const a = applyAllowlist(showdown);
  const b = applyAllowlist(battler);
  const n = Math.max(a.length, b.length);
  for (let i = 0; i < n; i++) {
    if (a[i] !== b[i]) {
      return {index: i, showdown: a[i] === undefined ? null : a[i], battler: b[i] === undefined ? null : b[i],
        context: a.slice(Math.max(0, i - 3), i)};
    }
  }
  return null;
}

/** A coarse key for grouping protocol lines: line type plus non-identity arguments. */
function lineShape(line) {
  if (line === null) return '<none>';
  return line.split('|').map((part, i) => {
    if (i <= 1) return part;
    if (/^p\d[a-z]?: /.test(part)) return 'MON';
    if (/^\d+\/\d+/.test(part) || /^\d+( \w+)?$/.test(part)) return 'N';
    return part;
  }).join('|');
}

module.exports = {diffDraws, diffJson, diffProtocol, applyAllowlist, normalizeLog, lineShape, UNCOMPARED};
