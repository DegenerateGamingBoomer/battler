# Spec: Pokémon Champions doubles on battler, verified against Pokémon Showdown

Status: draft, written 2026-10-11 before any work in this fork.

## 1. Purpose

Make battler a fast, exact engine for **Pokémon Champions VGC doubles** (Regulation M-C), for use
inside a game-tree search. That search does two things per decision:
- it restores a position from serialized state;
- it then plays thousands of short rollouts (1-4 turns) from it.

So what matters most is the cost of four operations: restoring a position, cloning it, advancing
a turn, and running a damage calculation. Correctness means matching Pokémon Showdown exactly.

## 2. Ground truth

- **Reference:** Pokémon Showdown (`smogon/pokemon-showdown`) at commit
  `3661ce40bf9001d185ce078b8920e12304204609`.
  - Format `gen9championsvgc2026regmc` ("[Gen 9 Champions] VGC 2026 Reg M-C"), mod `champions`.
  - Node >= 22.18 (`package.json` engines). Build with `node build`, then load `dist/sim`.
- **Showdown wins every disagreement, quirks included.** Do not "fix" toward cartridge behaviour
  when Showdown differs. Record each such case in `docs/showdown-quirks.md`.
- **Regulation stays configurable.** Reg M-B (mod `championsregmb`) is a later target. Do not
  hard-code M-C.
- Both projects are MIT-licensed. Keep attribution for data transcribed from Showdown.

## 3. What battler has today (read from source at upstream commit 2126bc4, 2026-09-26)

Paths are relative to the repository root.
- **Data:** mainline Scarlet/Violet values. There is no Champions format. Z-A Mega Stones (e.g.
  `garchompitez`) and the new Mega abilities (Dragonize, Mega Sol, Piercing Drill, Spicy Spray,
  Fire Mane, Eelevate) already exist.
- **Data access** goes through the pluggable `DataStore` trait
  (`battler-data/src/datastore/data_store.rs:23`). The local implementation is
  `battler-local-data`.
- **Effects are interpreted fxlang JSON programs.** They are parsed once into a per-battle
  `EffectManager` LRU (`battler/src/effect/effect_manager.rs:68`). This parse cost recurs for
  every new battle.
- **No cloning or state loading:**
  - `CoreBattle` (`battler/src/battle/core_battle.rs:285`), `Mon`, `Side` and `Field` are not
    `Clone` or `Serialize`.
  - Handles are zone_alloc indices (`battle/registry.rs`).
  - `turn` is private. `start()` fires the leads' entry effects.
- **Logs** use their own syntax: `key:value` fields, `Name,player,position` identities, effect
  names such as `Rain` rather than `RainDance`.
  - HP lines come as `split|side:N` with a private exact line and a public line.
  - Requests are a separate `Request` enum (`battler/src/battle/request.rs:101`) with no list of
    legal targets.
  - Choices are 0-based (`move 0,2;move 2,-1`, `switch 2`, `team 0 1 2 3`;
    `battler-choice/src/lib.rs`).
  - IVs default to 0, not 31.
- **Tests:** 1,466 tests, all checking battler's own expected logs.
  - None compares against Showdown.
  - The fuzz generator only checks that battles finish.
  - Doubles gaps: no Rage Powder test; Fake Out, Trick Room and Mega tests are singles only.
- **Determinism:**
  - seeded LCG (`battler-prng`) with a pluggable `rng_factory`;
  - `ControlledRandomNumberGenerator`, which replaces the Nth call (`battler-test-utils/src/rng.rs`);
  - options for a max/min damage roll and deterministic speed ties
    (`battler/src/battle/battle_options.rs`).
  - `log_time` is on by default.
- **Toolchain:** nightly Rust (`const_trait_impl`, `pattern`, `try_trait_v2`). CI uses a
  floating `nightly`.

## 4. Phases

Do one phase per session, in order. Each phase ends with a short report in `docs/progress.md`.
Stop and report instead of continuing when a gate fails.

### P0 — toolchain and baseline benchmarks (one session; raw timings only)

1. **Pin the toolchain.** Pin a dated nightly that builds the workspace in `rust-toolchain.toml`,
   and make CI use it.
2. **Set up Showdown.** Check out the pinned commit under `third_party/` (gitignored, fetched by a
   script) or as a pinned submodule, and build it.
3. **Generate test positions.**
   - Make 200 mid-game Reg M-C positions with Showdown: random-doubles teams at level 50, played
     3-6 turns with seeded random legal choices.
   - Teams come from Showdown's `[Gen 9 Champions] Random Doubles Battle` generator
     (`data/random-battles/champions/`), forced to level 50; its sets already carry stat points
     (starting from 11 per stat, at most 66 in total).
   - Store each position as `battle.toJSON()`, plus the two teams and the seeds, under
     `bench/positions/`.
4. **Benchmark**, single-threaded, with warm-up and at least 5 repetitions. Report the median and
   p90:
   - **(a) Restore.**
     - Showdown: `Battle.fromJSON(JSON.parse(text))`, and separately `Battle.fromJSON(object)`.
     - battler: a new battle from the same teams plus `start()`, as a stand-in until P2. Report
       the first battle in a process separately from later ones, to expose the fxlang parse cost.
   - **(b) Two-turn rollouts.**
     - Showdown: restore plus two turns of random legal choices per position.
     - battler: start plus two turns with random legal choices.
     - Report time per turn and per choice.
   - **(c) Damage probes.** Time Showdown's `battle.actions.getDamage(source, target, move, true)`
     for active pairs. Separately, check whether `battler-calc` covers doubles spread, abilities,
     items, weather, terrain, screens and crits, and time it.
5. **Write the results.** Put them in `docs/benchmarks/p0.md`, with raw JSON beside it. Record the
   CPU model, core count, OS, Node and rustc versions, and both commit hashes. Draw no
   conclusions beyond the numbers.

### P1a — Champions rules

Implement every difference in Showdown's `data/mods/champions/`, ideally as a `DataStore` overlay
plus a small set of engine options, so mainline behaviour stays available. The mod contains:

| File | Contents |
| --- | --- |
| `moves.ts` | 256 entries: 190 only change availability (`isNonstandard`); **66 change behaviour**. Examples: Protect, Spiky Shield, King's Shield, Baneful Bunker, Obstruct, Wish, Strength Sap, Sandstorm and Snowscape go to 5 PP (Burning Bulwark and Silk Trap only change availability); Moonblast 10% SpA drop; Iron Head 20% flinch; Freeze-Dry no freeze; Make It Rain 95% accuracy and −2 SpA; Slash 80; Snap Trap Steel; Growth Grass type; Milk Drink targets an adjacent ally or self; slicing flags on Dragon Claw, Shadow Claw, Metal Claw, Crush Claw, Dire Claw; Dire Claw 30% random psn/par/slp; new Curse, Disable, Encore and Salt Cure conditions; Fake Out and First Impression use `activeMoveActions`; Rage Fist's counter resets on switch. |
| `items.ts` | 253 entries: 252 availability; **Eject Button** changes behaviour. |
| `abilities.ts` | 6 availability (the new Mega abilities); **9 behaviour**: Anger Shell, Berserk, Emergency Exit, Wimp Out, Healer, Natural Cure, Regenerator, Run Away, Unseen Fist (contact moves go through Protect at 0.25x and log `-zbroken`). |
| `conditions.ts` | Paralysis: full paralysis 1/8. Sleep: `sample([2,3,3])` turns. Freeze: at most 3 turns, 1/4 thaw. |
| `scripts.ts` | `init` caps base PP at 20; `calculatePP` = `(pp/5+1)*4`. `statModify` (below). `getActionSpeed`: no Trick Room underflow. `formeChange`: Megas don't revert on faint. `clearVolatile` resets `timesAttacked`. `canTerastallize` always null. `canMegaEvo`. `modifyDamage`. `spreadMoveHit` runs `AfterHit` even when the user fainted. `hitStepMoveHitLoop`: Parental Bond hit count, `timesAttacked`. `afterMoveSecondaryEvent`: Sheer Force does not suppress `AfterMoveSecondary`. |
| `rulesets.ts` | Standard AG / Standard / Flat Rules overrides (level 50, Item Clause 1, Species Clause, Min Team Size 6, Picked Team Size Auto); team preview; Level Clause Mod. |
| `formats-data.ts`, `learnsets.ts` | Legality. |

**Stat points.** At level 50 without Level Clause Mod, Showdown computes non-HP stats as
`base + SP + 20` and HP as `base + SP + 75`, before the nature. SP is 0-32 per stat, at most 66
in total, carried in the set's `evs` field. With 31 IVs, the mainline formula reproduces this
exactly when **EV = max(8·SP − 4, 0)**. Implement either that mapping or the direct formula, and
let validation accept stat points.

**Public HP (Champions).** Showdown's public HP for this mod is a percentage
(`sim/pokemon.ts:2060-2074`):
- `floor(100·hp/maxhp)`, with a minimum of 1;
- at exactly 20 it appends `y` if `hp·5 > maxhp`, otherwise `r`;
- at exactly 50 it appends `g` if `hp·2 > maxhp`, otherwise `y`;
- a fainted Pokémon is `0 fnt`.

Acceptance: unit tests for each listed behaviour change and for the stat formula, plus the P1c
differential fuzz.

### P1b — Showdown compatibility layer (a new crate)

1. **Protocol output** in Showdown syntax for these line types. Each must be byte-identical to
   Showdown's lines, including `[from]`, `[of]`, `[silent]`, `[upkeep]`, `[spread]`,
   `[still]`, `[miss]`, `[notarget]` tags and `|split|pN` secret/public pairs.
   - **Line types:** `switch`, `drag`, `replace`, `-damage`, `-heal`, `-sethp`, `faint`, `move`,
     `cant`, `detailschange`, `-formechange`, `-mega`, `-status`, `-curestatus`, `-boost`,
     `-unboost`, `-setboost`, `-clearboost`, `-clearallboost`, `-clearpositiveboost`,
     `-clearnegativeboost`, `-invertboost`, `swap`, `-sidestart`, `-sideend`, `-weather`,
     `-fieldstart`, `-fieldend`, `-start`, `-end`, `-item`, `-enditem`, `-ability`, `-activate`,
     `-fail`, `-miss`, `-immune`, `-crit`, `-supereffective`, `-resisted`, `turn`, `win`, `tie`,
     `upkeep`.
   - **Identities:** `p1a: Nickname`, using Showdown's slot letters and side ids.
   - **Allowlist:** any intentional difference goes in `compat/allowlist.md`, with a reason.
2. **Choices:** a Showdown → battler choice translator. Showdown's slots are 1-based (`move 1 2,
   move 3 -1 mega`, `switch 3`, `team 1234`, `pass`); battler's are 0-based.
3. **Requests:** a battler → Showdown `|request|` JSON translator covering `active[].moves`
   (id, pp, maxpp, target, disabled), `trapped`, `maybeTrapped`, `canMegaEvo`, `forceSwitch`,
   `wait` and `side.pokemon`.
4. **Teams:** Showdown packed or exported teams → battler `MonData`, with IVs defaulting to 31.

### P1c — differential harness from turn 0

A driver (Node, running the pinned Showdown in-process) and a battler runner binary, talking
JSON lines over stdio. Both engines get the same teams, choices and **semantic random streams**.

**Semantic random streams.**
- Raw RNG streams cannot be shared:
  - battler maps values with `next() % n` (`battler-prng/src/rand_util.rs`), while Showdown uses
    `floor(r·n/2^32)`;
  - the two engines draw in different orders;
  - battler skips the draw for a one-element sample, while Showdown still draws.
- Instead, each **category** gets its own seeded stream of uniforms `u ∈ [0,1)`. Each engine
  computes `chance(p) = u < p`, `int(n) = floor(u·n)`, `sample(xs) = xs[floor(u·len)]`.
- Outcomes that are certain (`p = 0`, `p = 1`, `n = 1`) do not consume a draw in either engine.
- **Categories:** `accuracy`, `crit`, `secondary` (including self-boost chances), `damage-roll`,
  `multi-hit`, `full-para`, `confusion-hit`, `status-turns` (sleep, freeze thaw, confusion
  duration), `speed-tie`, and `effect:<id>` for effect-specific draws such as Healer, Dire Claw
  and Metronome.
- **Showdown hooks:** monkeypatch `Battle.prototype.random`, `randomChance`, `sample` and
  `PRNG.prototype.shuffle` from the driver. Classify each call by its caller frame against a
  table for the pinned build. Fail loudly on any unknown call site. Showdown's
  `forceRandomChance` is not enough: secondaries use `random(100)`
  (`sim/battle-actions.ts:1325,1343`), the damage roll uses `random(16)` (`sim/battle.ts:2393`),
  and multi-hit and sleep use `sample`.
- **battler hooks:** in `rand_util` and in fxlang's `chance`/`random`/`sample` functions
  (`battler/src/effect/fxlang/functions.rs` around 1522-1589), which know the effect id.
- **Draw logs:** both engines log every draw as `(category, p or n)`. Diffing the logs pinpoints
  probability-table bugs (paralysis 1/8, crit stages, accuracy changes) before any state
  diverges.

**Speed ties.**
- Prefer teams without exact speed ties among the active Pokémon.
- When either engine breaks a speed tie or shuffles equal-speed handlers between different
  owners (Showdown's `speedSort`, `sim/battle.ts:456,507,794`), mark that turn **tie-affected**.
  End the comparison there in P1; re-sync by transplant in P2.

**What is compared each turn:**
1. the draw log;
2. normalized state;
3. the P1b protocol lines.

Normalized state covers:
- **actives:** species and forme, exact HP, status and its counters, boosts, and volatiles with
  counters (substitute HP, Protect streak, Encore move and turns, Disable move and turns, choice
  lock, two-turn charge, locked move and turns, confusion, Taunt, flinch);
- **every team member:** HP, status, fainted, item, ability, PP per move, Mega used;
- **side conditions** with remaining turns;
- **field:** weather, terrain, Trick Room and other pseudo-weather with remaining turns;
- **the turn number**;
- **the translated request** for each side.

**Inputs.**
- Seeded random-choice battles with Champions random-doubles teams at level 50, from team
  preview to the end.
- **Scenario tests** written as Showdown-format team plus choice scripts:
  - Fake Out first turn only;
  - Protect streak decay;
  - Encore and Disable counters;
  - choice lock and switching out;
  - two-turn charge moves;
  - simultaneous replacement after a double KO, including a pass slot;
  - Mega Evolution in doubles, with turn order using post-Mega speed;
  - Trick Room ordering;
  - Tailwind duration;
  - Follow Me and Rage Powder redirection;
  - Wide Guard and Quick Guard;
  - Intimidate, Parting Shot and Eject Button interactions;
  - spread-move damage;
  - Ally Switch;
  - sleep and freeze turns under the Champions odds.

**Divergence budget.** Over at least 10,000 compared turns, fewer than **1%** of turns diverge
in state. Classify every divergence by mechanic, and fix in order of frequency. The phase is done
when the budget holds and every remaining divergence is listed with a cause.

### P2 — transplant, then clone

**Transplant (`apply_state`).**
- Build a battler battle from Showdown's serialized state (`battle.toJSON()`, i.e.
  `sim/state.ts` `serializeBattle` at the pinned commit) without replaying history:
  - a silent start: no entry effects, no RNG draws, no logs;
  - then a privileged patch of the turn number, actives and positions, HP, status and counters,
    boosts, volatiles and effect states with durations and counters;
  - then items, abilities, PP, side and slot conditions, weather, terrain and pseudo-weather
    with remaining turns, Mega form with recalculated stats, and fainted members;
  - then the pending request.
- Moves that refer to a previous move (Encore, Disable, choice lock) need a matching entry in the
  move registry.
- **Acceptance:** transplant positions from P1c's Showdown battles at every turn boundary, then
  continue both engines under the semantic streams for several turns. Transplanted mid-game fuzz
  becomes the main differential mode, with the same 1% budget. It reaches states that play from
  turn 0 rarely produces.

**Clone.**
- Add `Clone` for the full battle state:
  - roughly 13 types: `Mon`, `Side`, `Field`, `Player`, `ChoiceState`, `BattleRegistry`,
    `EffectManager`, `LinkedEffectsManager`, `BattleLog`, the faint and catch entries, and
    `Condition`;
  - a `box_clone` for the PRNG;
  - no clock in copies;
  - a **shared** Dex, because cloning `Dex` empties its caches;
  - copying `BattleRegistry` with the same handles;
  - truncating logs on copy.
- Cloning is allowed only at quiet points (no live context, empty effect stack).
- **Acceptance:**
  - a clone and a fresh transplant of the same state behave identically under the same streams
    and choices;
  - mutating a clone leaves the original untouched;
  - a microbenchmark reports µs and bytes per clone.
- Re-run P0's benchmarks with transplant and clone in place of new+start.

## 5. Session rules

- **Scope:** one phase per session, on its own branch (`p0-bench`, `p1a-rules`, ...). Commit and
  push at the end.
- **Progress log:** keep `docs/progress.md` current: what was done, the test status, open
  divergences, the next step.
- **Keep output small:** run `cargo test -q` and quiet driver output. Write long logs to files
  and summarize them.
- **Gates:** stop at a failed gate and report. Never weaken a test or widen the allowlist to make
  a phase pass.
- **Model choice:** Sonnet suits mechanical work (data overlay entries, Clone derives, protocol
  tables). Opus suits divergence triage and `apply_state`. Credits are limited.

## 6. P0 session brief (paste into a new session)

> Read `docs/champions-showdown-spec.md` in full, then do **P0 only** (section 4, P0). Pin a dated
> nightly toolchain that builds the workspace, check out and build Pokémon Showdown at commit
> `3661ce40bf9001d185ce078b8920e12304204609`, generate the 200 benchmark positions, and run
> benchmarks (a), (b) and (c) as specified. Write `docs/benchmarks/p0.md` with raw JSON beside
> it, recording the hardware and all versions. Report numbers only, with no projections or
> recommendations. Create `docs/progress.md` with what you did and anything that blocked you.
> Work on branch `p0-bench`, commit, and push. Do not start P1.
