'use strict';
// Normalized battle state of a Showdown battle (spec P1c, "What is compared each turn").
//
// The shape is shared with battler-diff-runner/src/state.rs. Showdown's IDs are the canonical
// names; the battler side maps its own names onto them. Keys that are absent mean "none".

const toID = s => String(s || '').toLowerCase().replace(/[^a-z0-9]/g, '');

// Per-volatile counters that are compared besides `duration`.
function volatileState(id, state) {
  const out = {};
  if (typeof state.duration === 'number') out.duration = state.duration;
  switch (id) {
  case 'confusion': out.time = state.time; break;
  case 'substitute': out.hp = state.hp; break;
  case 'stall': out.counter = state.counter; break;
  case 'encore': case 'disable': case 'choicelock': case 'twoturnmove': case 'lockedmove':
    out.move = toID(state.move); break;
  case 'stockpile': out.layers = state.layers; break;
  case 'partiallytrapped': out.source = toID(state.sourceEffect && state.sourceEffect.id); break;
  }
  return out;
}

function statusState(pokemon) {
  const s = pokemon.statusState || {};
  switch (pokemon.status) {
  case 'slp': return {time: s.time};
  case 'tox': return {stage: s.stage};
  case 'frz': return typeof s.time === 'number' ? {time: s.time} : {};
  default: return {};
  }
}

function boosts(pokemon) {
  const out = {};
  for (const [stat, value] of Object.entries(pokemon.boosts)) if (value) out[stat] = value;
  return out;
}

function active(pokemon) {
  if (!pokemon || pokemon.fainted) return null;
  // Waiting for a mid-turn switch (U-turn, Eject Button, ...): Showdown keeps the Pokémon in its
  // slot until the replacement comes in, battler has already taken it out.
  // Revival Blessing's "switch" only picks the Pokémon to revive; its user stays in.
  if (pokemon.switchFlag && pokemon.switchFlag !== 'revivalblessing') {
    return {name: pokemon.name, switching: true};
  }
  const volatiles = {};
  for (const [id, state] of Object.entries(pokemon.volatiles)) {
    // Showdown links a trapping move's user to its target with a helper volatile; battler ends
    // the trap when the user leaves without one.
    if (id === 'trapper') continue;
    volatiles[id] = volatileState(id, state);
  }
  return {
    name: pokemon.name,
    species: pokemon.species.id,
    hp: pokemon.hp,
    maxhp: pokemon.maxhp,
    status: pokemon.status || '',
    statusState: statusState(pokemon),
    boosts: boosts(pokemon),
    volatiles,
    activeTurns: pokemon.activeTurns,
    moveActions: pokemon.activeMoveActions,
  };
}

function member(pokemon) {
  return {
    species: pokemon.species.id,
    hp: pokemon.hp,
    maxhp: pokemon.maxhp,
    // A fainted Pokémon keeps its status in Showdown; battler replaces it with `fnt`.
    status: pokemon.fainted ? '' : pokemon.status || '',
    fainted: !!pokemon.fainted,
    item: pokemon.item || '',
    ability: pokemon.ability || '',
    pp: pokemon.moveSlots.map(m => ({id: m.id, pp: m.pp, maxpp: m.maxpp})),
  };
}

function conditions(map, turn) {
  const out = {};
  for (const [id, state] of Object.entries(map)) {
    // Showdown's marker for a pending Revival Blessing choice, not a condition in battle.
    if (id === 'revivalblessing') continue;
    const entry = {};
    if (typeof state.duration === 'number') entry.duration = state.duration;
    // Wish records the turn it was used and resolves at the end of the next turn: a two-turn
    // duration counted down at each residual.
    if (id === 'wish' && typeof state.startingTurn === 'number') {
      entry.duration = state.startingTurn + 1 - turn.turn + (turn.residualDone ? 0 : 1);
    }
    if (typeof state.layers === 'number') entry.layers = state.layers;
    out[id] = entry;
  }
  return out;
}

function normalize(battle) {
  const field = battle.field;
  const pseudo = {};
  for (const [id, state] of Object.entries(field.pseudoWeather)) {
    pseudo[id] = typeof state.duration === 'number' ? {duration: state.duration} : {};
  }
  return {
    turn: battle.turn,
    field: {
      weather: field.weather || '',
      weatherDuration: field.weather && typeof field.weatherState.duration === 'number' ? field.weatherState.duration : null,
      terrain: field.terrain || '',
      terrainDuration: field.terrain && typeof field.terrainState.duration === 'number' ? field.terrainState.duration : null,
      pseudo,
    },
    sides: battle.sides.map(side => ({
      conditions: conditions(side.sideConditions),
      slotConditions: side.slotConditions.slice(0, side.active.length).map(map => conditions(map, {
        turn: battle.getOverflowedTurnCount(),
        // A request after this turn's residual (a replacement for a Pokémon that fainted to it).
        residualDone: battle.midTurn && !battle.queue.list.some(action => action.choice === 'residual'),
      })),
      active: side.active.map(p => active(p)),
      team: Object.fromEntries(side.pokemon.map(p => [p.name, member(p)])),
    })),
  };
}

module.exports = {normalize, toID};
