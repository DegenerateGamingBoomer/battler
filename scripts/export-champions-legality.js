'use strict';
// Exports which species, moves, abilities and items are available in the Champions mod (no
// `isNonstandard` flag), for battler-champions' legality lists.
// Usage: node scripts/export-champions-legality.js [out]
const fs = require('fs');
const path = require('path');
const SD = process.env.SHOWDOWN_DIR || path.resolve(__dirname, '../third_party/pokemon-showdown');
const {Dex} = require(path.join(SD, 'dist/sim'));
const out = process.argv[2] || path.resolve(__dirname, '../battler-champions/data/mc/legality.json');
const c = Dex.mod('champions');
const ids = list => list.filter(e => !e.isNonstandard).map(e => e.id).sort();
fs.writeFileSync(out, JSON.stringify({
  source: 'pokemon-showdown data/mods/champions (isNonstandard unset)',
  commit: require('child_process').execSync('git rev-parse HEAD', {cwd: SD}).toString().trim(),
  moves: ids(c.moves.all()),
  items: ids(c.items.all()),
  abilities: ids(c.abilities.all()),
  species: ids(c.species.all().filter(s => s.num > 0)),
}));
console.log('wrote', out);
