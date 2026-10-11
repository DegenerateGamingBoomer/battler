'use strict';
// Generates a corpus of random Champions doubles battles on the pinned Showdown build, played with a
// constant random source, for finding translation gaps. Same fixture format as gen-fixtures.js.
// Usage: node compat/gen-corpus.js [count=60] [zero|max|mixed=mixed] [outDir=compat/corpus]
const fs = require('fs');
const path = require('path');
const SD = process.env.SHOWDOWN_DIR || path.resolve(__dirname, '../third_party/pokemon-showdown');
const {Battle, Teams} = require(path.join(SD, 'dist/sim'));
const {PRNG} = require(path.join(SD, 'dist/sim/prng'));
const {randomChoice} = require('../bench/showdown/common.js');

const count = Number(process.argv[2] || 60);
const mode = process.argv[3] || 'mixed';
const outDir = path.resolve(process.argv[4] || path.join(__dirname, 'corpus'));
fs.mkdirSync(outDir, {recursive: true});

const META = /^\|(t:|gametype|player|gen|tier|rule|clearpoke|poke|teampreview|uhtml|uhtmlchange|request|inactive|rated)(\||$)/;
function normalize(log) {
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

for (let idx = 0; idx < count; idx++) {
  const gen = new PRNG([77, idx, 0, 1]);
  const mk = () => {
    const t = Teams.generate('gen9championsrandomdoublesbattle', {seed: [gen.random(65536), gen.random(65536), gen.random(65536), gen.random(65536)]});
    for (const s of t) { s.level = 50; s.evs = {hp: 0, atk: 0, def: 0, spa: 0, spd: 0, spe: 0}; s.nature = s.nature || 'Hardy'; }
    return t;
  };
  const teams = [Teams.pack(mk()), Teams.pack(mk())];
  const rngMode = mode === 'mixed' ? (idx % 2 ? 'max' : 'zero') : mode;
  const battle = new Battle({formatid: 'gen9championsvgc2026regmc', seed: [1, 2, 3, 4], p1: {name: 'p1', team: teams[0]}, p2: {name: 'p2', team: teams[1]}});
  battle.prng.rng = {next: () => rngMode === 'max' ? 0xFFFFFFFF : 0};
  const chooser = new PRNG([5, idx, 0, 1]);
  const steps = [];
  const requests = [];
  let guard = 0;
  while (!battle.ended && guard++ < 60) {
    const choices = battle.sides.map(s => randomChoice(s.activeRequest, chooser, 0.3));
    const inputs = choices.map(c => c || '');
    steps.push(inputs);
    for (const [i, c] of inputs.entries()) {
      if (c && !battle.sides[i].choose(c)) {
        // Fall back to the default choice if a random choice was illegal.
        battle.sides[i].autoChoose();
        inputs[i] = 'default';
      }
    }
    battle.commitChoices();
  }
  const fixture = {
    name: `corpus-${String(idx).padStart(3, '0')}`, rng: rngMode, format: 'gen9championsvgc2026regmc', gametype: 'doubles',
    description: 'random battle', teams, steps, log: normalize(battle.log), requests: [], ended: battle.ended,
  };
  fs.writeFileSync(path.join(outDir, `${fixture.name}.json`), JSON.stringify(fixture));
}
console.log(`wrote ${count} battles to ${outDir}`);
