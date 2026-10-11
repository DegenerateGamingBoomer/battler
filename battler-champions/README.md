# battler-champions

Pokémon Champions rules for battler, verified against Pokémon Showdown's `champions` mod (Reg M-C,
`gen9championsvgc2026regmc`, pinned commit in `scripts/fetch-showdown.sh`).

Mainline behavior stays available: nothing here changes `battler` unless you opt in.

```rust
let data = ChampionsDataStore::new(&local_data_store, Regulation::MC)?;   // data overlay
let engine = battler_champions::rules::engine_options(Default::default()); // engine rules
let rules = battler_champions::rules::format_rules(Regulation::MC);        // FormatData rules
```

- **Data overlay** (`ChampionsDataStore`): JSON merge patches in `data/mc/` over moves, abilities,
  items and conditions (`par`, `slp`, `frz`, ...), applied on every lookup and cached.
- **Engine rules** (`battler::ChampionsRules`, in `CoreBattleEngineOptions::champions`): stat points,
  PP, public health, Mega persistence, attack counters, Sheer Force events, Parental Bond logging.
- **Legality** (`legality::Legality`): which species, moves, abilities and items exist in the format.

## Verification

`tests/data/*.json` are exported from Showdown (`scripts/export-champions-*.js`) and checked by
`tests/conformance.rs` (data fields), `tests/rules.rs` (stats, PP, public health) and the behavior
tests (`status.rs`, `moves.rs`, `effects.rs`, `engine.rs`), which compare Champions behavior with
mainline on the same scenario.

Known gaps are listed in `docs/progress.md`; deliberate Showdown quirks in `docs/showdown-quirks.md`.
