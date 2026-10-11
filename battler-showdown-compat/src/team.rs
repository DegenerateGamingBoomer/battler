//! Showdown teams → battler teams.
//!
//! Both Showdown's packed format (`Teams.pack`) and its exported (importable) text format are read.
//! IVs default to 31 as in Showdown. Under Champions, the EVs field carries stat points and is
//! passed through unchanged.

use anyhow::{
    Context,
    Result,
    bail,
};
use battler::TeamData;
use battler_data::{
    DataStore,
    Id,
};
use serde_json::{
    Map,
    Value,
    json,
};

use crate::{
    names::to_id,
    species::{
        ResolvedSpecies,
        resolve,
    },
};

/// A Pokémon set as Showdown describes it (`PokemonSet`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShowdownSet {
    pub name: String,
    pub species: String,
    pub item: String,
    pub ability: String,
    pub moves: Vec<String>,
    pub nature: String,
    pub evs: [u16; 6],
    pub gender: String,
    pub ivs: [u16; 6],
    pub shiny: bool,
    pub level: Option<u8>,
    pub happiness: Option<u8>,
    pub hp_type: String,
    pub pokeball: String,
}

const STATS: [&str; 6] = ["hp", "atk", "def", "spa", "spd", "spe"];

/// Options for converting teams.
#[derive(Debug, Clone, Copy)]
pub struct ConvertOptions {
    /// Level of sets that do not name one (Showdown: 100).
    pub default_level: u8,
    /// Forces every set to this level (Champions VGC: 50).
    pub force_level: Option<u8>,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            default_level: 100,
            force_level: None,
        }
    }
}

/// A converted team, with what is needed to print Showdown's spelling of its members later.
#[derive(Debug, Clone)]
pub struct ConvertedTeam {
    pub team: TeamData,
    /// How each member resolved, in team order.
    pub species: Vec<ResolvedSpecies>,
    /// The Showdown sets, in team order.
    pub sets: Vec<ShowdownSet>,
}

fn parse_stats(field: &str, default: u16) -> [u16; 6] {
    let mut out = [default; 6];
    if field.is_empty() {
        return out;
    }
    for (i, part) in field.split(',').take(6).enumerate() {
        out[i] = if part.is_empty() {
            default
        } else {
            part.parse().unwrap_or(0)
        };
    }
    out
}

/// Parses Showdown's packed team format.
pub fn parse_packed(packed: &str) -> Result<Vec<ShowdownSet>> {
    let mut sets = Vec::new();
    for entry in packed.split(']').filter(|e| !e.is_empty()) {
        let f: Vec<&str> = entry.split('|').collect();
        if f.len() < 11 {
            bail!(
                "packed set has {} fields, expected at least 11: {entry}",
                f.len()
            );
        }
        let name = f[0].to_owned();
        let species = if f[1].is_empty() { f[0] } else { f[1] }.to_owned();
        // The last field may carry `happiness,hpType,pokeball,gigantamax,dynamaxLevel,teraType`.
        let misc: Vec<&str> = f
            .get(11)
            .map(|m| m.split(',').collect())
            .unwrap_or_default();
        sets.push(ShowdownSet {
            name,
            species,
            item: f[2].to_owned(),
            ability: f[3].to_owned(),
            moves: f[4]
                .split(',')
                .filter(|m| !m.is_empty())
                .map(str::to_owned)
                .collect(),
            nature: f[5].to_owned(),
            evs: parse_stats(f[6], 0),
            gender: f[7].to_owned(),
            ivs: parse_stats(f[8], 31),
            shiny: !f[9].is_empty(),
            level: f[10].parse().ok(),
            happiness: misc.first().and_then(|h| h.parse().ok()),
            hp_type: misc.get(1).map(|s| s.to_string()).unwrap_or_default(),
            pokeball: misc.get(2).map(|s| s.to_string()).unwrap_or_default(),
        });
    }
    Ok(sets)
}

fn stat_index(name: &str) -> Option<usize> {
    Some(match name.trim().to_ascii_lowercase().as_str() {
        "hp" => 0,
        "atk" => 1,
        "def" => 2,
        "spa" | "spatk" => 3,
        "spd" | "spdef" => 4,
        "spe" | "spd." | "speed" => 5,
        _ => return None,
    })
}

/// Parses Showdown's exported (importable) team format.
pub fn parse_exported(text: &str) -> Result<Vec<ShowdownSet>> {
    let mut sets = Vec::new();
    let mut current: Option<ShowdownSet> = None;
    for line in text.lines().map(str::trim) {
        if line.is_empty() {
            if let Some(set) = current.take() {
                sets.push(set);
            }
            continue;
        }
        let set = match &mut current {
            Some(set) => set,
            None => {
                // `Nickname (Species) (M) @ Item`
                let mut head = line;
                let mut item = String::new();
                if let Some((h, i)) = head.split_once(" @ ") {
                    head = h.trim();
                    item = i.trim().to_owned();
                }
                let mut gender = String::new();
                for g in [" (M)", " (F)"] {
                    if let Some(h) = head.strip_suffix(g) {
                        gender = g[2..3].to_owned();
                        head = h.trim();
                    }
                }
                let (name, species) = match head.strip_suffix(')').and_then(|h| h.rsplit_once(" ("))
                {
                    Some((name, species)) => (name.trim().to_owned(), species.trim().to_owned()),
                    None => (head.to_owned(), head.to_owned()),
                };
                current = Some(ShowdownSet {
                    name,
                    species,
                    item,
                    gender,
                    ivs: [31; 6],
                    ..Default::default()
                });
                continue;
            }
        };
        if let Some(rest) = line.strip_prefix("- ") {
            set.moves.push(rest.trim().to_owned());
        } else if let Some(rest) = line.strip_prefix("Ability:") {
            set.ability = rest.trim().to_owned();
        } else if let Some(rest) = line.strip_prefix("Level:") {
            set.level = rest.trim().parse().ok();
        } else if let Some(rest) = line.strip_prefix("Happiness:") {
            set.happiness = rest.trim().parse().ok();
        } else if let Some(rest) = line.strip_prefix("Shiny:") {
            set.shiny = rest.trim().eq_ignore_ascii_case("yes");
        } else if let Some(rest) = line.strip_prefix("Hidden Power:") {
            set.hp_type = rest.trim().to_owned();
        } else if let Some(rest) = line.strip_prefix("EVs:") {
            for part in rest.split('/') {
                let mut it = part.trim().splitn(2, ' ');
                let value: u16 = it.next().unwrap_or("0").parse().unwrap_or(0);
                if let Some(i) = it.next().and_then(stat_index) {
                    set.evs[i] = value;
                }
            }
        } else if let Some(rest) = line.strip_prefix("IVs:") {
            for part in rest.split('/') {
                let mut it = part.trim().splitn(2, ' ');
                let value: u16 = it.next().unwrap_or("31").parse().unwrap_or(31);
                if let Some(i) = it.next().and_then(stat_index) {
                    set.ivs[i] = value;
                }
            }
        } else if let Some(nature) = line.strip_suffix(" Nature") {
            set.nature = nature.trim().to_owned();
        }
        // Unknown lines (Tera Type, Gigantamax, ...) are ignored.
    }
    if let Some(set) = current.take() {
        sets.push(set);
    }
    Ok(sets)
}

/// Parses a team in either format (packed if it contains no newline and has a `|`).
pub fn parse(text: &str) -> Result<Vec<ShowdownSet>> {
    if !text.contains('\n') && text.contains('|') {
        parse_packed(text)
    } else {
        parse_exported(text)
    }
}

fn gender_value(set: &ShowdownSet, implied: Option<battler_data::Gender>) -> &'static str {
    match set.gender.as_str() {
        "M" => "M",
        "F" => "F",
        "N" => "U",
        _ => match implied {
            Some(battler_data::Gender::Female) => "F",
            _ => "M",
        },
    }
}

/// Converts Showdown sets to a battler team.
pub fn convert(
    data: &dyn DataStore,
    sets: &[ShowdownSet],
    options: ConvertOptions,
) -> Result<ConvertedTeam> {
    let mut members = Vec::new();
    let mut resolved = Vec::new();
    for set in sets {
        let species = resolve(data, &set.species)
            .with_context(|| format!("while converting {}", set.name))?;
        let level = options
            .force_level
            .or(set.level)
            .unwrap_or(options.default_level);
        let stats = |values: &[u16; 6]| -> Value {
            let mut map = Map::new();
            for (stat, value) in STATS.iter().zip(values) {
                map.insert((*stat).to_owned(), json!(value));
            }
            Value::Object(map)
        };
        // An ability may be a slot index in packed teams.
        let ability = match set.ability.as_str() {
            "0" | "1" | "H" | "S" | "" => {
                let data = data
                    .get_species(&Id::from(species.name.as_str()))?
                    .context("species disappeared")?;
                match set.ability.as_str() {
                    "1" => data.abilities.get(1).cloned(),
                    "H" => Some(data.hidden_ability.clone()).flatten(),
                    _ => data.abilities.first().cloned(),
                }
                .unwrap_or_default()
            }
            other => other.to_owned(),
        };
        let mut member = json!({
            "name": species_nickname(set),
            "species": species.name,
            "ability": ability,
            "moves": set.moves,
            "item": if set.item.is_empty() { Value::Null } else { json!(set.item) },
            "nature": if set.nature.is_empty() { "Hardy" } else { set.nature.as_str() },
            "gender": gender_value(set, species.gender),
            "level": level,
            "shiny": set.shiny,
            "evs": stats(&set.evs),
            "ivs": stats(&set.ivs),
            "friendship": set.happiness.unwrap_or(255),
        });
        if !set.pokeball.is_empty() {
            member["ball"] = json!(set.pokeball);
        }
        members.push(member);
        resolved.push(species);
    }
    let team: TeamData = serde_json::from_value(json!({ "members": members }))
        .context("converted team is not valid battler data")?;
    Ok(ConvertedTeam {
        team,
        species: resolved,
        sets: sets.to_vec(),
    })
}

/// The nickname battler should use: Showdown's name, which defaults to the species name.
fn species_nickname(set: &ShowdownSet) -> &str {
    if set.name.is_empty() {
        &set.species
    } else {
        &set.name
    }
}

/// Parses and converts a team in either Showdown format.
pub fn convert_text(
    data: &dyn DataStore,
    text: &str,
    options: ConvertOptions,
) -> Result<ConvertedTeam> {
    convert(data, &parse(text)?, options)
}

/// The ID Showdown would use for a set's move, item, ability or species.
pub fn id_of(name: &str) -> String {
    to_id(name)
}
