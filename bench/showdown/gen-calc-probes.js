'use strict';
// Exports damage-calculation probes (attacker, defender, move, field) for every active pair in the
// positions, plus Showdown's getDamage output, for timing battler-calc on the same situations.
// Usage: node gen-calc-probes.js [out=../calc-probes.json]
const fs = require('fs');
const path = require('path');
const {sim} = require('./common');
const {Battle} = sim;

const out = path.resolve(process.argv[2] || path.join(__dirname, '../calc-probes.json'));
const posDir = path.join(__dirname, '../positions');
const files = fs.readdirSync(posDir).filter(f => f.endsWith('.json')).sort();

const WEATHER = {raindance: 'Rain', primordialsea: 'Heavy Rain', sunnyday: 'Harsh Sunlight', desolateland: 'Extremely Harsh Sunlight', sandstorm: 'Sandstorm', snowscape: 'Snow', hail: 'Hail'};
const TERRAIN = {electricterrain: 'Electric Terrain', grassyterrain: 'Grassy Terrain', mistyterrain: 'Misty Terrain', psychicterrain: 'Psychic Terrain'};
const SIDE = {reflect: 'Reflect', lightscreen: 'Light Screen', auroraveil: 'Aurora Veil'};

const sp2ev = sp => Math.max(8 * sp - 4, 0);
const mon = p => ({
  name: p.baseSpecies.name,
  level: p.level,
  hp: p.hp, maxhp: p.maxhp,
  ability: p.getAbility().name || null,
  item: p.getItem().name || null,
  nature: p.set.nature || 'Hardy',
  evs: Object.fromEntries(Object.entries(p.set.evs).map(([k, v]) => [k, sp2ev(v)])),
  ivs: p.set.ivs,
  boosts: p.boosts,
  status: p.status || null,
  types: p.getTypes(),
});

const probes = [];
for (const f of files) {
  const pos = JSON.parse(fs.readFileSync(path.join(posDir, f), 'utf8'));
  const battle = Battle.fromJSON(pos.state);
  const field = {
    weather: WEATHER[battle.field.weather] || null,
    terrain: TERRAIN[battle.field.terrain] || null,
    pseudo: Object.keys(battle.field.pseudoWeather),
  };
  const sideConds = battle.sides.map(s => Object.keys(s.sideConditions).map(c => SIDE[c]).filter(Boolean));
  for (const [si, side] of battle.sides.entries()) {
    for (const src of side.active) {
      if (!src || src.fainted) continue;
      for (const [ti, tside] of battle.sides.entries()) {
        for (const tgt of tside.active) {
          if (!tgt || tgt.fainted || tgt === src) continue;
          for (const ms of src.moveSlots) {
            const m = battle.dex.moves.get(ms.id);
            if (m.category === 'Status' && !m.damage) continue;
            const am = battle.dex.getActiveMove(ms.id);
            const dmg = battle.actions.getDamage(src, tgt, am, true);
            probes.push({
              position: pos.id, move: m.name, spread: ['allAdjacentFoes', 'allAdjacent'].includes(m.target) &&
                (tside.active.filter(x => x && !x.fainted).length + (m.target === 'allAdjacent' ? 1 : 0)) > 1,
              attacker: mon(src), defender: mon(tgt), attackerSide: sideConds[si], defenderSide: sideConds[ti],
              field, sdDamage: typeof dmg === 'number' ? dmg : null,
            });
          }
        }
      }
    }
  }
}
fs.writeFileSync(out, JSON.stringify(probes));
console.log(`wrote ${probes.length} probes to ${out}`);
