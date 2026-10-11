# battler-showdown-compat

Translates between battler and Pokémon Showdown (pinned at `3661ce40`):

- `protocol::Translator`: battler battle log → Showdown protocol lines (`|switch|`, `|-damage|`, …,
  with `|split|pN` pairs). Stateful: it tracks identities (`p1a: Nickname`), public HP and the
  attributes Showdown puts on `move` lines (`[still]`, `[spread]`, `[notarget]`, `[miss]`).
- `choice::translate`: Showdown choice string (1-based) → battler choice (0-based).
- `request::translate`: battler request + player data → Showdown `|request|` JSON.
- `team::parse` / `team::convert`: Showdown packed or exported teams → battler `TeamData` (IVs default
  to 31; Champions stat points → EVs).
- `names`, `species`: Showdown ↔ battler spelling of species, moves, items and abilities.

Tests replay scenarios (`compat/scenarios.js`) on Showdown and on battler with a constant random
source and compare byte for byte, except for the rules in `compat/allowlist.md`.

Regenerate the references after changing a scenario (needs `scripts/fetch-showdown.sh` first):

```
node compat/gen-fixtures.js [name]
cargo test -p battler-showdown-compat
```
