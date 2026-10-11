'use strict';
// Shared helpers for the Showdown side of the benchmarks.
const path = require('path');
const SD_ROOT = process.env.SHOWDOWN_DIR ||
  path.resolve(__dirname, '../../third_party/pokemon-showdown');
const sim = require(path.join(SD_ROOT, 'dist/sim'));
const {PRNG} = require(path.join(SD_ROOT, 'dist/sim/prng'));

const FORMAT = 'gen9championsvgc2026regmc';
const TEAM_FORMAT = 'gen9championsrandomdoublesbattle';

function range(a, b) {
  const r = [];
  for (let i = a; i <= b; i++) r.push(i);
  return r;
}

/**
 * Builds a legal choice string for one side from its request, modeled on
 * sim/tools/random-player-ai.ts (Mega allowed with probability `mega`).
 * Returns null when the side has nothing to choose.
 */
function randomChoice(request, prng, mega = 0.2) {
  if (!request || request.wait) return null;
  const pokemon = request.side.pokemon;
  if (request.forceSwitch) {
    const chosen = [];
    return request.forceSwitch.map((must, i) => {
      if (!must) return 'pass';
      const can = range(1, pokemon.length).filter(j =>
        j > request.forceSwitch.length && !chosen.includes(j) &&
        !pokemon[j - 1].condition.endsWith(' fnt'));
      if (!can.length) return 'pass';
      const t = prng.sample(can);
      chosen.push(t);
      return `switch ${t}`;
    }).join(', ');
  }
  if (request.teamPreview) {
    const order = range(1, pokemon.length);
    prng.shuffle(order);
    const size = request.maxChosenTeamSize || 4;
    return `team ${order.slice(0, size).join('')}`;
  }
  if (!request.active) return null;
  let canMega = true;
  const chosen = [];
  return request.active.map((active, i) => {
    if (pokemon[i].condition.endsWith(' fnt') || pokemon[i].commanding) return 'pass';
    canMega = canMega && !!active.canMegaEvo;
    const change = canMega && prng.random() < mega;
    const moves = active.moves.map((m, k) => ({slot: k + 1, target: m.target, disabled: m.disabled}))
      .filter(m => !m.disabled);
    const hasAlly = pokemon.length > 1 && !pokemon[i ^ 1].condition.endsWith(' fnt');
    let usable = moves.filter(m => m.target !== 'adjacentAlly' || hasAlly);
    if (!usable.length) usable = moves;
    const canSwitch = range(1, pokemon.length).filter(j =>
      !pokemon[j - 1].active && !chosen.includes(j) && !pokemon[j - 1].condition.endsWith(' fnt'));
    const switches = active.trapped ? [] : canSwitch;
    // Prefer moves (as the stock random AI does with move=1.0); switch only when no move is usable.
    if (switches.length && !usable.length) {
      const t = prng.sample(switches);
      chosen.push(t);
      return `switch ${t}`;
    }
    if (!usable.length) return 'default';
    const m = prng.sample(usable);
    let s = `move ${m.slot}`;
    if (request.active.length > 1) {
      if (['normal', 'any', 'adjacentFoe'].includes(m.target)) s += ` ${1 + prng.random(2)}`;
      if (m.target === 'adjacentAlly') s += ` -${(i ^ 1) + 1}`;
      if (m.target === 'adjacentAllyOrSelf') s += hasAlly ? ` -${1 + prng.random(2)}` : ` -${i + 1}`;
    }
    if (change) {
      canMega = false;
      s += ' mega';
    }
    return s;
  }).join(', ');
}

/** Plays one decision step: choose for every side that has a request, then commit. */
function step(battle, prng, timing) {
  const t0 = process.hrtime.bigint();
  const choices = battle.sides.map(side => randomChoice(side.activeRequest, prng));
  const t1 = process.hrtime.bigint();
  const inputs = choices.map(c => c || '');
  let ok = true;
  for (const [i, c] of inputs.entries()) {
    if (c && !battle.sides[i].choose(c)) ok = false;
  }
  if (!ok) {
    // Illegal random choice: fall back to the engine's default choice for stuck sides.
    for (const side of battle.sides) {
      if (side.activeRequest && !side.activeRequest.wait && !side.isChoiceDone()) side.autoChoose();
    }
  }
  battle.commitChoices();
  const t2 = process.hrtime.bigint();
  if (timing) {
    timing.choose += Number(t1 - t0);
    timing.advance += Number(t2 - t1);
    timing.choices += choices.filter(Boolean).length;
    timing.steps += 1;
  }
}

function quantile(sorted, q) {
  if (!sorted.length) return null;
  const idx = Math.min(sorted.length - 1, Math.ceil(q * sorted.length) - 1);
  return sorted[Math.max(0, idx)];
}

function stats(values) {
  const s = [...values].sort((a, b) => a - b);
  return {n: s.length, median: quantile(s, 0.5), p90: quantile(s, 0.9), min: s[0], max: s[s.length - 1]};
}

module.exports = {sim, PRNG, SD_ROOT, FORMAT, TEAM_FORMAT, range, randomChoice, step, stats};
