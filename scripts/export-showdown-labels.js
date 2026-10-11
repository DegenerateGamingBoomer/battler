'use strict';
// Extracts the effect labels Showdown prints in minor-action lines (`-start`, `-end`, `-singleturn`,
// `-sidestart`, ...), per effect, by scanning the pinned data files for `this.add('-kind', ..., 'label')`.
// The translator uses them to spell battler's effects the way Showdown does.
// Usage: node scripts/export-showdown-labels.js [out]
const fs = require('fs');
const path = require('path');
const SD = process.env.SHOWDOWN_DIR || path.resolve(__dirname, '../third_party/pokemon-showdown');
const out = process.argv[2] || path.resolve(__dirname, '../battler-showdown-compat/data/labels.json');

// Kinds whose label is the 2nd argument (after a Pokémon or side), and kinds whose label is the first.
const WITH_TARGET = ['start', 'end', 'singleturn', 'singlemove', 'sidestart', 'sideend', 'activate', 'block', 'item', 'enditem', 'ability', 'endability'];
const WITHOUT_TARGET = ['fieldstart', 'fieldend', 'fieldactivate'];

function scan(file, mod) {
  const text = fs.readFileSync(path.join(SD, file), 'utf8');
  const entries = text.split(/\n\t(?=[a-z0-9]+: \{)/).slice(1);
  const result = {};
  for (const entry of entries) {
    const id = entry.slice(0, entry.indexOf(':'));
    const labels = {};
    for (const kind of WITH_TARGET) {
      const re = new RegExp(`this\\.add\\('-${kind}',\\s*[^,]+,\\s*'([^']*)'`, 'g');
      const found = [...entry.matchAll(re)].map(m => m[1]);
      if (found.length) labels[kind] = [...new Set(found)];
    }
    for (const kind of WITHOUT_TARGET) {
      const re = new RegExp(`this\\.add\\('-${kind}',\\s*'([^']*)'`, 'g');
      const found = [...entry.matchAll(re)].map(m => m[1]);
      if (found.length) labels[kind] = [...new Set(found)];
    }
    if (Object.keys(labels).length) result[id] = labels;
  }
  return result;
}

const files = {
  moves: 'data/moves.ts', abilities: 'data/abilities.ts', items: 'data/items.ts', conditions: 'data/conditions.ts',
};
const table = {commit: require('child_process').execSync('git rev-parse HEAD', {cwd: SD}).toString().trim()};
for (const [kind, file] of Object.entries(files)) table[kind] = scan(file);
// The Champions mod overrides a few effects.
for (const [kind, file] of Object.entries({moves: 'data/mods/champions/moves.ts', abilities: 'data/mods/champions/abilities.ts', items: 'data/mods/champions/items.ts', conditions: 'data/mods/champions/conditions.ts'})) {
  Object.assign(table[kind], scan(file));
}
fs.writeFileSync(out, JSON.stringify(table));
console.log('wrote', out, Object.fromEntries(Object.keys(files).map(k => [k, Object.keys(table[k]).length])));
