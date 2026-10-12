//! Showdown's canonical names, by ID.
//!
//! battler and Showdown spell some names differently (apostrophes, forme suffixes, ...), and
//! Showdown's spelling is the reference. The tables are exported from the pinned Showdown build by
//! `scripts/export-showdown-names.js`.

use std::sync::LazyLock;

use ahash::HashMap;
use serde::Deserialize;

#[derive(Deserialize)]
struct NamesFile {
    species: HashMap<String, String>,
    moves: HashMap<String, String>,
    abilities: HashMap<String, String>,
    items: HashMap<String, String>,
}

static NAMES: LazyLock<NamesFile> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../data/names.json")).expect("names.json is valid")
});

/// Normalizes a name into a Showdown/battler ID: lowercase alphanumerics only.
pub fn to_id(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Showdown's name for a species ID, if it exists.
pub fn species(id: &str) -> Option<&'static str> {
    NAMES.species.get(id).map(String::as_str)
}

/// Showdown's name for a move, given any spelling of it.
pub fn move_name(name: &str) -> String {
    NAMES
        .moves
        .get(&to_id(name))
        .cloned()
        .unwrap_or_else(|| name.to_owned())
}

/// Showdown's name for an ability, given any spelling of it.
pub fn ability_name(name: &str) -> String {
    NAMES
        .abilities
        .get(&to_id(name))
        .cloned()
        .unwrap_or_else(|| name.to_owned())
}

/// Showdown's name for an item, given any spelling of it.
pub fn item_name(name: &str) -> String {
    NAMES
        .items
        .get(&to_id(name))
        .cloned()
        .unwrap_or_else(|| name.to_owned())
}

/// Showdown's name for a species, given any spelling of it.
pub fn species_name(name: &str) -> String {
    species(&to_id(name))
        .map(str::to_owned)
        .unwrap_or_else(|| name.to_owned())
}

/// Showdown's name for a battler species that Showdown splits by gender (Meowstic's Mega formes),
/// or `None` if the species is named the same in both.
pub fn gendered_species_name(battler_species: &str, female: bool) -> Option<&'static str> {
    match to_id(battler_species).as_str() {
        "meowsticmega" if female => Some("Meowstic-F-Mega"),
        "meowsticmega" => Some("Meowstic-M-Mega"),
        _ => None,
    }
}

/// All species IDs Showdown knows.
pub fn all_species_ids() -> impl Iterator<Item = &'static str> {
    NAMES.species.keys().map(String::as_str)
}

/// Labels Showdown prints for effects in minor-action lines, by effect kind and ID
/// (`scripts/export-showdown-labels.js`).
#[derive(Deserialize)]
struct LabelsFile {
    moves: HashMap<String, HashMap<String, Vec<String>>>,
    abilities: HashMap<String, HashMap<String, Vec<String>>>,
    items: HashMap<String, HashMap<String, Vec<String>>>,
    conditions: HashMap<String, HashMap<String, Vec<String>>>,
}

static LABELS: LazyLock<LabelsFile> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../data/labels.json")).expect("labels.json is valid")
});

/// The label Showdown prints in a `-{kind}` line (`start`, `end`, `singleturn`, ...) for the effect
/// `name` of kind `effect_kind` (`move`, `ability`, `item` or `condition`), if known.
pub fn label(kind: &str, effect_kind: &str, name: &str) -> Option<String> {
    let id = to_id(name);
    let table = match effect_kind {
        "move" => &LABELS.moves,
        "ability" => &LABELS.abilities,
        "item" => &LABELS.items,
        _ => &LABELS.conditions,
    };
    table.get(&id)?.get(kind)?.first().cloned()
}
