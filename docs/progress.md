# Progress log

Spec: `champions-showdown-spec.md`. One section per phase, newest first.

## P1c — differential harness from turn 0 (done: budget holds, remaining divergences listed)

**What exists**
- `battler-prng`: an optional semantic random source. Every engine draw goes through a named
  `DrawSite` (`accuracy`, `crit`, `secondary`, `damage-roll`, `multi-hit`, `full-para`,
  `confusion-hit`, `status-turns`, `speed-tie`, `target`, `effect:<id>` for fxlang draws). Off by
  default; ordinary battles are unchanged.
- `battler-diff-runner` (binary): plays a battle from JSON lines on stdin with the semantic streams
  and answers with the draw log, speed-tie owners, translated protocol, translated requests and
  the normalized state (`src/state.rs`).
- `compat/diff` (Node, pinned Showdown in-process):
  - `driver.js` — runs both engines step by step and compares draws, state, requests and protocol.
    `cargo build --release -p battler-diff-runner`, then
    `node compat/diff/driver.js --battles N [--start K] [--out DIR] [--verbose]`,
    `--battle K` for one battle, `--scenarios [name]` for the spec scenarios, and
    `--merge DIR,DIR,...` to summarize chunks run in parallel.
  - `hooks.js` — classifies every Showdown draw by its caller; an unknown call site throws.
  - `state.js` — Showdown's side of the normalized state (shape shared with `state.rs`).
  - `scenarios.js` — the 15 scenarios listed in the spec.
  - `show.js` (print a battle or step), `replay.js` (Showdown's damage calculation per hit),
    `check-orders.js` (residual orders of both engines' data).
- A turn where either engine breaks a speed tie is **tie-affected**; the comparison of that battle
  ends there (spec). Random targets that Showdown picks while ordering actions and then discards
  are not compared.

**Result** (seeded random-choice battles with Champions random-doubles teams, level 50):

| Sample | Battles | Compared turns | Diverged turns | Rate |
|---|---|---|---|---|
| battles 0–2399 | 2400 | 25,936 | 28 | 0.11% |

Outcomes: 2060 ended, 311 tie-affected, 28 diverged, 1 showdown-rejected. Scenarios: 15 run, no
state divergence (2 end early on a speed tie). The rate went from 6.3% at the start of the phase to
the figure above. Battles 1200–2399 were held out until the end and diverged in 0.21% of turns on
their first run; their divergences were then fixed or classified too, so they are no longer an
unseen sample.

**Fixed, by frequency** (Showdown is the reference; details in the P1c commits):
- Turn order and timing: residual orders and sub-orders copied from Showdown and default
  sub-orders by effect kind; durations count down only at their own residual; Mons faint between
  residual effects (but weather hits everyone first); fainted Mons' residual effects are skipped;
  priority and speed recalculated before each move (Gen 8+); mid-turn switch-outs (U-turn, Eject
  Button) happen when the replacement comes in; Emergency Exit checks after the residual phase;
  Champions Encore keeps the replaced move's place in the queue.
- Damage: Showdown's 4096-based modifier chaining and rounding (base power and damage modifiers,
  burn, Parental Bond, screens on every hit, protection-weakened hits, Heal Pulse, weather healing
  moves); Body Press uses attack modifiers; confusion damage uses unmodified stats; Unaware per
  target; multi-hit counts drawn after accuracy.
- Effects: Pressure, Poison Touch, Toxic Chain, Sticky Web (Clear Body, Defiant), Rest replacing a
  status, Telepathy, Sturdy, Charge, Beak Blast, King's Shield family vs status moves, Stone Axe and
  Ceaseless Edge on the foe side, Strength Sap vs Liquid Ooze, Reckless vs Struggle, choice lock
  after Knock Off, secondary effects and self-boosts on a substitute hit, a move that only hit a
  substitute still counting as used (Life Orb), Magic Bounce clearing Prankster on the reflected
  move, Parting Shot switching out after Mirror Armor, Unaware in accuracy checks, Disguise still
  triggering after-hit effects, Transform copying boosts before the new ability starts, Wandering
  Spirit on a knockout, Psychic Noise's 2-turn Heal Block, Thunder/Hurricane in sun (an fxlang
  assignment that failed), and many residual details (Harvest, Hydration, Wish, Shed Skin).
- Data typos found on the way: Bug type chart row, Rocky Helmet, Whirlwind and Signal Beam effect
  fields, Banettite's species.
- A panic: a foe-side move (Stealth Rock) used after every foe fainted.
- Translation (`battler-showdown-compat`): requests (base species and ability, Struggle, Mega
  options, revival, transformed Mon's own stats), status and cure attributions. Pollen Puff's
  request target is `adjacentFoe` under Heal Block (engine data, as for Curse).

**Remaining state divergences, with causes** (battles 0–2399; 28 turns):
- 14 × request `maybeTrapped` (108, 433, 584, 658, 802, 912, 963, 1515, 1585, 1669, 2095, 2102,
  2103, 2324): Showdown marks a Pokémon `maybeTrapped` when a foe's species could have a trapping
  ability it has not shown (Arena Trap, Magnet Pull, Shadow Tag), so a cancelled switch leaks
  nothing. battler's requests only report real traps. Request only; the battle state matches.
- 4 × `encore` (657, 838, 1415, 2365): Showdown records a Pokémon's last move before the move runs,
  battler after. Encore reflected by Magic Bounce fails in Showdown (the user's last move is already
  Encore) but encores the user's previous move in battler (1415, 2365); 657 and 838 are the same
  timing, picking a different move. Fixing it means moving the record ahead of the before-move
  checks in `use_active_move`, which is wider than four turns justify.
- 3 × `twoturnmove` (99, 572, 750): the battle ends during the turn a Pokémon starts charging
  Solar Beam; Showdown's final state still holds the charging volatile, battler's does not. Final
  step only.
- 2 × `species` (17, 470): Revival Blessing on a Pokémon that fainted earlier in the same turn and
  has not been replaced: Showdown sends it straight back into its empty slot; battler leaves the
  slot empty until the next request.
- 2 × `unburden` (136, 2260): Showdown keeps the `unburden` volatile after Trick or Switcheroo gives
  the Pokémon a new item and only applies the Speed boost while it holds nothing; battler removes
  the volatile when the item arrives. Same Speed either way (representation only).
- 1 × `lockedmove` (327): an Outrage turn that misses. battler reads the move's outcome before it
  is recorded, so the lock lasts one turn longer than in Showdown.
- 1 × `status` (1152): battler cures a Natural Cure holder when the battle ends (as if it switched
  out); Showdown does not. Final step only; battler's behaviour is intentional.
- 1 × `boosts.def` (1354): Clanging Scales knocks out the last foe; Showdown ends the battle before
  the user's Defense drop, battler applies it first. Final step only.

**Other differences (outside the state budget)**
- Draw logs: the `target` stream differs in 133 of 2400 battles (5.5%) because Showdown picks a
  random target for a move chosen without one when it resolves the action, and again while sorting
  the queue; battler picks once, when the move is used. The picks agree whenever state matches.
- Protocol: the most frequent first differences are translation gaps, e.g. `[still]` on self-
  and field-targeted moves (`|move|...|Trick Room||[still]`), `-end|...|Encore` vs
  `move: Encore`, `[from] Stealth Rock` vs `move: Stealth Rock`, Bad Poison damage `[from] psn`,
  form changes with `[from] ability:` and Mimikyu's Disguise lines. They do not affect state.

**Test expectation changes** (each to Showdown's behaviour):
- Residual order and timing: many residual logs; Emergency Exit now activates after all residual
  effects (`emergency_exit_activates_after_residuals`, renamed).
- Mid-turn switch-outs no longer log a separate `switchout` line (~30 tests: U-turn, Baton Pass,
  Eject Button/Pack, Emergency Exit, Parting Shot, Shed Tail, ...).
- Rounding: Heal Pulse, Shore Up, Parental Bond, Z-Move through Protect (±1 HP).
- Random sequence: drawing the hit count after accuracy re-rolls later values in accuracy,
  focus energy, metronome, multihit, rage and Water Shuriken tests (same assertions); the
  affection and substitute tests move a forced value or seed so they still show what they test.
- Behaviour: Shell Bell does not activate when only a substitute is hit (renamed test); Rapid Spin
  raises Speed through a substitute; Sticky Web names the foe as the source; transformed abilities
  start after the `transform` line; Bug type chart; Bad Poison and healing amounts; Refrigerate.
- New tests: Champions Encore ordering, Thunder accuracy in sun, Strength Sap vs Liquid Ooze,
  Psychic Noise, Stealth Rock after every foe fainted, every Mega Stone's species, Wandering Spirit
  on a knockout, Life Orb after only hitting a substitute, Magic Bounce vs Prankster, Parting Shot
  vs Mirror Armor, Pollen Puff under Heal Block, Unaware vs accuracy boosts.

**Test status**: `cargo test -q -p battler -p battler-showdown-compat -p battler-champions
-p battler-prng -p battler-state -p battler-service -p battler-client -p battler-diff-runner` all
pass (battler: 1436). `battler-ai` does not build in this container (its build script needs
external data); unrelated to this phase.

**Gaps**
- Tie-affected battles (311 of 2400, 13%) stop being compared at the first speed tie; re-syncing by
  transplant is P2.
- Protocol differences are listed, not budgeted; the translation gaps above should be closed
  before protocol output is relied on.
- One battle ends `showdown-rejected`: Showdown's own request offers a move its Encore then
  rejects.

**Next step**: P2 (transplant, then clone).

## P1b — Showdown compatibility layer (done, with the gaps listed below)

Crate `battler-showdown-compat` (README inside). Everything below is tested against fixtures
generated from the pinned Showdown (`compat/gen-fixtures.js`, `compat/scenarios.js`).

**Done**
- Protocol: `protocol::Translator` handles every line type in the spec (switch, drag, replace,
  -damage, -heal, -sethp, faint, move, cant, detailschange, -formechange, -mega, -status,
  -curestatus, the boost family, swap, side/field/weather start and end, -start/-end, -item,
  -enditem, -ability, -activate, -fail, -miss, -immune, -crit, -supereffective, -resisted, turn,
  win, tie, upkeep) with `[from]`/`[of]`/`[silent]`/`[upkeep]`/`[spread]`/`[still]`/`[miss]`/
  `[notarget]` and `|split|pN` pairs. 13 scenarios (doubles, Showdown's constant-0 or constant-max
  random source on both engines) match byte for byte; every spec line type except `tie` and all
  8 tags appear in them. `|tie` cannot happen by fainting in gen 9 (the last to faint wins), so it is
  tested as a single line.
- Choices: `choice::translate` (1-based → 0-based, `default`, team order, mega, switch/pass).
- Requests: `request::translate` matches Showdown's request JSON (team preview, move requests with
  pp/maxpp/target/disabled/trapped/canMegaEvo, forceSwitch, noCancel, wait, `side.pokemon`) at every
  step of every fixture, for both players.
- Teams: packed and exported formats → `TeamData`; species/forme mapping incl. cosmetic formes.
- `compat/allowlist.md`: three rules (R1 spread/side-move target, R2 `-hint`, R3 `[notarget]`
  target), each with the reason. Nothing else is ignored.

**Engine fixes found on the way** (all with Showdown as the reference): Close Combat dropped the
wrong stat, Shift Gear boosted Atk by the wrong amount, 33% recoil moves recoiled by 1/3, recoil on
zero damage, user effects (Life Orb) after a missed move, fainted slots did not swap team position
with their replacement (affects `side.pokemon` order), `[still]` handling for stopped moves, and
supereffective/resisted stages under the Champions rules. Tests for those moves were updated to
Showdown's numbers.

**Test status**: `cargo test -q -p battler -p battler-data -p battler-test-utils -p battler-champions
-p battler-showdown-compat` all pass (battler: 1425). `cargo check --workspace --all-targets` clean.

**Gaps**
- Scenarios are doubles only and short; singles, multi-turn volatiles and many abilities are not
  covered by a reference fixture. The random-battle corpus (40 battles, informational, run with
  `--ignored`) matches exactly in only 1 battle; the first differences are engine behaviour, not
  translation (list in `compat/allowlist.md`).
- Trick/Switcheroo, Leech Seed, Pain Split, Spectral Thief and the herbs are translated by special
  cases because battler logs them differently; other multi-line Showdown idioms will need the same
  as P1c finds them.
- `request::translate` treats Mega/Primal/Ultra as the only permanent formes.

**Open divergences** (engine): speed-tie order (Showdown shuffles), White Herb vs faint order,
Taunt/Yawn residual order, plus the corpus list above.

**Next step**: P1c (differential harness), starting from the corpus first-difference list.

## P1a — Champions rules (done, with the gaps listed below)

Branch: `claude/compassionate-albattani-8i2vor`. Done in the same session as P0 because the request
asked for both; the spec's "one phase per session" was not followed.

**What exists**
- New crate `battler-champions` (see its README):
  - `ChampionsDataStore`: a `DataStore` overlay (JSON merge patches in `data/mc/`) over moves,
    abilities, items and the `par`/`slp`/`frz` conditions. `Regulation::{MC, MB}`; MB currently has no
    layer of its own and behaves like MC.
  - `legality::Legality`: species, move, ability and item availability exported from Showdown.
  - `rules::{engine_options, format_rules}`.
- `battler` gained `ChampionsRules` (`CoreBattleEngineOptions::champions`, all off by default):
  stat points, PP formula, public health, Mega persists on faint, attack counter reset and per-hit
  counting, Sheer Force keeps after-secondary events, Parental Bond single-hit log. Mainline behavior
  is unchanged unless a toggle is set.
- `TestBattleBuilder::with_champions_rules()`.
- Verification against Showdown (exports under `battler-champions/tests/data`, regenerated by
  `scripts/export-champions-*.js`):
  - every Champions-delta field of every move that exists in battler (base power, accuracy, PP, type,
    target, flags, secondaries) matches;
  - 180 random stat-point sets × natures match Showdown's stats exactly;
  - max PP matches for every Showdown-legal move that battler has data for (over 400);
  - public health matches for 367 (HP, max HP) pairs;
  - behavior tests compare Champions with mainline on the same scenario for paralysis, sleep, freeze,
    Fake Out / First Impression, Belch / Stuff Cheeks, Salt Cure, Trick Room, Healer, Eject Button,
    Run Away, Emergency Exit, Berserk, Curse, Unseen Fist, Rage Fist, Mega on faint, Sheer Force,
    Parental Bond.

**Test status**: `cargo test -q -p battler -p battler-data -p battler-test-utils -p battler-champions`:
all pass (battler 1425 integration + 102 lib tests, battler-champions 36). `cargo check --workspace
--all-targets`, `cargo check -p battler --features typescript` and `cargo build -p battler
--no-default-features` succeed.

**Gaps (not implemented; none are checked by a passing test)**
- `nihillight` is a Champions move missing from battler's data; its 5 PP is unpatched.
- Disable: Showdown skips the "can't use" check for moves with the `cantusetwice` flag. battler models
  Gigaton Hammer / Blood Moon with a condition, not a flag, so this is not mirrored.
- Encore: Showdown rewrites the target's queued action when Encore lands (no Mental Herb);
  battler overrides the move at execution. Queue priority can differ.
- Curse (Ghost): Showdown's `onHit` re-targets an ally target to a random foe and re-adds the volatile;
  battler re-targets before the hit only.
- Emergency Exit / Wimp Out: Showdown sets a switch flag (the switch happens after the move, other
  switch flags are untouched); battler switches out immediately. Other Pokémon's pending switches are
  no longer cleared.
- Eject Button: same immediate-switch difference.
- Protocol-level differences (silent Natural Cure / Regenerator, `-zbroken`, `0 fnt`, 4x/0.25x effectiveness
  messages) belong to P1b; the battle state is already the same.
- `Level Clause Mod`, Species/learnset legality in the validator (only the availability lists exist),
  per-stat limit of 32 stat points (only the total, via `EV Limit = 66`), item/Mega-stone rules beyond the
  existing data.
- Reg M-B (`championsregmb`) is untouched.
- Trick Room uses `65535 - speed` instead of signed speed; ordering is identical, but action speed
  values other effects might read differ from Showdown's.
- The calculator (`battler-calc`) does not know the Champions rules.

**Open divergences**: not measured; the differential harness is P1c.

**Next step**: P1b (Showdown compatibility layer), then P1c (differential harness), which will find
whatever the gaps above and the untested interactions cost.

## P0 — toolchain and baseline benchmarks (done)

Branch: `claude/compassionate-albattani-8i2vor` (the session's designated branch, used instead of `p0-bench`).

**Done**
- `rust-toolchain.toml` pins `nightly-2026-09-25` (rustc 1.100.0-nightly, f7575a9da 2026-09-24) with
  `rustfmt` and `rust-src`. `.github/actions/rust` no longer asks for floating `nightly`, so CI follows
  the file; `published-crates-tests.yml` pins the same name.
- `cargo build --workspace --release` succeeds on the pin. `battler-ai`'s build script needs
  `pyinstaller` (`GEMINI_PYTHON_VENV=/usr/local` here); this is unrelated to the toolchain.
- `scripts/fetch-showdown.sh` clones and builds Showdown at `3661ce40…` into `third_party/` (gitignored).
- 200 positions in `bench/positions/` (`bench/showdown/gen-positions.js`, deterministic), plus
  `bench/calc-probes.json`.
- Benchmarks (a), (b), (c) for Showdown (`bench/showdown/bench.js`) and battler + battler-calc
  (`bench/battler-bench`). Results: `docs/benchmarks/p0.md` and the raw JSON beside it.

**Test status**: `cargo test -q -p battler` on the pinned nightly: 1425 passed, 0 failed (plus 2 ignored doctests). Other crates' tests were not re-run in P0.

**Caveats worth knowing before P1/P2**
- battler has no restore, so its "restore" and rollouts are fresh battles from turn 1 with the same
  teams, not the same positions.
- Showdown's Champions random-doubles sets carry stat points 11/11/11/11/11/11 in `evs`; the battler
  conversion uses EV = max(8·SP − 4, 0) (the spec's mapping), unvalidated until P1a.
- Species-name differences between Showdown and battler (`-F` forms, Tauros-Paldea, Squawkabilly,
  Maushold-Four, apostrophes, some cosmetic forms) are mapped by hand in the bench crate; unmapped
  names make battler-calc error on some probes (counts in `p0.md`). Real mapping belongs to P1b.
- Timings come from a shared 4-vCPU VM and are noisy.

**Open divergences**: none measured yet (no differential harness until P1c).

**Next step**: P1a (Champions rules overlay).
