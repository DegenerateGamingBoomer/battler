'use strict';
// Prints one battle (or one step) of a driver run.
// Usage: node compat/diff/show.js <battles.jsonl> <battle-name> [step]
const fs = require('fs');

const [file, name, stepArg] = process.argv.slice(2);
const record = fs.readFileSync(file, 'utf8').split('\n').filter(Boolean).map(JSON.parse)
  .find(r => r.name === name);
if (!record) throw new Error(`no battle ${name}`);
console.log(`${record.name} seed=${record.seed} outcome=${record.outcome} turns=${record.turns}` +
  (record.divergence ? ` divergence=${JSON.stringify(record.divergence)}` : '') +
  (record.error ? ` error=${record.error}` : ''));
const show = v => (typeof v === 'string' ? v : JSON.stringify(v));
for (const step of record.steps) {
  if (stepArg !== undefined && step.i !== Number(stepArg)) continue;
  console.log(`--- step ${step.i} (turn ${step.turn}) choices=${JSON.stringify(step.choices)}${step.tieAffected ? ' TIE' : ''}`);
  for (const d of step.state || []) console.log(`  state ${d.path}: SD ${show(d.showdown)} | BT ${show(d.battler)}`);
  for (const d of (step.requests || []).slice(0, 8)) console.log(`  request ${d.path}: SD ${show(d.showdown)} | BT ${show(d.battler)}`);
  for (const d of step.draws || []) console.log(`  draws ${d.category}: SD [${d.showdown.join(', ')}] | BT [${d.battler.join(', ')}]`);
  if (step.protocol) {
    if (step.protocol.translateError) console.log(`  protocol: translate error ${step.protocol.translateError}`);
    else {
      for (const c of step.protocol.context) console.log(`     ${c}`);
      console.log(`  protocol SD: ${step.protocol.showdown}\n  protocol BT: ${step.protocol.battler}`);
    }
  }
  if (step.rawDraws) console.log(`  raw draws: ${step.rawDraws}`);
  if (step.showdownTies && (step.showdownTies.length || step.battlerTies.length)) {
    console.log(`  ties SD ${JSON.stringify(step.showdownTies)} | BT ${JSON.stringify(step.battlerTies)}`);
  }
  if (stepArg !== undefined && step.showdownLog) {
    console.log('  showdown log:');
    for (const l of step.showdownLog) console.log(`    ${l}`);
    console.log('  battler protocol:');
    for (const l of step.battlerProtocol || []) console.log(`    ${l}`);
    console.log(`  showdown draws: ${step.showdownDraws.map(d => `${d.c}:${d.site}:${d.k}${d.p !== undefined ? ` ${d.p}/` : ' '}${d.n}=${d.r}`).join(' ')}`);
    console.log(`  battler draws:  ${step.battlerDraws.map(d => `${d.c}:${d.site}:${d.k}${d.p !== undefined ? ` ${d.p}/` : ' '}${d.n}=${d.r}`).join(' ')}`);
  }
}
