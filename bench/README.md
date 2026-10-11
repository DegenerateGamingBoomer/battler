# Benchmarks

Layout:
- `positions/` – 200 mid-game Reg M-C positions generated with the pinned Showdown build
  (`battle.toJSON()` plus teams, seeds, input log).
- `calc-probes.json` – damage probes exported from those positions (attacker, defender, move, field,
  Showdown's rolled damage).
- `showdown/` – Node scripts (run against `third_party/pokemon-showdown`).
- `battler-bench/` – Rust benchmark binary for the battler engine and `battler-calc`.

Reproduce (results land in `docs/benchmarks/`):

```sh
scripts/fetch-showdown.sh                         # clone + build Showdown at the pinned commit
node bench/showdown/gen-positions.js 200          # bench/positions/*.json (deterministic)
node bench/showdown/gen-calc-probes.js            # bench/calc-probes.json
node bench/showdown/bench.js 10                   # docs/benchmarks/p0-showdown.json

export DATA_DIR=$PWD/battle-data/data DESCRIPTIONS_DIR=$PWD/battle-data/descriptions CRATE_ROOT=$PWD
cargo build --release -p battler-bench
./target/release/battler-bench bench/positions 10 docs/benchmarks/p0-battler.json bench/calc-probes.json
```

Building the whole workspace also needs `pyinstaller` (see `battler-ai/build.rs`; set
`GEMINI_PYTHON_VENV` to the prefix that holds `bin/pyinstaller`).
