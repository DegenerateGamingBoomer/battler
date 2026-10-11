'use strict';
// Showdown-side benchmarks (a) restore, (b) two-turn rollouts, (c) damage probes.
// Usage: node bench.js [reps=7] [out=../../docs/benchmarks/p0-showdown.json]
const fs = require('fs');
const path = require('path');
const {sim, PRNG, step, stats} = require('./common');
const {Battle} = sim;

const reps = Number(process.argv[2] || 7);
const out = path.resolve(process.argv[3] || path.join(__dirname, '../../docs/benchmarks/p0-showdown.json'));
const posDir = path.join(__dirname, '../positions');
const files = fs.readdirSync(posDir).filter(f => f.endsWith('.json')).sort();
const positions = files.map(f => JSON.parse(fs.readFileSync(path.join(posDir, f), 'utf8')));
const texts = positions.map(p => JSON.stringify(p.state));
const ns = () => Number(process.hrtime.bigint());
const us = n => n / 1e3;

const result = {engine: 'pokemon-showdown', positions: positions.length, reps};

// (a) Restore ---------------------------------------------------------------
{
  // Cold: first restore in a fresh process is reported separately by running this
  // script's first call before any warm-up.
  let t = ns();
  Battle.fromJSON(JSON.parse(texts[0]));
  const firstFromObjectIncludingParse = ns() - t;
  const fromText = [], fromObject = [];
  // Warm-up pass.
  for (const tx of texts) Battle.fromJSON(tx);
  for (let r = 0; r < reps; r++) {
    for (const tx of texts) {
      t = ns();
      Battle.fromJSON(tx);
      fromText.push(us(ns() - t));
    }
    for (const tx of texts) {
      const obj = JSON.parse(tx); // parse is not timed
      t = ns();
      Battle.fromJSON(obj);
      fromObject.push(us(ns() - t));
    }
  }
  result.restore = {
    unit: 'us',
    first_in_process_us_incl_json_parse: us(firstFromObjectIncludingParse),
    fromJSON_text: stats(fromText),
    fromJSON_object: stats(fromObject),
  };
}

// (b) Two-turn rollouts -------------------------------------------------------
{
  const restoreT = [], rolloutT = [], perTurn = [], perChoice = [], chooseT = [], advanceT = [];
  let ended = 0, turns = 0;
  const run = (measure, rep) => {
    positions.forEach((p, i) => {
      const prng = new PRNG([0xbe9c, i, rep, 7]);
      const timing = {choose: 0, advance: 0, choices: 0, steps: 0};
      const t0 = ns();
      const battle = Battle.fromJSON(texts[i]);
      const t1 = ns();
      const startTurn = battle.turn;
      let guard = 0;
      while (!battle.ended && battle.turn < startTurn + 2 && guard++ < 50) step(battle, prng, timing);
      const t2 = ns();
      if (!measure) return;
      const turnsPlayed = battle.turn - startTurn;
      if (battle.ended) ended++;
      turns += turnsPlayed;
      restoreT.push(us(t1 - t0));
      rolloutT.push(us(t2 - t0));
      if (turnsPlayed > 0) perTurn.push(us(t2 - t1) / turnsPlayed);
      if (timing.choices > 0) perChoice.push(us(t2 - t1) / timing.choices);
      chooseT.push(us(timing.choose));
      advanceT.push(us(timing.advance));
    });
  };
  run(false, -1);
  for (let r = 0; r < reps; r++) run(true, r);
  result.rollout2 = {
    unit: 'us',
    note: 'Battle.fromJSON(text) + 2 turns of random legal choices (rollouts that end early are included with fewer turns)',
    restore_plus_two_turns: stats(rolloutT),
    restore_part: stats(restoreT),
    advance_per_turn_incl_choice_gen: stats(perTurn),
    advance_per_choice_incl_choice_gen: stats(perChoice),
    choice_generation_per_rollout: stats(chooseT),
    engine_advance_per_rollout: stats(advanceT),
    rollouts_ended_early: ended,
    turns_played: turns,
  };
}

// (c) Damage probes -------------------------------------------------------------
{
  const times = [], perPos = [];
  let calls = 0;
  const probe = (measure) => {
    for (const tx of texts) {
      const battle = Battle.fromJSON(tx);
      const pairs = [];
      for (const src of battle.sides.flatMap(s => s.active)) {
        if (!src || src.fainted) continue;
        for (const tgt of battle.sides.flatMap(s => s.active)) {
          if (!tgt || tgt.fainted || tgt === src) continue;
          for (const mv of src.moveSlots) {
            const m = battle.dex.moves.get(mv.id);
            if (m.category === 'Status' && !m.damage) continue;
            pairs.push([src, tgt, mv.id]);
          }
        }
      }
      const tp0 = ns();
      for (const [src, tgt, id] of pairs) {
        const move = battle.dex.getActiveMove(id);
        const t = ns();
        battle.actions.getDamage(src, tgt, move, true);
        const d = ns() - t;
        if (measure) times.push(us(d));
      }
      if (measure) { perPos.push(us(ns() - tp0)); calls += pairs.length; }
    }
  };
  probe(false);
  for (let r = 0; r < reps; r++) probe(true);
  result.damage = {
    unit: 'us',
    note: 'battle.actions.getDamage(source, target, move, true), all (attacker, any other active mon, damaging move) triples; getActiveMove construction excluded from per-call timing',
    per_call: stats(times),
    calls_measured: calls,
  };
}

result.env = {node: process.version, platform: process.platform, arch: process.arch};
fs.mkdirSync(path.dirname(out), {recursive: true});
fs.writeFileSync(out, JSON.stringify(result, null, 2));
console.log('wrote', out);
