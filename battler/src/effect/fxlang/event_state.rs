use alloc::string::String;
use core::cell::{
    Cell,
    RefCell,
    RefMut,
};

use battler_data::Id;
use hashbrown::{
    HashMap,
    HashSet,
};

use crate::effect::fxlang::Value;

/// The neutral modifier, in 4096ths.
pub const NEUTRAL_MODIFIER: u32 = 4096;

/// State for the evaluation of an event.
#[derive(Debug)]
pub struct EventState {
    effect_ids_to_skip: RefCell<HashSet<Id>>,
    values: RefCell<HashMap<String, Value>>,
    modifier: Cell<u32>,
}

impl Default for EventState {
    fn default() -> Self {
        Self {
            effect_ids_to_skip: RefCell::default(),
            values: RefCell::default(),
            modifier: Cell::new(NEUTRAL_MODIFIER),
        }
    }
}

impl EventState {
    fn effect_ids_to_skip(&self) -> RefMut<'_, HashSet<Id>> {
        self.effect_ids_to_skip.borrow_mut()
    }

    fn values(&self) -> RefMut<'_, HashMap<String, Value>> {
        self.values.borrow_mut()
    }

    /// Marks the effect's callback to be skipped for the event.
    pub fn skip_effect(&self, effect: Id) {
        let mut skip = self.effect_ids_to_skip();
        skip.insert(effect);
    }

    /// Checks if the effect's callback for the event should run.
    pub fn effect_should_run(&self, effect: &str) -> bool {
        !self.effect_ids_to_skip().contains(effect)
    }

    /// Saves an arbitrary value to a key.
    pub fn save_value(&self, key: String, value: Value) {
        self.values().insert(key, value);
    }

    /// Reads an artbirary value by key.
    pub fn read_value(&self, key: &str) -> Option<Value> {
        self.values().get(key).cloned()
    }

    /// Chains a multiplier onto the event's modifier, which is applied once to the event's numeric
    /// result after every callback ran.
    ///
    /// Mirrors Pokémon Showdown's `chainModify`: the modifier is kept in 4096ths, each multiplier
    /// is truncated to 4096ths, and the product of two is rounded.
    pub fn chain_modify(&self, numerator: u64, denominator: u64) {
        let next = numerator * 4096 / denominator;
        let previous = self.modifier.get() as u64;
        self.modifier.set(((previous * next + 2048) >> 12) as u32);
    }

    /// The event's chained modifier, in 4096ths.
    pub fn modifier(&self) -> u32 {
        self.modifier.get()
    }

    /// Applies the chained modifier to `value` now and resets it, like Pokémon Showdown's
    /// `finalModify`.
    pub fn final_modify(&self, value: u64) -> u64 {
        let value = modify(value, self.modifier.get());
        self.modifier.set(NEUTRAL_MODIFIER);
        value
    }
}

/// Applies a modifier in 4096ths to a value, rounding like Pokémon Showdown's `modify`.
pub fn modify(value: u64, modifier: u32) -> u64 {
    (value * modifier as u64 + 2048 - 1) / 4096
}
