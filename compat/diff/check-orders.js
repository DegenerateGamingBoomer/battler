'use strict';
// Compares the residual order, sub-order and priority of every effect battler and the pinned
// Showdown both define: moves' conditions, abilities, items and conditions. Prints one line per
// mismatch. An order Showdown leaves unset means "last", like a battler callback without one.
// Usage: node compat/diff/check-orders.js [--json] [event-filter]
// With --json, prints the mismatches as JSON lines ({kind, id, event, order, subOrder, priority,
// durationOnly, missing}) for scripts that apply them.
const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '../..');
const SD = process.env.SHOWDOWN_DIR || path.join(ROOT, 'third_party/pokemon-showdown');
const {Dex} = require(path.join(SD, 'dist/sim'));
const dex = Dex.mod('champions');
const json = process.argv.includes('--json');
const filter = process.argv.slice(2).find(a => a !== '--json');
function report(kind, id, event, text, data) {
  if (json) console.log(JSON.stringify({kind, id, event, ...data}));
  else console.log(`${kind} ${id}: ${text}`);
}

function mergePatch(target, patch) {
  for (const [k, v] of Object.entries(patch)) {
    if (k.startsWith('+')) {
      target[k.slice(1)] = [...(target[k.slice(1)] || []), ...v];
      continue;
    }
    if (v === null) delete target[k];
    else if (v && typeof v === 'object' && !Array.isArray(v) && target[k] && typeof target[k] === 'object' && !Array.isArray(target[k])) mergePatch(target[k], v);
    else target[k] = v;
  }
  return target;
}

function load(kind, overlayName) {
  const out = {};
  const p = path.join(ROOT, 'battle-data/data', kind);
  const files = fs.statSync(p).isDirectory() ? fs.readdirSync(p).map(f => path.join(p, f)) : [p];
  for (const f of files) Object.assign(out, JSON.parse(fs.readFileSync(f, 'utf8')));
  const overlay = path.join(ROOT, 'battler-champions/data/mc', overlayName);
  if (fs.existsSync(overlay)) {
    for (const [id, patch] of Object.entries(JSON.parse(fs.readFileSync(overlay, 'utf8')))) {
      out[id] = out[id] ? mergePatch(out[id], patch) : patch;
    }
  }
  return out;
}

// Showdown callback → battler callback, for the residual events.
const EVENTS = {
  onResidual: 'on_residual',
  onFieldResidual: 'on_field_residual',
  onSideResidual: 'on_side_residual',
  onSlotResidual: 'on_slot_residual',
};

function battlerOrders(callbacks, name) {
  const cb = callbacks && callbacks[name];
  if (cb === undefined) return undefined;
  if (Array.isArray(cb) || typeof cb === 'string') return {order: undefined, sub: undefined, priority: undefined};
  return {order: cb.order, sub: cb.sub_order, priority: cb.priority};
}

function compare(kind, id, sdEffect, btCallbacks, hasDuration) {
  for (const [sdName, btName] of Object.entries(EVENTS)) {
    if (filter && !sdName.toLowerCase().includes(filter.toLowerCase())) continue;
    const sdHas = typeof sdEffect[sdName] === 'function';
    const sdOrder = sdEffect[`${sdName}Order`];
    const sdSub = sdEffect[`${sdName}SubOrder`];
    const sdPriority = sdEffect[`${sdName}Priority`];
    // An order without a callback still orders the duration countdown (and the end).
    if (!sdHas && sdOrder === undefined && sdSub === undefined) continue;
    const bt = battlerOrders(btCallbacks, btName);
    if (!bt) {
      // battler counts durations down in a default callback without an order.
      if (!sdHas && hasDuration && sdOrder === undefined) continue;
      report(kind, id, btName, `${sdName} missing in battler (Showdown order ${sdOrder} sub ${sdSub}${sdHas ? '' : ', duration only'})`,
        {order: sdOrder, subOrder: sdSub, priority: sdPriority, durationOnly: !sdHas, missing: true});
      continue;
    }
    const diffs = [];
    if ((sdOrder || undefined) !== (bt.order || undefined)) diffs.push(`order SD ${sdOrder} BT ${bt.order}`);
    if ((sdSub || undefined) !== (bt.sub || undefined)) diffs.push(`sub SD ${sdSub} BT ${bt.sub}`);
    if ((sdPriority || undefined) !== (bt.priority || undefined)) diffs.push(`priority SD ${sdPriority} BT ${bt.priority}`);
    if (diffs.length) {
      report(kind, id, btName, `${btName} ${diffs.join(', ')}`, {order: sdOrder, subOrder: sdSub, priority: sdPriority, durationOnly: !sdHas, missing: false});
    }
  }
}

const moves = load('moves', 'moves.json');
for (const [id, data] of Object.entries(moves)) {
  const sd = dex.moves.get(id);
  if (!sd.exists || !sd.condition) continue;
  const condition = data.condition || {};
  compare('move', id, sd.condition, condition.callbacks, !!sd.condition.duration);
}
const abilities = load('abilities', 'abilities.json');
for (const [id, data] of Object.entries(abilities)) {
  const sd = dex.abilities.get(id);
  if (!sd.exists) continue;
  compare('ability', id, sd, (data.effect || {}).callbacks, false);
  if (sd.condition) compare('ability-condition', id, sd.condition, (data.condition || {}).callbacks, !!sd.condition.duration);
}
const items = load('items', 'items.json');
for (const [id, data] of Object.entries(items)) {
  const sd = dex.items.get(id);
  if (!sd.exists) continue;
  compare('item', id, sd, (data.effect || {}).callbacks, false);
  if (sd.condition) compare('item-condition', id, sd.condition, (data.condition || {}).callbacks, !!sd.condition.duration);
}
const conditions = load('conditions.json', 'conditions.json');
for (const [id, data] of Object.entries(conditions)) {
  const sd = dex.conditions.get(id);
  if (!sd.exists) continue;
  compare('condition', id, sd, (data.condition || data.effect || {}).callbacks, !!sd.duration);
}
