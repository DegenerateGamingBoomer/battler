//! Semantic random streams (spec P1c).
//!
//! Every draw category gets its own stream of 32-bit values, seeded from the battle seed and the
//! category name. Both engines map a value `x` the same way: `int(n) = floor(x * n / 2^32)` and
//! `chance(p / q) = int(q) < p`. Certain outcomes (`n = 1`, `p = 0`, `p >= q`) draw nothing.
//!
//! The generator and the mapping are mirrored exactly by `compat/diff/semantic.js`.

use std::{
    any::Any,
    collections::HashMap,
};

use battler_prng::{
    DrawSite,
    PseudoRandomNumberGenerator,
    SemanticRandom,
};
use serde::Serialize;

/// FNV-1a over the bytes of `text`.
fn fnv1a(text: &str) -> u32 {
    let mut hash = 0x811c9dc5u32;
    for byte in text.bytes() {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    hash
}

/// Mulberry32: one stream of 32-bit values.
#[derive(Debug, Clone)]
pub struct Stream {
    state: u32,
}

impl Stream {
    pub fn new(seed: u32, category: &str) -> Self {
        Self {
            state: fnv1a(&format!("{seed}:{category}")),
        }
    }

    pub fn next(&mut self) -> u32 {
        self.state = self.state.wrapping_add(0x6D2B79F5);
        let a = self.state;
        let mut t = (a ^ (a >> 15)).wrapping_mul(1 | a);
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        t ^ (t >> 14)
    }
}

/// One logged draw.
#[derive(Debug, Clone, Serialize)]
pub struct Draw {
    /// Category.
    pub c: String,
    /// Kind: `chance` or `int`.
    pub k: &'static str,
    /// Denominator (`chance`) or range size (`int`).
    pub n: u64,
    /// Numerator (`chance` only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p: Option<u64>,
    /// Result.
    pub r: u64,
    /// The engine's own name for the site, for debugging.
    pub site: String,
}

/// One speed tie the engine is about to shuffle.
#[derive(Debug, Clone, Serialize)]
pub struct Tie {
    pub owners: Vec<Option<u64>>,
    /// The tied items have more than one owner.
    pub distinct: bool,
}

/// Maps a battler draw site to its semantic category.
///
/// Effect draws are `effect:<id>` unless the table below names a shared category. The Showdown
/// side has the matching table in `compat/diff/hooks.js`.
pub fn category(site: DrawSite) -> String {
    match site {
        DrawSite::Engine(name) => name.to_owned(),
        DrawSite::Effect { effect, event } => match (effect, event) {
            ("par", "BeforeMove") => "full-para".to_owned(),
            ("slp", "Start") => "status-turns".to_owned(),
            ("frz", "BeforeMove") => "status-turns".to_owned(),
            ("confusion", "Start") => "status-turns".to_owned(),
            ("confusion", "BeforeMove") => "confusion-hit".to_owned(),
            ("twoturnmove", _) => "target".to_owned(),
            _ => format!("effect:{effect}"),
        },
    }
}

fn site_name(site: DrawSite) -> String {
    match site {
        DrawSite::Engine(name) => name.to_owned(),
        DrawSite::Effect { effect, event } => format!("{effect}@{event}"),
    }
}

/// A [`PseudoRandomNumberGenerator`] whose draws all go through semantic streams.
pub struct SemanticRng {
    seed: u32,
    streams: HashMap<String, Stream>,
    pub draws: Vec<Draw>,
    pub ties: Vec<Tie>,
    /// Draws that bypassed the semantic interface (raw `next()` calls).
    pub raw_draws: usize,
}

impl SemanticRng {
    pub fn new(seed: u32) -> Self {
        Self {
            seed,
            streams: HashMap::new(),
            draws: Vec::new(),
            ties: Vec::new(),
            raw_draws: 0,
        }
    }

    fn value(&mut self, category: &str) -> u32 {
        let seed = self.seed;
        self.streams
            .entry(category.to_owned())
            .or_insert_with(|| Stream::new(seed, category))
            .next()
    }

    fn int_value(&mut self, category: &str, n: u64) -> u64 {
        (self.value(category) as u64 * n) >> 32
    }
}

impl SemanticRandom for SemanticRng {
    fn chance(&mut self, site: DrawSite, numerator: u64, denominator: u64) -> bool {
        if numerator == 0 {
            return false;
        }
        if numerator >= denominator {
            return true;
        }
        let category = category(site);
        let value = self.int_value(&category, denominator);
        let result = value < numerator;
        self.draws.push(Draw {
            c: category,
            k: "chance",
            n: denominator,
            p: Some(numerator),
            r: result as u64,
            site: site_name(site),
        });
        result
    }

    fn int(&mut self, site: DrawSite, n: u64) -> u64 {
        if n <= 1 {
            return 0;
        }
        let category = category(site);
        let value = self.int_value(&category, n);
        self.draws.push(Draw {
            c: category,
            k: "int",
            n,
            p: None,
            r: value,
            site: site_name(site),
        });
        value
    }

    fn ties(&mut self, _: DrawSite, owners: &[Option<u64>]) {
        let first = owners.first().copied().flatten();
        let distinct = owners
            .iter()
            .any(|owner| owner.is_none() || *owner != first);
        self.ties.push(Tie {
            owners: owners.to_vec(),
            distinct,
        });
    }
}

impl PseudoRandomNumberGenerator for SemanticRng {
    fn initial_seed(&self) -> u64 {
        self.seed as u64
    }

    fn next(&mut self) -> u64 {
        self.raw_draws += 1;
        0
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn semantic(&mut self) -> Option<&mut dyn SemanticRandom> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference values printed by `compat/diff/semantic.js` for the same seeds and categories.
    #[test]
    fn stream_matches_javascript() {
        let mut stream = Stream::new(1, "accuracy");
        let values = (0..4).map(|_| stream.next()).collect::<Vec<_>>();
        assert_eq!(values, [1967493806, 365266975, 107005309, 3686817290]);
        let mut stream = Stream::new(u32::MAX, "effect:healer");
        let values = (0..2).map(|_| stream.next()).collect::<Vec<_>>();
        assert_eq!(values, [3035294749, 2773199422]);
    }

    #[test]
    fn certain_outcomes_draw_nothing() {
        let mut rng = SemanticRng::new(7);
        let site = DrawSite::Engine("accuracy");
        assert!(rng.chance(site, 100, 100));
        assert!(!rng.chance(site, 0, 100));
        assert_eq!(rng.int(site, 1), 0);
        assert!(rng.draws.is_empty());
        rng.chance(site, 50, 100);
        assert_eq!(rng.draws.len(), 1);
    }
}
