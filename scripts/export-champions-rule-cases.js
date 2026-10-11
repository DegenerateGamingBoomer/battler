'use strict';
// Exports Showdown (champions mod) stats, max PP and public health for the battler-champions tests.
// Usage: node scripts/export-champions-rule-cases.js [out]
const fs = require('fs');
const path = require('path');
const SD = process.env.SHOWDOWN_DIR || path.resolve(__dirname, '../third_party/pokemon-showdown');
const {Battle, Teams, Dex} = require(path.join(SD, 'dist/sim'));
const {PRNG} = require(path.join(SD, 'dist/sim/prng'));
const out = process.argv[2] || path.resolve(__dirname, '../battler-champions/tests/data/rule-cases.json');
const FORMAT = 'gen9championsvgc2026regmc';
const prng = new PRNG([7, 7, 7, 7]);

const c = Dex.mod('champions');
const natures = c.natures.all().map(n => n.name);
const species = c.species.all().filter(s => !s.isNonstandard && !s.battleOnly && s.baseSpecies === s.name && s.num > 0);

function battleFor(sets) {
  return new Battle({
    formatid: FORMAT, seed: [1, 2, 3, 4],
    p1: {name: 'p1', team: Teams.pack(sets)},
    p2: {name: 'p2', team: Teams.pack([{species: 'Pikachu', moves: ['splash'], ability: 'static', evs: {}, level: 50}])},
  });
}

// Stats: random legal-looking stat points and natures.
const stats = [];
for (let i = 0; i < 180; i++) {
  const sp = {hp: 0, atk: 0, def: 0, spa: 0, spd: 0, spe: 0};
  let left = 66;
  const order = Object.keys(sp);
  prng.shuffle(order);
  for (const k of order) {
    const v = Math.min(left, prng.random(33));
    sp[k] = v; left -= v;
  }
  const s = prng.sample(species);
  stats.push({species: s.name, nature: prng.sample(natures), evs: sp});
}
// Compute expected stats in chunks of 6 sets per battle.
for (let i = 0; i < stats.length; i += 6) {
  const chunk = stats.slice(i, i + 6);
  const sets = chunk.map(x => ({species: x.species, moves: ['splash'], ability: Object.values(c.species.get(x.species).abilities)[0], nature: x.nature, evs: x.evs, ivs: {hp: 31, atk: 31, def: 31, spa: 31, spd: 31, spe: 31}, level: 50}));
  const b = battleFor(sets);
  chunk.forEach((x, j) => {
    const p = b.sides[0].pokemon[j];
    x.expected = {hp: p.maxhp, atk: p.storedStats.atk, def: p.storedStats.def, spa: p.storedStats.spa, spd: p.storedStats.spd, spe: p.storedStats.spe};
    x.base = c.species.get(x.species).baseStats;
  });
}

// PP: max PP of every legal move as Showdown computes it for a Pokémon knowing it.
const pp = {};
const moves = c.moves.all().filter(m => !m.isNonstandard && !m.isZ && !m.isMax && m.id !== 'hiddenpower' && m.id !== 'struggle');
for (let i = 0; i < moves.length; i += 4) {
  const chunk = moves.slice(i, i + 4);
  const b = battleFor([{species: 'Pikachu', moves: chunk.map(m => m.id), ability: 'static', evs: {}, level: 50}]);
  b.sides[0].pokemon[0].moveSlots.forEach(slot => { pp[slot.id] = {name: c.moves.get(slot.id).name, base: c.moves.get(slot.id).pp, max: slot.maxpp}; });
}

// Public health.
const health = [];
const mon = battleFor([{species: 'Pikachu', moves: ['splash'], ability: 'static', evs: {}, level: 50}]).sides[0].pokemon[0];
for (const maxhp of [1, 5, 10, 20, 37, 100, 101, 143, 200, 215, 300, 359, 500]) {
  mon.maxhp = maxhp;
  const seen = new Set();
  const cands = new Set([1, 2, maxhp - 1, maxhp]);
  for (let h = 1; h <= maxhp; h++) if ([20, 50].includes(Math.floor(100 * h / maxhp)) || h % 7 === 0) cands.add(h);
  for (const hp of [...cands].filter(h => h >= 1 && h <= maxhp)) {
    mon.hp = hp;
    health.push({hp, maxhp, shared: mon.getHealth().shared});
  }
}
fs.writeFileSync(out, JSON.stringify({commit: require('child_process').execSync('git rev-parse HEAD', {cwd: SD}).toString().trim(), stats, pp, health}));
console.log('wrote', out, stats.length, Object.keys(pp).length, health.length);
