'use strict';
// Exports Showdown's canonical names by ID (species, moves, abilities, items) for the protocol
// translator, which must print Showdown's spelling rather than battler's.
// Usage: node scripts/export-showdown-names.js [out]
const fs = require('fs');
const path = require('path');
const SD = process.env.SHOWDOWN_DIR || path.resolve(__dirname, '../third_party/pokemon-showdown');
const {Dex} = require(path.join(SD, 'dist/sim'));
const out = process.argv[2] || path.resolve(__dirname, '../battler-showdown-compat/data/names.json');
const c = Dex.mod('champions');
// Cosmetic formes (Alcremie flavors, Vivillon patterns, ...) are not in `all()`.
const cosmetic = {};
for (const sp of c.species.all()) {
  for (const name of sp.cosmeticFormes || []) {
    const e = c.species.get(name);
    if (e.exists) cosmetic[e.id] = e.name;
  }
}
const table = list => Object.fromEntries(list.filter(e => e.exists && e.name).map(e => [e.id, e.name]).sort());
fs.writeFileSync(out, JSON.stringify({
  commit: require('child_process').execSync('git rev-parse HEAD', {cwd: SD}).toString().trim(),
  species: {...cosmetic, ...table(c.species.all())},
  moves: table(c.moves.all()),
  abilities: table(c.abilities.all()),
  items: table(c.items.all()),
}));
console.log('wrote', out);
