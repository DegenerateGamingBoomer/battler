#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod rand_util;

use core::any::Any;

#[cfg(feature = "std")]
use rand::RngExt;

/// A pseudo-random number generator, created with the intention of using a random number generator
/// that can be deterministically "replayed" for battle simulations.
pub trait PseudoRandomNumberGenerator: Send + Sync {
    /// Returns the initial seed the generator was created with.
    ///
    /// The initial seed can be used to replay the random number generation sequence.
    fn initial_seed(&self) -> u64;

    /// Returns the next integer in the sequence.
    fn next(&mut self) -> u64;

    /// Mutable cast to [`Any`]` for testing.
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// The semantic random source that should receive draws instead of [`Self::next`], if any.
    ///
    /// Ordinary generators return [`None`]. Differential testing uses a semantic source so that two
    /// engines that draw in different orders still get the same outcome for the same event.
    fn semantic(&mut self) -> Option<&mut dyn SemanticRandom> {
        None
    }
}

/// Where a random draw comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawSite<'a> {
    /// A draw made by the engine itself, such as `accuracy` or `damage-roll`.
    Engine(&'static str),
    /// A draw made by an effect program while handling an event.
    Effect { effect: &'a str, event: &'a str },
}

/// A random source that is told what each draw is for.
///
/// Implementations decide the outcome from the draw site, so that every engine that makes the same
/// draw for the same reason gets the same result, regardless of how many unrelated draws happened
/// before it.
pub trait SemanticRandom: Send + Sync {
    /// A `numerator / denominator` chance.
    fn chance(&mut self, site: DrawSite, numerator: u64, denominator: u64) -> bool;

    /// An integer in `[0, n)`.
    fn int(&mut self, site: DrawSite, n: u64) -> u64;

    /// Called before the engine shuffles tied items, with an owner for each tied item (for
    /// example, the Mon an action or effect belongs to), or [`None`] if the owner is unknown.
    fn ties(&mut self, site: DrawSite, owners: &[Option<u64>]) {
        let _ = (site, owners);
    }
}

/// A real implementation of [`PseudoRandomNumberGenerator`].
pub struct RealPseudoRandomNumberGenerator {
    initial_seed: u64,
    seed: u64,
}

impl RealPseudoRandomNumberGenerator {
    /// Creates a new random number generator.
    ///
    /// If two random number generators are created with the same seed, their output should be
    /// exactly the same.
    pub fn new(seed: Option<u64>) -> Self {
        let seed = seed.unwrap_or_else(|| Self::generate_seed());
        Self {
            initial_seed: seed,
            seed,
        }
    }

    fn generate_seed() -> u64 {
        #[cfg(feature = "std")]
        {
            let mut rng = rand::rng();
            return rng.random();
        }
        #[allow(unreachable_code)]
        u64::MIN
    }

    /// Linear Congruential Generator (LCRNG).
    fn next_seed(seed: u64) -> u64 {
        // Constants in the generation V and VI games.
        const A: u64 = 0x5D588B656C078965;
        const C: u64 = 0x0000000000269EC3;
        seed.wrapping_mul(A).overflowing_add(C).0
    }
}

impl PseudoRandomNumberGenerator for RealPseudoRandomNumberGenerator {
    fn initial_seed(&self) -> u64 {
        self.initial_seed
    }

    fn next(&mut self) -> u64 {
        self.seed = Self::next_seed(self.seed);
        // Use the upper 32 bits. The lower ones are predictable in some situations.
        self.seed >> 32
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod prng_test {
    use crate::{
        PseudoRandomNumberGenerator,
        RealPseudoRandomNumberGenerator,
    };

    #[test]
    fn stores_initial_seed() {
        assert_eq!(
            RealPseudoRandomNumberGenerator::new(Some(12345)).initial_seed(),
            12345
        );
        assert_eq!(
            RealPseudoRandomNumberGenerator::new(Some(6789100000)).initial_seed(),
            6789100000
        );
    }
}
