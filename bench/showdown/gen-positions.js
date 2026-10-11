'use strict';
// Generates mid-game Reg M-C doubles positions with the pinned Showdown build.
// Usage: node gen-positions.js [count=200] [outDir=../positions]
const fs = require('fs');
const path = require('path');
const {sim, PRNG, FORMAT, TEAM_FORMAT, step, range} = require('./common');
const {Battle, Teams} = sim;

const count = Number(process.argv[2] || 200);
const outDir = path.resolve(process.argv[3] || path.join(__dirname, '../positions'));
fs.mkdirSync(outDir, {recursive: true});

function teamsFor(seed) {
  const gen = new PRNG(seed);
  const mk = () => {
    const t = Teams.generate(TEAM_FORMAT, {seed: [gen.random(2 ** 16), gen.random(2 ** 16), gen.random(2 ** 16), gen.random(2 ** 16)]});
    for (const set of t) set.level = 50; // Reg M-C is fixed at level 50.
    return t;
  };
  return [mk(), mk()];
}

let made = 0;
for (let idx = 0; made < count; idx++) {
  const seed = [0x5eed, idx, 0, 1];
  const [t1, t2] = teamsFor(seed);
  const pickPrng = new PRNG([0xc0de, idx, 0, 1]);
  const turnsWanted = 3 + pickPrng.random(4); // 3-6
  const battle = new Battle({
    formatid: FORMAT,
    seed: [0xba77, idx, 0, 1],
    p1: {name: 'p1', team: Teams.pack(t1)},
    p2: {name: 'p2', team: Teams.pack(t2)},
  });
  let guard = 0;
  while (!battle.ended && (battle.turn < turnsWanted + 1 || battle.sides.some(s => s.activeRequest && s.activeRequest.forceSwitch)) && guard++ < 200) {
    step(battle, pickPrng);
  }
  // Keep only live positions that wait on a normal move request from both sides.
  const requests = battle.sides.map(s => s.activeRequest);
  if (battle.ended || !requests.every(r => r && r.active && !r.forceSwitch && !r.wait)) continue;
  const name = String(made).padStart(3, '0');
  fs.writeFileSync(path.join(outDir, `${name}.json`), JSON.stringify({
    id: made,
    format: FORMAT,
    battleSeed: [0xba77, idx, 0, 1],
    teamSeed: seed,
    turn: battle.turn,
    teams: [t1, t2],
    packedTeams: [Teams.pack(t1), Teams.pack(t2)],
    inputLog: battle.inputLog,
    state: battle.toJSON(),
  }));
  made++;
}
console.log(`wrote ${made} positions to ${outDir}`);
