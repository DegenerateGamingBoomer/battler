'use strict';
// Replays a recorded battle on Showdown with the semantic streams and prints the damage
// calculation of every hit, for triaging damage divergences.
// Usage: node compat/diff/replay.js <battles.jsonl> <battle-name> [last-step]
const fs = require('fs');
const path = require('path');

const SD = process.env.SHOWDOWN_DIR || path.resolve(__dirname, '../../third_party/pokemon-showdown');
const hooks = require('./hooks');
hooks.install(SD);
const {Battle} = require(path.join(SD, 'dist/sim'));
const {BattleActions} = require(path.join(SD, 'dist/sim/battle-actions'));
const {SemanticRandom} = require('./semantic');

const [file, name, lastArg] = process.argv.slice(2);
const record = fs.readFileSync(file, 'utf8').split('\n').filter(Boolean).map(JSON.parse).find(r => r.name === name);
const last = lastArg === undefined ? Infinity : Number(lastArg);

let step = 0;
const getDamage = BattleActions.prototype.getDamage;
BattleActions.prototype.getDamage = function (source, target, move, suppress) {
  const battle = this.battle;
  const trace = [];
  const runEvent = battle.runEvent;
  battle.runEvent = function (event, ...args) {
    const result = runEvent.call(this, event, ...args);
    if (/^(BasePower|ModifyAtk|ModifySpA|ModifyDef|ModifySpD|ModifyDamage|WeatherModifyDamage|ModifySTAB|ModifyCritRatio)$/.test(event)) {
      trace.push(`${event}(${JSON.stringify(args[3])}) -> ${JSON.stringify(result)}`);
    }
    return result;
  };
  const priorityEvent = battle.priorityEvent;
  battle.priorityEvent = function (event, ...args) {
    const result = priorityEvent.call(this, event, ...args);
    trace.push(`${event}(${JSON.stringify(args[3])}) -> ${JSON.stringify(result)}`);
    return result;
  };
  const modifyDamage = this.modifyDamage;
  this.modifyDamage = function (base, ...rest) {
    trace.push(`modifyDamage(base=${base})`);
    return modifyDamage.call(this, base, ...rest);
  };
  let result;
  try {
    result = getDamage.call(this, source, target, move, suppress);
  } finally {
    battle.runEvent = runEvent;
    battle.priorityEvent = priorityEvent;
    delete this.modifyDamage;
  }
  const m = typeof move === 'string' ? move : move.id;
  console.log(`[step ${step}] ${source.name} ${m} -> ${target.name}: ${result}  atk=${source.getStat('atk')} spa=${source.getStat('spa')} def=${target.getStat('def')} spd=${target.getStat('spd')}`);
  for (const t of trace) console.log(`    ${t}`);
  return result;
};

const battle = new Battle({formatid: 'gen9championsvgc2026regmc', seed: [1, 2, 3, 4]});
hooks.attach(battle, new SemanticRandom(record.seed));
battle.setPlayer('p1', {name: 'p1', team: record.teams[0]});
battle.setPlayer('p2', {name: 'p2', team: record.teams[1]});
for (const s of record.steps) {
  if (!s.choices) continue;
  if (s.i > last) break;
  step = s.i;
  for (const [k, c] of s.choices.entries()) if (c) battle.sides[k].choose(c);
  battle.commitChoices();
}
