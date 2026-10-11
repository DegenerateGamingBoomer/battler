'use strict';
// Exports Showdown's effective Champions-mod (Reg M-C) move/ability/item values and the delta from
// mainline gen9, as the ground truth for the battler-champions conformance tests.
// Usage: node scripts/export-champions-expected.js [out]
const fs = require('fs');
const path = require('path');
const SD = process.env.SHOWDOWN_DIR || path.resolve(__dirname, '../third_party/pokemon-showdown');
const {Dex} = require(path.join(SD, 'dist/sim'));
const out = process.argv[2] || path.resolve(__dirname, '../battler-champions/tests/data/showdown-champions.json');
const c = Dex.mod('champions'), g = Dex.mod('gen9');

const fn = o => JSON.stringify(o, (k, v) => typeof v === 'function' ? '[fn]' + v.toString() : v);
const flags = m => Object.keys(m.flags).filter(f => m.flags[f]).sort();
const sec = s => s ? {
  chance: s.chance ?? null, boosts: s.boosts ?? null, volatileStatus: s.volatileStatus ?? null,
  status: s.status ?? null, hasOnHit: !!s.onHit,
} : null;

const moves = {};
for (const m of c.moves.all()) {
  const b = g.moves.get(m.id);
  // Compare everything the data tables can express, including callback source.
  const changed = [];
  for (const k of new Set([...Object.keys(m), ...Object.keys(b)])) {
    if (['num', 'gen', 'effectType', 'exists', 'fullname', 'zMove', 'maxMove'].includes(k)) continue;
    if (fn(m[k]) !== fn(b[k])) changed.push(k);
  }
  moves[m.id] = {
    name: m.name, basePower: m.basePower, accuracy: m.accuracy, pp: m.pp, type: m.type, category: m.category,
    target: m.target, priority: m.priority, flags: flags(m),
    secondaries: (m.secondaries || []).map(sec), selfBoosts: m.self?.boosts ?? null, boosts: m.boosts ?? null,
    flagsAdded: flags(m).filter(f => !flags(b).includes(f)), flagsRemoved: flags(b).filter(f => !flags(m).includes(f)),
    legal: !m.isNonstandard, changed,
  };
}
const abilities = {};
for (const a of c.abilities.all()) {
  const b = g.abilities.get(a.id);
  const changed = [];
  for (const k of new Set([...Object.keys(a), ...Object.keys(b)])) {
    if (['num', 'gen', 'effectType', 'exists', 'fullname'].includes(k)) continue;
    if (fn(a[k]) !== fn(b[k])) changed.push(k);
  }
  abilities[a.id] = {name: a.name, legal: !a.isNonstandard, changed};
}
const items = {};
for (const i of c.items.all()) {
  const b = g.items.get(i.id);
  const changed = [];
  for (const k of new Set([...Object.keys(i), ...Object.keys(b)])) {
    if (['num', 'gen', 'effectType', 'exists', 'fullname'].includes(k)) continue;
    if (fn(i[k]) !== fn(b[k])) changed.push(k);
  }
  items[i.id] = {name: i.name, legal: !i.isNonstandard, changed};
}
fs.mkdirSync(path.dirname(out), {recursive: true});
fs.writeFileSync(out, JSON.stringify({
  source: 'pokemon-showdown data/mods/champions (mod champions) vs gen9',
  commit: require('child_process').execSync('git rev-parse HEAD', {cwd: SD}).toString().trim(),
  moves, abilities, items,
}, null, 1));
console.log('wrote', out);
