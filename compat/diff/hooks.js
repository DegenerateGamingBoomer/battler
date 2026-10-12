'use strict';
// Semantic random hooks for the pinned Showdown build (spec P1c).
//
// Battle.prototype.random / randomChance / sample and PRNG.prototype.shuffle are replaced. Each
// call is classified by its caller frame against the tables below, and answered from the battle's
// semantic streams (`battle.semantic`, a SemanticRandom). An unknown call site throws.
//
// Two methods are rewritten instead of hooked: BattleActions#secondaries and #selfDrops roll
// `random(100)` even for a 100% (chance-less) effect. Their rewrites roll `randomChance(chance,
// 100)` only when there is a chance, which is the same comparison (floor(u * 100) < chance) under
// the rule that certain outcomes draw nothing. The originals are checked to still contain the
// rolls being replaced.
const fs = require('fs');
const path = require('path');

const SPREAD_TARGETS = new Set([
  'allAdjacentFoes', 'allAdjacent', 'foeSide', 'allySide', 'allyTeam', 'all', 'self',
]);

// Sim call sites: `${file}:${function}` → category, or a function of (battle, method) for sites
// that need more context.
const SIM_SITES = {
  'battle-actions.js:hitStepAccuracy': 'accuracy',
  'battle-actions.js:hitStepMoveHitLoop': (battle, method) => method === 'randomChance' ? 'accuracy' : 'multi-hit',
  'champions/scripts.js:hitStepMoveHitLoop': (battle, method) => method === 'randomChance' ? 'accuracy' : 'multi-hit',
  'battle-actions.js:getDamage': 'crit',
  'battle.js:randomizer': 'damage-roll',
  'hooks.js:secondaries': 'secondary',
  'hooks.js:selfDrops': 'secondary',
  'battle-queue.js:insertChoice': 'speed-tie',
  'battle.js:speedSort': 'speed-tie',
  'battle.js:getRandomSwitchable': 'random-switch',
  'battle.js:getRandomTarget': battle => targetCategory(battle),
  'side.js:randomFoe': battle => targetCategory(battle),
};

// Random targets that nothing uses go to a category that is not compared:
// - the printed target of a spread or side move, which battler does not pick (allowlist R1);
// - targets picked while ordering actions: getActionSpeed looks up a target only for priority
//   modifiers, and a random-target move (Struggle, Outrage) picks its target again when it is
//   used, ignoring the one resolveAction stored.
function targetCategory(battle) {
  const move = battle.__randomTargetMove;
  if (move && SPREAD_TARGETS.has(move.target)) return 'display-target';
  if (battle.__orderingActions) return 'display-target';
  if (battle.__resolvingAction && move && move.target === 'randomNormal') return 'display-target';
  return 'target';
}

// Data call sites that share a category; every other data site is `effect:<id>`.
const DATA_SITES = {
  'par.onBeforeMove': 'full-para',
  'slp.onStart': 'status-turns',
  'frz.onBeforeMove': 'status-turns',
  'confusion.onStart': 'status-turns',
  'confusion.onBeforeMove': 'confusion-hit',
  'twoturnmove.onStart': 'target',
};

const RANDOM_CALL = /\b(this|battle)\.(random|randomChance|sample)\(|\.prng\./;

// For each compiled data file: the top-level entry of every line, and which entries draw.
const dataFiles = new Map();
function dataFile(file) {
  let entry = dataFiles.get(file);
  if (entry) return entry;
  const lines = fs.readFileSync(file, 'utf8').split('\n');
  const keyOfLine = new Array(lines.length + 1).fill(null);
  const drawing = new Set();
  let key = null;
  for (let i = 0; i < lines.length; i++) {
    const m = /^ {2}"?([a-z0-9]+)"?: \{/.exec(lines[i]);
    if (m) key = m[1];
    keyOfLine[i + 1] = key;
    if (key && RANDOM_CALL.test(lines[i])) drawing.add(key);
  }
  entry = {keyOfLine, drawing};
  dataFiles.set(file, entry);
  return entry;
}

function callerFrames(fn) {
  const prepare = Error.prepareStackTrace;
  const limit = Error.stackTraceLimit;
  Error.prepareStackTrace = (_, frames) => frames;
  Error.stackTraceLimit = 4;
  const holder = {};
  Error.captureStackTrace(holder, fn);
  const frames = holder.stack;
  Error.prepareStackTrace = prepare;
  Error.stackTraceLimit = limit;
  return frames;
}

function classify(battle, method, fn) {
  const frame = callerFrames(fn)[0];
  const file = frame.getFileName() || '';
  const name = frame.getFunctionName() || frame.getMethodName() || '<anonymous>';
  const dataMatch = /\/dist\/data\/(?:mods\/([a-z0-9]+)\/)?([a-z-]+)\.js$/.exec(file);
  if (dataMatch && dataMatch[2] !== 'scripts') {
    const {keyOfLine, drawing} = dataFile(file);
    const id = keyOfLine[frame.getLineNumber()];
    if (!id || !drawing.has(id)) {
      throw new Error(`unknown Showdown random call site: ${file}:${frame.getLineNumber()} (${name})`);
    }
    return {category: DATA_SITES[`${id}.${name}`] || `effect:${id}`, site: `${id}.${name}`};
  }
  const base = dataMatch ? `${dataMatch[1]}/scripts.js` : path.basename(file);
  const key = `${base}:${name}`;
  let category = SIM_SITES[key];
  if (typeof category === 'function') category = category(battle, method);
  if (!category) throw new Error(`unknown Showdown random call site: ${key} (${method})`);
  return {category, site: key};
}

function owner(item) {
  const holder = item.effectHolder || item.pokemon || item;
  if (holder && holder.side && typeof holder.position === 'number' && holder.getStat) {
    return `${holder.side.id}:${holder.side.team.indexOf(holder.set)}`;
  }
  if (holder && holder.sideConditions) return holder.id;
  return 'field';
}

function install(SD) {
  const {Battle} = require(path.join(SD, 'dist/sim/battle'));
  const {PRNG} = require(path.join(SD, 'dist/sim/prng'));
  const {BattleActions} = require(path.join(SD, 'dist/sim/battle-actions'));
  if (Battle.prototype.__semanticHooks) return;
  Battle.prototype.__semanticHooks = true;

  const original = {
    random: Battle.prototype.random,
    randomChance: Battle.prototype.randomChance,
    sample: Battle.prototype.sample,
    shuffle: PRNG.prototype.shuffle,
    getRandomTarget: Battle.prototype.getRandomTarget,
    secondaries: BattleActions.prototype.secondaries,
    selfDrops: BattleActions.prototype.selfDrops,
  };
  for (const [method, roll] of [['secondaries', 'const secondaryRoll = this.battle.random(100);'],
    ['selfDrops', 'const secondaryRoll = this.battle.random(100);']]) {
    if (!original[method].toString().includes(roll)) {
      throw new Error(`BattleActions#${method} changed; update compat/diff/hooks.js`);
    }
  }

  Battle.prototype.random = function random(m, n) {
    if (!this.semantic) return original.random.call(this, m, n);
    const {category, site} = classify(this, 'random', random);
    if (m === undefined) throw new Error(`float random() at ${site}`);
    if (n === undefined) return this.semantic.int(category, m, site);
    m = Math.floor(m);
    return m + this.semantic.int(category, Math.floor(n) - m, site);
  };
  Battle.prototype.randomChance = function randomChance(numerator, denominator) {
    if (!this.semantic) return original.randomChance.call(this, numerator, denominator);
    if (this.forceRandomChance !== null) return this.forceRandomChance;
    const {category, site} = classify(this, 'randomChance', randomChance);
    return this.semantic.chance(category, numerator, denominator, site);
  };
  Battle.prototype.sample = function sample(items) {
    if (!this.semantic) return original.sample.call(this, items);
    if (!items.length) throw new Error('Cannot sample an empty array');
    const {category, site} = classify(this, 'sample', sample);
    return items[this.semantic.int(category, items.length, site)];
  };
  const {BattleQueue: Queue} = require(path.join(SD, 'dist/sim/battle-queue'));
  for (const [proto, method, flag] of [
    [Battle.prototype, 'getActionSpeed', '__orderingActions'],
    [Queue.prototype, 'resolveAction', '__resolvingAction'],
  ]) {
    const originalMethod = proto[method];
    proto[method] = function (...args) {
      const battle = this.battle || this;
      battle[flag] = (battle[flag] || 0) + 1;
      try {
        return originalMethod.apply(this, args);
      } finally {
        battle[flag]--;
      }
    };
  }
  Battle.prototype.getRandomTarget = function (pokemon, move) {
    const previous = this.__randomTargetMove;
    this.__randomTargetMove = this.dex.moves.get(move);
    try {
      return original.getRandomTarget.call(this, pokemon, move);
    } finally {
      this.__randomTargetMove = previous;
    }
  };
  PRNG.prototype.shuffle = function shuffle(items, start = 0, end = items.length) {
    const battle = this.__battle;
    if (!battle || !battle.semantic) return original.shuffle.call(this, items, start, end);
    const {category, site} = classify(battle, 'shuffle', shuffle);
    battle.semantic.tie(items.slice(start, end).map(owner));
    while (start < end - 1) {
      const next = start + battle.semantic.int(category, end - start, site);
      if (start !== next) [items[start], items[next]] = [items[next], items[start]];
      start++;
    }
  };

  // Tie owners for queue insertions: the actions already tied and the inserted ones.
  const {BattleQueue} = require(path.join(SD, 'dist/sim/battle-queue'));
  const insertChoice = BattleQueue.prototype.insertChoice;
  BattleQueue.prototype.insertChoice = function (choices, midTurn = false) {
    const battle = this.battle;
    if (!battle.semantic) return insertChoice.call(this, choices, midTurn);
    const queue = this;
    const inserted = Array.isArray(choices) ? choices : [choices];
    battle.random = function random(m, n) {
      const {category, site} = classify(battle, 'random', random);
      if (site !== 'battle-queue.js:insertChoice') throw new Error(`unexpected random() in insertChoice: ${site}`);
      battle.semantic.tie([...queue.list.slice(m, n - 1), ...inserted].map(owner));
      return m + battle.semantic.int(category, n - m, site);
    };
    try {
      return insertChoice.call(this, choices, midTurn);
    } finally {
      delete battle.random;
    }
  };

  BattleActions.prototype.selfDrops = function selfDrops(targets, source, move, moveData, isSecondary) {
    if (!this.battle.semantic) return original.selfDrops.call(this, targets, source, move, moveData, isSecondary);
    for (const target of targets) {
      if (target === false) continue;
      if (moveData.self && !move.selfDropped) {
        if (!isSecondary && moveData.self.boosts) {
          if (typeof moveData.self.chance === 'undefined' || this.battle.randomChance(moveData.self.chance, 100)) {
            this.moveHit(source, source, move, moveData.self, isSecondary, true);
          }
          if (!move.multihit) move.selfDropped = true;
        } else {
          this.moveHit(source, source, move, moveData.self, isSecondary, true);
        }
      }
    }
  };
  BattleActions.prototype.secondaries = function secondaries(targets, source, move, moveData, isSelf) {
    if (!this.battle.semantic) return original.secondaries.call(this, targets, source, move, moveData, isSelf);
    if (!moveData.secondaries) return;
    for (const target of targets) {
      if (target === false) continue;
      const list = this.battle.runEvent('ModifySecondaries', target, source, moveData, moveData.secondaries.slice());
      for (const secondary of list) {
        const overflow = (secondary.boosts || secondary.self) && this.battle.gen <= 8;
        if (typeof secondary.chance === 'undefined' ||
          this.battle.randomChance(overflow ? secondary.chance % 256 : secondary.chance, 100)) {
          this.moveHit(target, source, move, secondary, true, isSelf);
        }
      }
    }
  };
}

/** Attaches semantic streams to a battle. Raw PRNG draws after this point throw. */
function attach(battle, semantic) {
  battle.semantic = semantic;
  battle.prng.__battle = battle;
  battle.prng.rng = {next() { throw new Error('unclassified raw PRNG draw'); }};
}

module.exports = {install, attach, classify};
