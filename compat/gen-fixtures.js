'use strict';
// Generates Showdown reference fixtures for the battler-showdown-compat tests.
//
// Every scenario is played on the pinned Showdown build with a constant-zero random source, so
// every random draw is the lowest possible value (all accuracy and chance rolls succeed, every move
// crits, the minimum damage roll is used, ties are never shuffled). The same scenario is replayed on
// battler with every draw forced to 0, which makes the two engines deterministic and comparable
// without a shared semantic random stream (that is P1c).
//
// Usage: node compat/gen-fixtures.js [name-filter]
const fs = require('fs');
const path = require('path');
const SD = process.env.SHOWDOWN_DIR || path.resolve(__dirname, '../third_party/pokemon-showdown');
const {Battle, Teams} = require(path.join(SD, 'dist/sim'));
const scenarios = require('./scenarios');

const filter = process.argv[2];
const outDir = path.join(__dirname, 'fixtures');
fs.mkdirSync(outDir, {recursive: true});

function set(species, ability, moves, extra = {}) {
  return {
    name: species, species, ability, moves, item: '', gender: 'M', level: 50, nature: 'Hardy',
    evs: {hp: 0, atk: 0, def: 0, spa: 0, spd: 0, spe: 0},
    ivs: {hp: 31, atk: 31, def: 31, spa: 31, spd: 31, spe: 31}, ...extra,
  };
}

// Lines that carry no battle semantics (headers, timestamps, Open Team Sheets UI) are dropped so
// that fixtures compare equal to the translator's output.
const META = /^\|(t:|gametype|player|gen|tier|rule|clearpoke|poke|teampreview|uhtml|uhtmlchange|request|inactive|rated)(\||$)/;
function normalize(log) {
  const out = [];
  for (let i = 0; i < log.length; i++) {
    const line = log[i];
    if (line === '|' || line === '') continue;
    if (line.startsWith('|split|')) {
      const secret = log[i + 1], pub = log[i + 2];
      if (META.test(secret) && META.test(pub === '' ? '|uhtml' : pub)) { i += 2; continue; }
      if (META.test(secret)) { i += 2; continue; }
    }
    if (META.test(line)) continue;
    out.push(line.replace(/\n+$/, ''));
  }
  return out;
}

for (const sc of scenarios(set)) {
  if (filter && !sc.name.includes(filter)) continue;
  const teams = sc.teams.map(t => Teams.pack(t));
  const battle = new Battle({
    formatid: sc.format || 'gen9championsvgc2026regmc',
    seed: [1, 2, 3, 4],
    p1: {name: 'p1', team: teams[0]},
    p2: {name: 'p2', team: teams[1]},
  });
  // 'zero': every draw is 0 (all chances succeed, every move crits). 'max': every draw is the
  // largest value (all chances fail, so no crits, no secondaries, and moves below 100% accuracy miss).
  const rngMode = sc.rng || 'zero';
  battle.prng.rng = {next: () => rngMode === 'max' ? 0xFFFFFFFF : 0};
  const requests = [];
  const snap = () => requests.push(battle.sides.map(s => s.activeRequest ? JSON.parse(JSON.stringify(s.activeRequest)) : null));
  snap();
  const steps = [];
  const apply = (p1, p2) => {
    steps.push([p1, p2]);
    for (const [i, input] of [p1, p2].entries()) {
      if (input && !battle.sides[i].choose(input)) {
        if (process.env.DEBUG_CHOICE) console.error(battle.log.slice(-3), battle.sides[i].choice);
        throw new Error(`${sc.name}: step ${steps.length}: invalid choice '${input}' for p${i + 1}; request: ` +
          JSON.stringify(battle.sides[i].activeRequest).slice(0, 400));
      }
    }
    battle.commitChoices();
    snap();
  };
  if (sc.picks) apply(...sc.picks.map(p => `team ${p}`));
  for (const [p1, p2] of sc.turns) apply(p1, p2);
  const fixture = {
    name: sc.name, rng: rngMode, format: sc.format || 'gen9championsvgc2026regmc', gametype: battle.gameType,
    description: sc.description || '', teams, steps, log: normalize(battle.log), requests, ended: battle.ended, rawLog: battle.log,
  };
  fs.writeFileSync(path.join(outDir, `${sc.name}.json`), JSON.stringify(fixture, null, 1));
  console.log(`${sc.name}: ${battle.log.length} log lines, ${steps.length} steps`);
}
