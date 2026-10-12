'use strict';
// Differential harness driver (spec P1c): plays the same battles on the pinned Showdown (in this
// process) and on battler (battler-diff-runner, over JSON lines), with the same teams, choices and
// semantic random streams, and compares draws, normalized state, protocol lines and requests after
// every decision step.
//
// Usage:
//   cargo build --release -p battler-diff-runner
//   node compat/diff/driver.js [--battles N] [--start K] [--scenarios [filter]] [--out DIR]
//                              [--verbose] [--battle K] [--transplant [TURNS]]
//
// --transplant: plays each battle on Showdown alone and, at every turn boundary, transplants its
// serialized state into battler (spec P2) and compares TURNS turns (default 3) from there.
//
// Output: a summary on stdout and, in --out (default compat/diff/out, gitignored), one JSON line
// per battle in battles.jsonl plus summary.json.
const fs = require('fs');
const path = require('path');
const readline = require('readline');
const {spawn} = require('child_process');

const SD = process.env.SHOWDOWN_DIR || path.resolve(__dirname, '../../third_party/pokemon-showdown');
const hooks = require('./hooks');
hooks.install(SD);
const {Battle, Teams, Dex} = require(path.join(SD, 'dist/sim'));
const {PRNG} = require(path.join(SD, 'dist/sim/prng'));
const {SemanticRandom} = require('./semantic');
const {normalize} = require('./state');
const {diffDraws, diffJson, diffProtocol, normalizeLog, lineShape} = require('./compare');

const FORMAT = 'gen9championsvgc2026regmc';
const TEAM_FORMAT = 'gen9championsrandomdoublesbattle';

function parseArgs(argv) {
  const args = {battles: 100, start: 0, out: path.join(__dirname, 'out'), verbose: false, scenarios: null, battle: null,
    runner: path.resolve(__dirname, '../../target/release/battler-diff-runner')};
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--battles') args.battles = Number(argv[++i]);
    else if (a === '--start') args.start = Number(argv[++i]);
    else if (a === '--battle') { args.battle = Number(argv[++i]); }
    else if (a === '--out') args.out = path.resolve(argv[++i]);
    else if (a === '--runner') args.runner = path.resolve(argv[++i]);
    else if (a === '--verbose') args.verbose = true;
    else if (a === '--scenarios') args.scenarios = (argv[i + 1] && !argv[i + 1].startsWith('--')) ? argv[++i] : '';
    else if (a === '--merge') args.merge = argv[++i].split(',').map(d => path.resolve(d));
    else if (a === '--transplant') args.transplant = (argv[i + 1] && /^\d+$/.test(argv[i + 1])) ? Number(argv[++i]) : 3;
    else throw new Error(`unknown argument ${a}`);
  }
  return args;
}

class Runner {
  constructor(file) {
    if (!fs.existsSync(file)) throw new Error(`${file} not found; run cargo build --release -p battler-diff-runner`);
    this.proc = spawn(file, [], {stdio: ['pipe', 'pipe', 'inherit']});
    this.pending = [];
    readline.createInterface({input: this.proc.stdout}).on('line', line => {
      const next = this.pending.shift();
      if (next) next(JSON.parse(line));
    });
  }
  send(command) {
    return new Promise(resolve => {
      this.pending.push(resolve);
      this.proc.stdin.write(JSON.stringify(command) + '\n');
    });
  }
  close() { this.proc.stdin.end(); }
}

// ---------------------------------------------------------------------------------------------
// Teams

function speedStats(teams) {
  const battle = new Battle({formatid: FORMAT, seed: [1, 2, 3, 4]});
  battle.setPlayer('p1', {name: 'p1', team: teams[0]});
  battle.setPlayer('p2', {name: 'p2', team: teams[1]});
  return battle.sides.flatMap(side => side.pokemon.map(p => p.storedStats.spe));
}

/** Two Champions random-doubles teams at level 50 with explicit genders and no equal speeds. */
function makeTeams(seed) {
  const prng = new PRNG([seed & 0xffff, (seed >>> 16) & 0xffff, 77, 1]);
  let last;
  for (let attempt = 0; attempt < 50; attempt++) {
    const teams = [0, 1].map(() => {
      const sets = Teams.generate(TEAM_FORMAT, {seed: [prng.random(65536), prng.random(65536), prng.random(65536), prng.random(65536)]});
      for (const set of sets) {
        set.level = 50;
        if (!set.gender) {
          const species = Dex.species.get(set.species);
          set.gender = species.gender || (prng.random(2) ? 'F' : 'M');
        }
      }
      return Teams.pack(sets);
    });
    last = teams;
    const speeds = speedStats(teams);
    if (new Set(speeds).size === speeds.length) return {teams, attempts: attempt + 1};
  }
  return {teams: last, attempts: 50, speedTies: true};
}

// ---------------------------------------------------------------------------------------------
// Choices

function range(a, b) {
  const r = [];
  for (let i = a; i <= b; i++) r.push(i);
  return r;
}

/** A random legal choice for one side's request, or '' when there is nothing to choose. */
function randomChoice(request, prng) {
  if (!request || request.wait) return '';
  const pokemon = request.side.pokemon;
  const alive = j => !pokemon[j - 1].condition.endsWith(' fnt');
  if (request.teamPreview) {
    const order = range(1, pokemon.length);
    prng.shuffle(order);
    return `team ${order.slice(0, request.maxChosenTeamSize || 4).join('')}`;
  }
  if (request.forceSwitch) {
    const chosen = [];
    return request.forceSwitch.map((must, i) => {
      if (!must) return 'pass';
      // Revival Blessing revives a fainted Pokémon.
      const can = pokemon[i].reviving
        ? range(1, pokemon.length).filter(j => !chosen.includes(j) && !alive(j))
        : range(1, pokemon.length).filter(j => j > request.forceSwitch.length && !chosen.includes(j) && alive(j));
      if (!can.length) return 'pass';
      const t = prng.sample(can);
      chosen.push(t);
      return `switch ${t}`;
    }).join(', ');
  }
  if (!request.active) return '';
  let canMega = true;
  const chosen = [];
  return request.active.map((active, i) => {
    if (!alive(i + 1) || pokemon[i].commanding) return 'pass';
    const hasAlly = request.active.length > 1 && alive((i ^ 1) + 1);
    // A Pokémon that may be trapped (by an ability Showdown keeps hidden) does not try to switch.
    const switches = active.trapped || active.maybeTrapped ? [] : range(1, pokemon.length).filter(j =>
      !pokemon[j - 1].active && !chosen.includes(j) && alive(j));
    const usable = active.moves.map((m, k) => ({slot: k + 1, target: m.target, disabled: m.disabled}))
      .filter(m => !m.disabled);
    const moves = usable.filter(m => m.target !== 'adjacentAlly' || hasAlly);
    if (switches.length && (!moves.length || prng.random() < 0.1)) {
      const t = prng.sample(switches);
      chosen.push(t);
      return `switch ${t}`;
    }
    // Only a move that needs an ally left (Encore into Helping Hand): Showdown still takes it.
    if (!moves.length && usable.length) return `move ${usable[0].slot}${request.active.length > 1 ? ` -${(i ^ 1) + 1}` : ''}`;
    if (!moves.length) return 'move 1';
    const m = prng.sample(moves);
    let s = `move ${m.slot}`;
    if (request.active.length > 1) {
      if (['normal', 'any', 'adjacentFoe'].includes(m.target)) {
        s += (m.target !== 'adjacentFoe' && hasAlly && prng.random() < 0.05) ? ` -${(i ^ 1) + 1}` : ` ${1 + prng.random(2)}`;
      }
      if (m.target === 'adjacentAlly') s += ` -${(i ^ 1) + 1}`;
      if (m.target === 'adjacentAllyOrSelf') s += hasAlly && prng.random(2) ? ` -${(i ^ 1) + 1}` : ` -${i + 1}`;
    }
    if (canMega && active.canMegaEvo && prng.random() < 0.3) {
      canMega = false;
      s += ' mega';
    }
    return s;
  }).join(', ');
}

// ---------------------------------------------------------------------------------------------
// One battle

function snapshotRequests(battle) {
  return battle.sides.map(s => (s.activeRequest ? JSON.parse(JSON.stringify(s.activeRequest)) : null));
}

/** A comparison of one decision step: draws, normalized state, protocol and requests. */
function makeComparer(battle, semantic, verbose) {
  let logPos = battle.log.length;
  return (bt, stepIndex, turn, choices, {skipProtocol = false} = {}) => {
    const sd = semantic.take();
    const log = normalizeLog(battle.log.slice(logPos));
    logPos = battle.log.length;
    const step = {i: stepIndex, turn, choices};
    if (sd.ties.some(t => t.distinct) || (bt.ties || []).some(t => t.distinct)) step.tieAffected = true;
    const draws = diffDraws(sd.draws, bt.draws || []);
    if (draws.length) step.draws = draws;
    if (bt.rawDraws) step.rawDraws = bt.rawDraws;
    const state = diffJson(normalize(battle), bt.state);
    if (state.length) step.state = state;
    if (!skipProtocol) {
      const protocol = bt.translateError ? {translateError: bt.translateError} : diffProtocol(log, bt.protocol || []);
      if (protocol) step.protocol = protocol;
    }
    const showdownRequests = snapshotRequests(battle);
    const requests = diffJson(showdownRequests, bt.requests);
    if (requests.length) {
      step.requests = requests;
      // The move behind a move-slot difference, for triage.
      for (const r of requests) {
        const m = /^\.(\d)\.active\.(\d)\.moves\.(\d)\./.exec(r.path);
        if (m) r.move = showdownRequests[m[1]]?.active?.[m[2]]?.moves?.[m[3]]?.id;
      }
    }
    if (verbose) {
      step.showdownLog = log;
      step.battlerProtocol = bt.protocol;
      step.battlerLog = bt.log;
      step.showdownDraws = sd.draws;
      step.battlerDraws = bt.draws;
      step.showdownTies = sd.ties;
      step.battlerTies = bt.ties;
    }
    return step;
  };
}

/**
 * Plays decision steps on both engines from where they are, comparing after each, until the battle
 * ends, `done` says so, or the engines diverge. Fills `record` (steps, outcome, divergence, turns).
 */
async function playSteps(runner, battle, record, compare, {choose, fallback, done, maxSteps = 200}) {
  const comparedTurns = new Set();
  for (let i = 1; i <= maxSteps && !battle.ended; i++) {
    if (done && done(battle)) break;
    const turn = battle.turn;
    const choices = battle.sides.map((side, k) => choose(side.activeRequest, k, i));
    const fallbacks = [];
    for (const [k, c] of choices.entries()) {
      if (c && !battle.sides[k].choose(c)) {
        const error = battle.sides[k].choice.error || '';
        if (!fallback) {
          record.outcome = 'showdown-rejected';
          record.error = `Showdown rejected p${k + 1} '${c}': ${error}`;
          record.turns = comparedTurns.size;
          return record;
        }
        // A scripted choice that no longer fits the battle: play a random legal one instead.
        choices[k] = fallback(battle.sides[k].activeRequest);
        fallbacks.push(`p${k + 1} '${c}' (${error}) -> '${choices[k]}'`);
        if (choices[k] && !battle.sides[k].choose(choices[k])) {
          record.outcome = 'showdown-rejected';
          record.error = `Showdown rejected fallback p${k + 1} '${choices[k]}'`;
          record.turns = comparedTurns.size;
          return record;
        }
      }
    }
    try {
      battle.commitChoices();
    } catch (e) {
      record.outcome = 'showdown-error';
      record.error = String(e.stack || e);
      break;
    }
    const current = await runner.send({op: 'choose', choices});
    const step = compare(current, i, turn, choices);
    if (fallbacks.length) step.fallbacks = fallbacks;
    record.steps.push(step);
    if (step.tieAffected) {
      record.outcome = 'tie-affected';
      record.tieTurn = turn;
      break;
    }
    if (turn > 0) comparedTurns.add(turn);
    if (!current.ok) {
      record.outcome = 'diverged';
      record.divergence = {step: i, turn, kind: 'battler-error', error: current.error};
      break;
    }
    if (step.draws) record.drawDiffs.push(i);
    if (step.protocol) record.protocolDiffs.push(i);
    if (step.requests) record.requestDiffs.push(i);
    // The spec counts the translated requests as part of the normalized state.
    if (step.state || step.requests) {
      record.outcome = 'diverged';
      record.divergence = {step: i, turn, kind: step.state ? 'state' : 'request'};
      break;
    }
    if (current.ended !== battle.ended) {
      record.outcome = 'diverged';
      record.divergence = {step: i, turn, kind: 'ended', battler: current.ended, showdown: battle.ended};
      break;
    }
  }
  if (record.outcome === 'ended' && !battle.ended && !(done && done(battle))) record.outcome = 'step-limit';
  record.turns = comparedTurns.size;
  return record;
}

function newRecord(fields) {
  return {...fields, steps: [], turns: 0, outcome: 'ended', divergence: null, drawDiffs: [], protocolDiffs: [], requestDiffs: []};
}

/**
 * Plays one battle on both engines. `plan` gives the choices: a function (request, side index,
 * step) → choice string. Returns the per-battle record.
 */
async function playBattle(runner, {name, seed, teams, choose, fallback, done, maxSteps = 200, verbose}) {
  const record = newRecord({name, seed, teams});
  const semantic = new SemanticRandom(seed);
  const battle = new Battle({formatid: FORMAT, seed: [1, 2, 3, 4]});
  hooks.attach(battle, semantic);
  const started = await runner.send({op: 'start', seed, teams});
  if (!started.ok) {
    record.outcome = 'setup-error';
    record.error = started.error;
    return record;
  }
  try {
    battle.setPlayer('p1', {name: 'p1', team: teams[0]});
    battle.setPlayer('p2', {name: 'p2', team: teams[1]});
  } catch (e) {
    record.outcome = 'showdown-error';
    record.error = String(e.stack || e);
    return record;
  }
  const compare = makeComparer(battle, semantic, verbose);

  // Team preview.
  const step = compare(started, 0, 0, null);
  record.steps.push(step);
  if (step.state || step.requests) {
    record.outcome = 'diverged';
    record.divergence = {step: 0, turn: 0, kind: step.state ? 'state' : 'request'};
    return record;
  }
  await playSteps(runner, battle, record, compare, {choose, fallback, done, maxSteps});
  record.winner = battle.winner;
  return record;
}

/** Showdown's serialized battle, without the harness's own fields. */
function serialize(battle) {
  const {semantic} = battle;
  const rng = battle.prng.rng;
  delete battle.semantic;
  battle.prng.rng = {getSeed: () => '0,0,0,0', next() { throw new Error('unclassified raw PRNG draw'); }};
  try {
    return battle.toJSON();
  } finally {
    battle.semantic = semantic;
    battle.prng.rng = rng;
  }
}

/**
 * Transplants Showdown's battle at its current turn boundary into a new battler battle (and a
 * restored Showdown battle), then plays `turns` turns on both with fresh semantic streams and
 * random choices, comparing after every step.
 */
async function playSegment(runner, source, {name, seed, turns, chooser, verbose}) {
  const startTurn = source.turn;
  const state = serialize(source);
  const record = newRecord({name: `${name}@${startTurn}`, seed, startTurn});
  const semantic = new SemanticRandom(seed);
  const battle = Battle.fromJSON(JSON.stringify(state));
  hooks.attach(battle, semantic);
  const transplanted = await runner.send({op: 'transplant', seed, state});
  if (!transplanted.ok) {
    record.outcome = 'transplant-error';
    record.error = transplanted.error;
    return record;
  }
  const compare = makeComparer(battle, semantic, verbose);
  // The transplanted position itself, before any choice (no protocol lines on either side).
  const step = compare(transplanted, 0, startTurn, null, {skipProtocol: true});
  record.steps.push(step);
  if (step.state || step.requests) {
    record.outcome = 'diverged';
    record.divergence = {step: 0, turn: startTurn, kind: step.state ? 'state' : 'request'};
    record.turns = 1;
    return record;
  }
  await playSteps(runner, battle, record, compare, {
    choose: request => randomChoice(request, chooser),
    done: b => b.turn >= startTurn + turns,
  });
  return record;
}

/**
 * Plays a Showdown battle with random choices and, at every turn boundary, transplants it into
 * battler and compares a few turns from there (spec P2). Returns one record per segment.
 */
async function transplantBattle(runner, {idx, name, seed, teams, turns, verbose, maxSteps = 200}) {
  const source = new Battle({formatid: FORMAT, seed: [1, 2, 3, 4]});
  hooks.attach(source, new SemanticRandom(seed));
  source.setPlayer('p1', {name: 'p1', team: teams[0]});
  source.setPlayer('p2', {name: 'p2', team: teams[1]});
  const chooser = new PRNG([idx & 0xffff, 5, 7, 11]);
  const records = [];
  for (let i = 0; i < maxSteps && !source.ended; i++) {
    source.semantic.take();
    if (source.requestState === 'move' && source.turn >= 1 && !source.queue.list.length) {
      const segmentSeed = (seed ^ Math.imul(source.turn + 1, 0x9E3779B1)) >>> 0;
      records.push(await playSegment(runner, source, {
        name, seed: segmentSeed, turns, verbose,
        chooser: new PRNG([idx & 0xffff, source.turn & 0xffff, 13, 17]),
      }));
    }
    for (const side of source.sides) {
      const c = randomChoice(side.activeRequest, chooser);
      if (c && !side.choose(c)) throw new Error(`source battle rejected '${c}': ${side.choice.error}`);
    }
    source.commitChoices();
  }
  return records;
}

// ---------------------------------------------------------------------------------------------
// Summary

function divergenceKey(record) {
  const d = record.divergence;
  if (!d) return null;
  const step = record.steps.find(s => s.i === d.step);
  if (d.kind === 'battler-error') return `battler error: ${String(d.error).split('\n')[0].slice(0, 120)}`;
  if (d.kind === 'ended') return 'battle end differs';
  if (d.kind === 'request') return `request ${step.requests[0].path.replace(/\.\d+/g, '.N')}`;
  const first = step.state[0];
  const pathShape = first.path.replace(/\.\d+/g, '.N');
  return `state ${pathShape}`;
}

function summarize(records) {
  const s = {battles: records.length, outcomes: {}, comparedTurns: 0, divergedTurns: 0, byKind: {}, protocolFirst: {}, drawFirst: {}, transplantErrors: {}};
  for (const r of records) {
    s.outcomes[r.outcome] = (s.outcomes[r.outcome] || 0) + 1;
    s.comparedTurns += r.turns;
    if (r.outcome === 'transplant-error') {
      const key = String(r.error).split('\n')[0].replace(/p\d: "[^"]*": /, '').slice(0, 140);
      (s.transplantErrors[key] = s.transplantErrors[key] || []).push(r.name);
    }
    if (r.outcome === 'diverged') {
      s.divergedTurns++;
      const key = divergenceKey(r);
      (s.byKind[key] = s.byKind[key] || []).push(r.name);
    }
    for (const step of r.steps) {
      if (step.protocol) {
        const key = step.protocol.translateError ? `translate error: ${step.protocol.translateError.slice(0, 100)}` :
          `SD ${lineShape(step.protocol.showdown)}  |  BT ${lineShape(step.protocol.battler)}`;
        (s.protocolFirst[key] = s.protocolFirst[key] || []).push(`${r.name}#${step.i}`);
        break;
      }
    }
    for (const step of r.steps) {
      if (step.draws) {
        const key = step.draws.map(d => d.category).join(',');
        (s.drawFirst[key] = s.drawFirst[key] || []).push(`${r.name}#${step.i}`);
        break;
      }
    }
  }
  s.divergenceRate = s.comparedTurns ? s.divergedTurns / s.comparedTurns : 0;
  return s;
}

function printSummary(s) {
  const sorted = obj => Object.entries(obj).sort((a, b) => b[1].length - a[1].length);
  console.log(`battles: ${s.battles}  outcomes: ${JSON.stringify(s.outcomes)}`);
  console.log(`compared turns: ${s.comparedTurns}  diverged: ${s.divergedTurns}  rate: ${(100 * s.divergenceRate).toFixed(2)}%`);
  if (Object.keys(s.transplantErrors || {}).length) {
    console.log('transplant errors:');
    for (const [k, v] of sorted(s.transplantErrors).slice(0, 40)) console.log(`  ${String(v.length).padStart(4)}  ${k}   [${v.slice(0, 3).join(' ')}]`);
  }
  console.log('state divergences by first difference:');
  for (const [k, v] of sorted(s.byKind).slice(0, 40)) console.log(`  ${String(v.length).padStart(4)}  ${k}   [${v.slice(0, 3).join(' ')}]`);
  console.log('first draw-log difference per battle (categories):');
  for (const [k, v] of sorted(s.drawFirst).slice(0, 20)) console.log(`  ${String(v.length).padStart(4)}  ${k}   [${v.slice(0, 3).join(' ')}]`);
  console.log('first protocol difference per battle:');
  for (const [k, v] of sorted(s.protocolFirst).slice(0, 30)) console.log(`  ${String(v.length).padStart(4)}  ${k}   [${v.slice(0, 3).join(' ')}]`);
}

// ---------------------------------------------------------------------------------------------

async function main() {
  const args = parseArgs(process.argv);
  fs.mkdirSync(args.out, {recursive: true});
  if (args.merge) {
    // Summarizes earlier runs (for example, chunks run in parallel with --start) together.
    const merged = args.merge.flatMap(dir => fs.readFileSync(path.join(dir, 'battles.jsonl'), 'utf8')
      .split('\n').filter(Boolean).map(line => JSON.parse(line)));
    const summary = summarize(merged);
    fs.writeFileSync(path.join(args.out, 'summary.json'), JSON.stringify(summary, null, 1));
    printSummary(summary);
    return;
  }
  const runner = new Runner(args.runner);
  const records = [];
  const out = fs.createWriteStream(path.join(args.out, 'battles.jsonl'));

  if (args.scenarios !== null) {
    const {scenarios} = require('./scenarios');
    for (const sc of scenarios()) {
      if (args.scenarios && !sc.name.includes(args.scenarios)) continue;
      const teams = sc.teams.map(t => Teams.pack(t));
      // Scripted choices answer each side's move requests in order; forced switches are answered
      // with a seeded random legal choice.
      const queues = [0, 1].map(k => sc.turns.map(turn => turn[k]));
      const auto = new PRNG([sc.seed || 1, 3, 5, 7]);
      const record = await playBattle(runner, {
        name: sc.name, seed: sc.seed || 1, teams, verbose: args.verbose,
        choose: (request, k) => {
          if (!request || request.wait) return '';
          if (request.teamPreview) return `team ${sc.picks[k]}`;
          if (request.forceSwitch) return randomChoice(request, auto);
          return queues[k].shift() || '';
        },
        fallback: request => randomChoice(request, auto),
        done: () => queues.every(q => !q.length),
      });
      if (record.outcome === 'step-limit' || record.outcome === 'ended') record.outcome = 'complete';
      records.push(record);
      out.write(JSON.stringify(record) + '\n');
    }
  } else if (args.transplant) {
    const indices = args.battle !== null ? [args.battle] : range(args.start, args.start + args.battles - 1);
    for (const idx of indices) {
      const seed = 1000003 * (idx + 1) >>> 0;
      const {teams} = makeTeams(seed);
      const segments = await transplantBattle(runner, {
        idx, name: `battle-${String(idx).padStart(5, '0')}`, seed, teams, turns: args.transplant, verbose: args.verbose,
      });
      for (const record of segments) {
        records.push(record);
        out.write(JSON.stringify(record) + '\n');
      }
      if ((idx - args.start + 1) % 25 === 0) process.stderr.write(`${idx - args.start + 1} battles, ${records.length} segments\n`);
    }
  } else {
    const indices = args.battle !== null ? [args.battle] : range(args.start, args.start + args.battles - 1);
    for (const idx of indices) {
      const seed = 1000003 * (idx + 1) >>> 0;
      const {teams, speedTies} = makeTeams(seed);
      const chooser = new PRNG([idx & 0xffff, 5, 7, 11]);
      const record = await playBattle(runner, {
        name: `battle-${String(idx).padStart(5, '0')}`, seed, teams, verbose: args.verbose,
        choose: request => randomChoice(request, chooser),
      });
      if (speedTies) record.teamSpeedTies = true;
      records.push(record);
      out.write(JSON.stringify(record) + '\n');
      if (records.length % 50 === 0) process.stderr.write(`${records.length} battles\n`);
    }
  }
  out.end();
  runner.close();
  const summary = summarize(records);
  fs.writeFileSync(path.join(args.out, 'summary.json'), JSON.stringify(summary, null, 1));
  printSummary(summary);
}

if (require.main === module) {
  main().catch(e => {
    console.error(e.stack || e);
    process.exit(1);
  });
}

module.exports = {makeTeams, randomChoice, playBattle, transplantBattle};
