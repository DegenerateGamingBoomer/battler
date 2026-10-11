//! Pokémon Champions rules for battler.
//!
//! The rules come in two parts:
//! - [`ChampionsDataStore`], an overlay over any [`DataStore`] that applies the data differences of
//!   Pokémon Showdown's `champions` mod (moves, abilities, items, conditions), so mainline behavior
//!   stays available by simply not using the overlay.
//! - [`rules`], the rules that cannot be expressed as data (stat points, PP, public health), as
//!   engine options plus plain functions that tests can call.
//!
//! Data patches are embedded JSON merge patches (see [`patch`]) in `data/`, one set per regulation.

pub mod legality;
pub mod patch;
pub mod rules;

use std::sync::RwLock;

use ahash::HashMap;
use anyhow::{
    Context,
    Result,
};
use battler_data::{
    AbilityData,
    ClauseData,
    ConditionData,
    DataStore,
    DataStoreByName,
    Id,
    ItemData,
    MoveData,
    ResourceType,
    SpeciesData,
    TypeChart,
};
use serde::{
    Serialize,
    de::DeserializeOwned,
};
use serde_json::Value;

use crate::patch::apply_patch;

/// A Pokémon Champions regulation.
///
/// All regulations share the `champions` mod's rules; a regulation adds its own patch layer on top.
/// The mod `championsregmb` (Reg M-B) is a later target and currently has no layer of its own, so
/// it behaves exactly like [`Regulation::MC`] until it is verified against Showdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Regulation {
    /// Regulation M-C (`gen9championsvgc2026regmc`, mod `champions`).
    MC,
    /// Regulation M-B (`gen9championsvgc2026regmb`, mod `championsregmb`). Not yet verified.
    MB,
}

impl Regulation {
    /// The Showdown format ID this regulation is verified against.
    pub fn showdown_format(&self) -> &'static str {
        match self {
            Self::MC => "gen9championsvgc2026regmc",
            Self::MB => "gen9championsvgc2026regmb",
        }
    }
}

/// All patches for one regulation, by resource type and resource ID.
#[derive(Debug, Default, Clone)]
pub struct PatchSet {
    moves: HashMap<Id, Value>,
    abilities: HashMap<Id, Value>,
    items: HashMap<Id, Value>,
    conditions: HashMap<Id, Value>,
    /// Upper bound applied to every move's base PP before patches.
    pub max_base_pp: Option<u8>,
}

fn parse_table(source: &str, what: &str) -> Result<HashMap<Id, Value>> {
    let table: HashMap<String, Value> =
        serde_json::from_str(source).with_context(|| format!("failed to parse {what} patches"))?;
    Ok(table.into_iter().map(|(k, v)| (Id::from(k), v)).collect())
}

impl PatchSet {
    /// The patches for the given regulation.
    pub fn for_regulation(regulation: Regulation) -> Result<Self> {
        let patches = Self {
            moves: parse_table(include_str!("../data/mc/moves.json"), "move")?,
            abilities: parse_table(include_str!("../data/mc/abilities.json"), "ability")?,
            items: parse_table(include_str!("../data/mc/items.json"), "item")?,
            conditions: parse_table(include_str!("../data/mc/conditions.json"), "condition")?,
            max_base_pp: Some(rules::MAX_BASE_PP),
        };
        match regulation {
            // Reg M-B has no verified layer of its own yet.
            Regulation::MC | Regulation::MB => Ok(patches),
        }
    }
}

/// A [`DataStore`] that applies the Pokémon Champions rules on top of another [`DataStore`].
pub struct ChampionsDataStore<'d> {
    inner: &'d dyn DataStoreByName,
    regulation: Regulation,
    patches: PatchSet,
    moves: RwLock<HashMap<Id, Option<MoveData>>>,
    abilities: RwLock<HashMap<Id, Option<AbilityData>>>,
    items: RwLock<HashMap<Id, Option<ItemData>>>,
    conditions: RwLock<HashMap<Id, Option<ConditionData>>>,
    /// Condition names to IDs for patched conditions (condition IDs, like `par`, are not derived
    /// from their names).
    condition_names: HashMap<String, Id>,
}

impl<'d> ChampionsDataStore<'d> {
    /// Creates a new overlay over `inner` for the given regulation.
    pub fn new(inner: &'d dyn DataStoreByName, regulation: Regulation) -> Result<Self> {
        let patches = PatchSet::for_regulation(regulation)?;
        let mut condition_names = HashMap::default();
        for id in patches.conditions.keys() {
            if let Some(condition) = inner.get_condition(id)? {
                condition_names.insert(condition.name, id.clone());
            }
        }
        Ok(Self {
            inner,
            regulation,
            patches,
            moves: RwLock::default(),
            abilities: RwLock::default(),
            items: RwLock::default(),
            conditions: RwLock::default(),
            condition_names,
        })
    }

    /// The regulation in effect.
    pub fn regulation(&self) -> Regulation {
        self.regulation
    }

    /// The patch set in effect.
    pub fn patches(&self) -> &PatchSet {
        &self.patches
    }

    fn patched<T>(
        cache: &RwLock<HashMap<Id, Option<T>>>,
        id: &Id,
        patch: Option<&Value>,
        base: impl FnOnce() -> Result<Option<T>>,
        prepare: impl FnOnce(&mut Value),
    ) -> Result<Option<T>>
    where
        T: Clone + Serialize + DeserializeOwned,
    {
        if let Ok(cache) = cache.read()
            && let Some(cached) = cache.get(id)
        {
            return Ok(cached.clone());
        }
        let result = match base()? {
            None => None,
            Some(data) => {
                let mut value = serde_json::to_value(&data)?;
                prepare(&mut value);
                if let Some(patch) = patch {
                    apply_patch(&mut value, patch);
                }
                Some(
                    serde_json::from_value(value)
                        .with_context(|| format!("patched data for {id:?} is invalid"))?,
                )
            }
        };
        if let Ok(mut cache) = cache.write() {
            cache.insert(id.clone(), result.clone());
        }
        Ok(result)
    }

    fn patched_move(&self, id: &Id) -> Result<Option<MoveData>> {
        let cap = self.patches.max_base_pp;
        Self::patched(
            &self.moves,
            id,
            self.patches.moves.get(id),
            || self.inner.get_move(id),
            |value| {
                if let (Some(cap), Some(pp)) = (cap, value.get("pp").and_then(Value::as_u64)) {
                    value["pp"] = Value::from(pp.min(cap as u64));
                }
            },
        )
    }

    fn patched_ability(&self, id: &Id) -> Result<Option<AbilityData>> {
        Self::patched(
            &self.abilities,
            id,
            self.patches.abilities.get(id),
            || self.inner.get_ability(id),
            |_| (),
        )
    }

    fn patched_item(&self, id: &Id) -> Result<Option<ItemData>> {
        Self::patched(
            &self.items,
            id,
            self.patches.items.get(id),
            || self.inner.get_item(id),
            |_| (),
        )
    }

    fn patched_condition(&self, id: &Id) -> Result<Option<ConditionData>> {
        Self::patched(
            &self.conditions,
            id,
            self.patches.conditions.get(id),
            || self.inner.get_condition(id),
            |_| (),
        )
    }

    fn filtered<T>(
        &self,
        ids: Vec<Id>,
        get: impl Fn(&Id) -> Result<Option<T>>,
        filter: &dyn Fn(&T) -> bool,
    ) -> Result<Vec<Id>> {
        let mut out = Vec::new();
        for id in ids {
            if let Some(data) = get(&id)?
                && filter(&data)
            {
                out.push(id);
            }
        }
        Ok(out)
    }
}

impl DataStore for ChampionsDataStore<'_> {
    fn all_ability_ids(&self, filter: &dyn Fn(&AbilityData) -> bool) -> Result<Vec<Id>> {
        let ids = self.inner.all_ability_ids(&|_| true)?;
        self.filtered(ids, |id| self.patched_ability(id), filter)
    }

    fn all_item_ids(&self, filter: &dyn Fn(&ItemData) -> bool) -> Result<Vec<Id>> {
        let ids = self.inner.all_item_ids(&|_| true)?;
        self.filtered(ids, |id| self.patched_item(id), filter)
    }

    fn all_move_ids(&self, filter: &dyn Fn(&MoveData) -> bool) -> Result<Vec<Id>> {
        let ids = self.inner.all_move_ids(&|_| true)?;
        self.filtered(ids, |id| self.patched_move(id), filter)
    }

    fn all_species_ids(&self, filter: &dyn Fn(&SpeciesData) -> bool) -> Result<Vec<Id>> {
        self.inner.all_species_ids(filter)
    }

    fn get_type_chart(&self) -> Result<TypeChart> {
        self.inner.get_type_chart()
    }

    fn translate_alias(&self, resource_type: ResourceType, id: &Id) -> Result<Option<Id>> {
        self.inner.translate_alias(resource_type, id)
    }

    fn get_ability(&self, id: &Id) -> Result<Option<AbilityData>> {
        self.patched_ability(id)
    }

    fn get_clause(&self, id: &Id) -> Result<Option<ClauseData>> {
        self.inner.get_clause(id)
    }

    fn get_condition(&self, id: &Id) -> Result<Option<ConditionData>> {
        self.patched_condition(id)
    }

    fn get_item(&self, id: &Id) -> Result<Option<ItemData>> {
        self.patched_item(id)
    }

    fn get_move(&self, id: &Id) -> Result<Option<MoveData>> {
        self.patched_move(id)
    }

    fn get_species(&self, id: &Id) -> Result<Option<SpeciesData>> {
        self.inner.get_species(id)
    }
}

impl DataStoreByName for ChampionsDataStore<'_> {
    fn get_ability_by_name(&self, name: &str) -> Result<Option<AbilityData>> {
        match self.inner.get_ability_by_name(name)? {
            Some(data) => self.patched_ability(&Id::from(data.name.as_str())),
            None => Ok(None),
        }
    }

    fn get_clause_by_name(&self, name: &str) -> Result<Option<ClauseData>> {
        self.inner.get_clause_by_name(name)
    }

    fn get_condition_by_name(&self, name: &str) -> Result<Option<ConditionData>> {
        if let Some(id) = self.condition_names.get(name) {
            return self.patched_condition(id);
        }
        self.inner.get_condition_by_name(name)
    }

    fn get_item_by_name(&self, name: &str) -> Result<Option<ItemData>> {
        match self.inner.get_item_by_name(name)? {
            Some(data) => self.patched_item(&Id::from(data.name.as_str())),
            None => Ok(None),
        }
    }

    fn get_move_by_name(&self, name: &str) -> Result<Option<MoveData>> {
        match self.inner.get_move_by_name(name)? {
            Some(data) => self.patched_move(&Id::from(data.name.as_str())),
            None => Ok(None),
        }
    }

    fn get_species_by_name(&self, name: &str) -> Result<Option<SpeciesData>> {
        self.inner.get_species_by_name(name)
    }
}
