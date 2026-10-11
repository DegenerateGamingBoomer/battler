# Showdown ⇄ battler allowlist

Intentional differences between Showdown's protocol output and the translated battler log. The
comparison in `battler-showdown-compat/tests/common/mod.rs` (`apply_allowlist`) implements exactly
these rules and nothing else. A difference that is not listed here is a bug in the translator or in
battler (see `docs/progress.md` for the open ones); do not add a rule to make a test pass.

Each rule blanks information battler cannot know. It never rewrites HP, damage, order or the set of
lines.

| ID | Lines | What is ignored | Why |
|----|-------|-----------------|-----|
| R1 | `\|move\|` lines with `[spread]`, and the side-targeting moves Spikes, Stealth Rock, Toxic Spikes, Sticky Web | The printed target (field 4). The `[spread]` list itself is compared. | Showdown prints a target picked with a random draw (`getRandomTarget`). battler does not make that draw, so it has no target to log. |
| R2 | `\|-hint\|` | The whole line. | Advice for the client UI (for example "Max Move usage"), not a battle event. battler has no equivalent. |
| R3 | `\|move\|` lines ending in `[notarget]` | The printed target (field 4). | When every target is gone Showdown names the fainted Pokémon the move had picked. battler logs no target. The `[notarget]` flag and the following `-fail` are compared. |

## Normalization (not allowlist entries)

These are applied to the Showdown log when the fixtures are generated (`compat/gen-fixtures.js`),
because they carry no battle semantics: `|t:|`, `|gametype|`, `|player|`, `|gen|`, `|tier|`,
`|rule|`, `|clearpoke|`, `|poke|`, `|teampreview|`, `|uhtml|`, `|request|` and the `|split|` pairs
that wrap them.

## Known differences not allowlisted (open, tracked in `docs/progress.md`)

These make the corpus tool (`tests/corpus.rs`) report a difference. They are engine behaviour that
differs from Showdown, so they belong to P1c, not to this list.

- Speed ties: Showdown shuffles tied actions with its PRNG; battler is run with `Keep`. Scenarios
  avoid ties.
- White Herb after a faint: Showdown prints `faint` before the herb activates, battler runs the
  herb first.
- Residual order of Taunt's end and Yawn's sleep (Showdown: Taunt first; battler: Yawn first).
- The random-battle corpus (`tests/corpus.rs`, informational) matches exactly in 1 of 40 battles.
  First differences are mostly engine ones: redirection (Rage Powder, Lightning Rod), spread-move
  misses, Knock Off/faint ordering, Trace's `-ability` line, Flower Veil's `-block`, damage
  off-by-one in a few modifier chains.
