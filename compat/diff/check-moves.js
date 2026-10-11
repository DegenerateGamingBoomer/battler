'use strict';
// Compares battler's move data with the pinned Showdown's Champions dex for every move the
// regulation allows: power, accuracy, priority, type, category, target, secondary and self effects,
// drain, recoil, multi-hit and the flags that change mechanics. Prints one line per mismatch.
// Usage: node compat/diff/check-moves.js
const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '../..');
const SD = process.env.SHOWDOWN_DIR || path.join(ROOT, 'third_party/pokemon-showdown');
const {Dex} = require(path.join(SD, 'dist/sim'));
const dex = Dex.mod('champions');
const toID = s => String(s).toLowerCase().replace(/[^a-z0-9]/g, '');

function mergePatch(target, patch) {
  for (const [k, v] of Object.entries(patch)) {
    if (k.startsWith('+')) {
      target[k.slice(1)] = [...(target[k.slice(1)] || []), ...v];
      continue;
    }
    if (v === null) delete target[k];
    else if (v && typeof v === 'object' && !Array.isArray(v) && target[k] && typeof target[k] === 'object' && !Array.isArray(target[k])) mergePatch(target[k], v);
    else target[k] = v;
  }
  return target;
}

const moves = {};
const dir = path.join(ROOT, 'battle-data/data/moves');
for (const f of fs.readdirSync(dir)) Object.assign(moves, JSON.parse(fs.readFileSync(path.join(dir, f), 'utf8')));
const overlay = JSON.parse(fs.readFileSync(path.join(ROOT, 'battler-champions/data/mc/moves.json'), 'utf8'));
for (const [id, patch] of Object.entries(overlay)) if (moves[id]) mergePatch(moves[id], patch);
const legality = JSON.parse(fs.readFileSync(path.join(ROOT, 'battler-champions/data/mc/legality.json'), 'utf8'));

const TARGETS = {
  Normal: 'normal', User: 'self', AllAdjacentFoes: 'allAdjacentFoes', AdjacentFoe: 'adjacentFoe', Any: 'any',
  Field: 'all', AllAdjacent: 'allAdjacent', AllySide: 'allySide', RandomNormal: 'randomNormal', Allies: 'allies',
  FoeSide: 'foeSide', All: 'all', Scripted: 'scripted', AdjacentAlly: 'adjacentAlly', AllyTeam: 'allyTeam',
  AdjacentAllyOrUser: 'adjacentAllyOrSelf',
};
const FLAGS = {Contact: 'contact', Sound: 'sound', Punch: 'punch', Bite: 'bite', Pulse: 'pulse', Slicing: 'slicing',
  Wind: 'wind', Powder: 'powder', Bullet: 'bullet', Dance: 'dance', Protect: 'protect', Reflectable: 'reflectable',
  Heal: 'heal', BypassSubstitute: 'bypasssub', Thawing: 'defrost', Snatch: 'snatch', Mirror: 'mirror',
  Gravity: 'gravity', Charge: 'charge', Recharge: 'recharge', Distance: 'distance', FailEncore: 'failencore',
  NoSleepTalk: 'nosleeptalk', FailInstruct: 'failinstruct', FailCopycat: 'failcopycat', NoMetronome: 'metronome'};

const pct = s => (typeof s === 'string' && s.endsWith('%') ? Number(s.slice(0, -1)) : s === undefined ? 100 : s);
const frac = s => {
  if (s === undefined) return null;
  if (typeof s === 'number') return s;
  if (s.endsWith('%')) return Number(s.slice(0, -1)) / 100;
  const [a, b] = s.split('/').map(Number);
  return a / b;
};
const STAT = {acc: 'accuracy', eva: 'evasion'};
const sortObj = o => JSON.stringify(Object.keys(o || {}).map(k => [STAT[k] || k, o[k]]).sort()
  .reduce((r, [k, v]) => (v ? (r[k] = v, r) : r), {}));
function sdSecondary(s) {
  return JSON.stringify({chance: s.chance || 100, boosts: s.boosts ? sortObj(s.boosts) : null, status: s.status || null,
    volatile: s.volatileStatus || null, self: s.self && s.self.boosts ? sortObj(s.self.boosts) : null});
}
function btSecondary(s) {
  const t = s.target || {};
  return JSON.stringify({chance: pct(s.chance), boosts: t.boosts ? sortObj(t.boosts) : null, status: t.status || null,
    volatile: t.volatile_status || null, self: s.user && s.user.boosts ? sortObj(s.user.boosts) : null});
}

const out = [];
for (const id of legality.moves) {
  const sd = dex.moves.get(id);
  const bt = moves[id];
  if (!sd.exists) continue;
  if (!bt) { out.push(`${id}: missing from battler`); continue; }
  const diff = (what, a, b) => { if (JSON.stringify(a) !== JSON.stringify(b)) out.push(`${id}: ${what}: battler ${JSON.stringify(a)} showdown ${JSON.stringify(b)}`); };
  diff('category', bt.category, sd.category);
  diff('type', bt.primary_type, sd.type);
  diff('basePower', bt.base_power || 0, sd.basePower);
  diff('accuracy', typeof bt.accuracy === 'number' ? bt.accuracy : true, sd.accuracy);
  diff('priority', bt.priority || 0, sd.priority);
  diff('target', TARGETS[bt.target] || bt.target, sd.target);
  diff('pp', bt.pp, sd.pp);
  diff('critRatio', bt.crit_ratio || 1, sd.critRatio || 1);
  diff('multihit', bt.multihit ? (typeof bt.multihit === 'number' ? bt.multihit : bt.multihit) : null, sd.multihit || null);
  const drain = bt.drain_percent ? frac(bt.drain_percent) : null;
  diff('drain', drain, sd.drain ? sd.drain[0] / sd.drain[1] : null);
  const recoil = bt.recoil ? frac(bt.recoil.percent) : null;
  diff('recoil', recoil && Math.round(recoil * 1000) / 1000, sd.recoil ? Math.round(sd.recoil[0] / sd.recoil[1] * 1000) / 1000 : null);
  const sdSecs = (sd.secondaries || []).map(sdSecondary).sort();
  const btSecs = (bt.secondary_effects || []).map(btSecondary).sort();
  diff('secondaries', btSecs, sdSecs);
  const sdSelf = sd.self && sd.self.boosts ? sortObj(sd.self.boosts) : null;
  const btSelf = bt.user_effect && bt.user_effect.boosts ? sortObj(bt.user_effect.boosts) : null;
  diff('self boosts', btSelf, sdSelf);
  const sdBoosts = sd.boosts ? sortObj(sd.boosts) : null;
  const btBoosts = bt.hit_effect && bt.hit_effect.boosts ? sortObj(bt.hit_effect.boosts) : null;
  if (sd.category === 'Status') diff('boosts', btBoosts, sdBoosts);
  const sdStatus = sd.status || null;
  const btStatus = (bt.hit_effect && bt.hit_effect.status) || null;
  if (sd.category === 'Status') diff('status', btStatus, sdStatus);
  for (const [flag, sdFlag] of Object.entries(FLAGS)) {
    const a = (bt.flags || []).includes(flag);
    const b = sdFlag === 'metronome' ? !!sd.flags.metronome === false : !!sd.flags[sdFlag];
    if (a !== b) out.push(`${id}: flag ${flag}: battler ${a} showdown ${b}`);
  }
}
console.log(out.join('\n'));
console.error(`${out.length} mismatches over ${legality.moves.length} legal moves`);
