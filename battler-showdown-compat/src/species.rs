//! Resolving Showdown species IDs to battler species.
//!
//! Most IDs are identical. The exceptions are gendered formes (`indeedeef`), battler's `-Breed` /
//! `-Plumage` / `-Mask` suffixes, a swapped Maushold naming, and cosmetic formes that battler does
//! not model separately (Alcremie flavors, Vivillon patterns, ...).

use anyhow::{
    Result,
    bail,
};
use battler_data::{
    DataStore,
    Gender,
    Id,
};

use crate::names::{
    species_name,
    to_id,
};

/// A Showdown species resolved to battler data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSpecies {
    /// battler's species name.
    pub name: String,
    /// A gender implied by the Showdown forme (`Indeedee-F`).
    pub gender: Option<Gender>,
    /// Showdown's spelling of the species, for printing protocol lines.
    pub showdown_name: String,
    /// The Showdown forme is cosmetic and was folded into its base species.
    pub cosmetic: bool,
}

fn exists(data: &dyn DataStore, id: &str) -> Result<Option<String>> {
    Ok(data.get_species(&Id::from(id))?.map(|s| s.name))
}

/// Alternate battler IDs for a Showdown species ID, tried in order.
fn alternates(id: &str) -> (Vec<String>, Option<Gender>) {
    // Names that exist in both but mean different things.
    match id {
        "maushold" => return (vec!["mausholdfamilyofthree".to_owned()], None),
        "mausholdfour" => return (vec!["maushold".to_owned()], None),
        "meowsticfmega" | "meowsticmmega" => return (vec!["meowsticmega".to_owned()], None),
        _ => (),
    }
    let mut out = Vec::new();
    let mut gender = None;
    if let Some(base) = id.strip_suffix('f')
        && matches!(base, "indeedee" | "meowstic" | "basculegion" | "oinkologne")
    {
        out.push(format!("{base}female"));
        gender = Some(Gender::Female);
    }
    if id.starts_with("taurospaldea") {
        out.push(format!("{id}breed"));
    }
    if id.starts_with("squawkabilly") {
        out.push(format!("{id}plumage"));
    }
    if let Some(rest) = id.strip_prefix("ogerpon")
        && !rest.is_empty()
    {
        match rest.strip_suffix("tera") {
            Some(mask) => out.push(format!("ogerpon{mask}masktera")),
            None => out.push(format!("ogerpon{rest}mask")),
        }
    }
    (out, gender)
}

/// Resolves a Showdown species ID (or name) for battler.
pub fn resolve(data: &dyn DataStore, showdown_species: &str) -> Result<ResolvedSpecies> {
    let id = to_id(showdown_species);
    let showdown_name = species_name(&id);
    let (alts, gender) = alternates(&id);
    for alt in &alts {
        if let Some(name) = exists(data, alt)? {
            return Ok(ResolvedSpecies {
                name,
                gender,
                showdown_name,
                cosmetic: false,
            });
        }
    }
    if let Some(name) = exists(data, &id)? {
        return Ok(ResolvedSpecies {
            name,
            gender: None,
            showdown_name,
            cosmetic: false,
        });
    }
    // Cosmetic formes: the longest existing species ID that prefixes this one.
    for len in (4..id.len()).rev() {
        if !id.is_char_boundary(len) {
            continue;
        }
        if let Some(name) = exists(data, &id[..len])? {
            return Ok(ResolvedSpecies {
                name,
                gender: None,
                showdown_name,
                cosmetic: true,
            });
        }
    }
    bail!("species {showdown_species} does not exist in battler data")
}
