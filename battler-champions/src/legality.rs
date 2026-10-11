//! Which species, moves, abilities and items are available in a Champions regulation.
//!
//! The lists are exported from Pokémon Showdown's `champions` mod (everything without an
//! `isNonstandard` flag) by `scripts/export-champions-legality.js`. IDs are Showdown IDs, which are
//! normalized names; look them up with [`battler_data::Id::from`]. Learnset legality (which species
//! can learn which move) is not included.

use ahash::HashSet;
use anyhow::{
    Context,
    Result,
};
use battler_data::Id;
use serde::Deserialize;

use crate::Regulation;

#[derive(Deserialize)]
struct LegalityFile {
    moves: Vec<String>,
    items: Vec<String>,
    abilities: Vec<String>,
    species: Vec<String>,
}

/// Availability lists for a regulation.
#[derive(Debug, Clone)]
pub struct Legality {
    moves: HashSet<Id>,
    items: HashSet<Id>,
    abilities: HashSet<Id>,
    species: HashSet<Id>,
}

impl Legality {
    /// The availability lists for the given regulation.
    pub fn for_regulation(regulation: Regulation) -> Result<Self> {
        let file: LegalityFile = match regulation {
            // Reg M-B has no verified lists of its own yet.
            Regulation::MC | Regulation::MB => {
                serde_json::from_str(include_str!("../data/mc/legality.json"))
                    .context("failed to parse legality lists")?
            }
        };
        let set = |ids: Vec<String>| ids.into_iter().map(Id::from).collect();
        Ok(Self {
            moves: set(file.moves),
            items: set(file.items),
            abilities: set(file.abilities),
            species: set(file.species),
        })
    }

    pub fn is_move_legal(&self, id: &Id) -> bool {
        self.moves.contains(id)
    }

    pub fn is_item_legal(&self, id: &Id) -> bool {
        self.items.contains(id)
    }

    pub fn is_ability_legal(&self, id: &Id) -> bool {
        self.abilities.contains(id)
    }

    pub fn is_species_legal(&self, id: &Id) -> bool {
        self.species.contains(id)
    }

    pub fn move_count(&self) -> usize {
        self.moves.len()
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    pub fn ability_count(&self) -> usize {
        self.abilities.len()
    }

    pub fn species_count(&self) -> usize {
        self.species.len()
    }
}

#[cfg(test)]
mod legality_test {
    use battler_data::Id;

    use super::Legality;
    use crate::Regulation;

    #[test]
    fn matches_the_exported_lists() {
        let legality = Legality::for_regulation(Regulation::MC).unwrap();
        assert_eq!(legality.move_count(), 515);
        assert_eq!(legality.item_count(), 166);
        assert_eq!(legality.ability_count(), 316);
        assert_eq!(legality.species_count(), 392);
        assert!(legality.is_species_legal(&Id::from("Garchomp")));
        assert!(legality.is_item_legal(&Id::from("Garchompite Z")));
        assert!(legality.is_ability_legal(&Id::from("Dragonize")));
        assert!(legality.is_move_legal(&Id::from("Protect")));
        // Unavailable in Champions.
        assert!(!legality.is_move_legal(&Id::from("Tera Blast")));
        assert!(!legality.is_item_legal(&Id::from("Eject Pack")));
        assert!(legality.is_item_legal(&Id::from("Eject Button")));
    }
}
