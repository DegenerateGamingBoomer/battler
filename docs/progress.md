# Progress log

Spec: `champions-showdown-spec.md`. One section per phase, newest first.

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
