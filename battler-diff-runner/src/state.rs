//! Normalized battle state of a battler battle, in the shape of `compat/diff/state.js`.
//!
//! Showdown's IDs are canonical, so battler names that differ are mapped here.

use std::collections::HashMap;

use anyhow::Result;
use battler::{
    Gender,
    Id,
    PublicCoreBattle,
    battle::{
        Context,
        Mon,
        MonHandle,
    },
    effect::fxlang::{
        EffectState,
        Value,
    },
};
use battler_showdown_compat::{
    names::{
        gendered_species_name,
        species_name,
        to_id,
    },
    team::ConvertedTeam,
};
use serde_json::{
    Map,
    Value as Json,
    json,
};

/// battler weather condition → Showdown weather.
fn weather_id(id: &str) -> String {
    match id {
        "rainweather" => "raindance",
        "heavyrainweather" => "primordialsea",
        "harshsunlight" => "sunnyday",
        "extremelyharshsunlight" => "desolateland",
        "sandstormweather" => "sandstorm",
        "hailweather" => "hail",
        "snowweather" => "snowscape",
        "strongwinds" => "deltastream",
        other => other,
    }
    .to_owned()
}

fn number(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Fraction(f) => Some(f.floor()),
        Value::UFraction(f) => Some(f.floor() as i64),
        _ => None,
    }
}

fn id_value(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(s) => Some(to_id(s)),
        Value::Effect(handle) => handle.try_id().map(|id| id.to_string()),
        _ => None,
    }
}

fn duration(state: &EffectState) -> Option<u8> {
    state.duration()
}

fn volatile_state(id: &str, state: &EffectState) -> Json {
    let mut out = Map::new();
    if let Some(duration) = duration(state) {
        out.insert("duration".to_owned(), json!(duration));
    }
    match id {
        "confusion" => {
            out.insert("time".to_owned(), json!(number(state.get("time"))));
        }
        "substitute" => {
            out.insert("hp".to_owned(), json!(number(state.get("hp"))));
        }
        "stall" => {
            out.insert("counter".to_owned(), json!(number(state.get("counter"))));
        }
        "encore" | "disable" | "choicelock" | "twoturnmove" | "lockedmove" => {
            out.insert("move".to_owned(), json!(id_value(state.get("move"))));
        }
        "stockpile" => {
            out.insert("layers".to_owned(), json!(number(state.get("layers"))));
        }
        "partiallytrapped" => {
            out.insert(
                "source".to_owned(),
                json!(
                    state
                        .source_effect()
                        .and_then(|h| h.try_id().map(|id| id.to_string()))
                ),
            );
        }
        _ => (),
    }
    Json::Object(out)
}

fn status_state(mon: &Mon) -> Json {
    match mon.status.as_ref().map(|s| s.as_ref()) {
        Some("slp") => json!({"time": number(mon.status_state.get("time"))}),
        Some("tox") => json!({"stage": number(mon.status_state.get("stage"))}),
        Some("frz") => match number(mon.status_state.get("time")) {
            Some(time) => json!({"time": time}),
            None => json!({}),
        },
        _ => json!({}),
    }
}

fn boosts(mon: &Mon) -> Json {
    let b = &mon.volatile_state.boosts;
    let mut out = Map::new();
    for (stat, value) in [
        ("atk", b.atk),
        ("def", b.def),
        ("spa", b.spa),
        ("spd", b.spd),
        ("spe", b.spe),
        ("accuracy", b.acc),
        ("evasion", b.eva),
    ] {
        if value != 0 {
            out.insert(stat.to_owned(), json!(value));
        }
    }
    Json::Object(out)
}

fn status_id(mon: &Mon) -> String {
    mon.status
        .as_ref()
        .map(|s| s.to_string())
        .unwrap_or_default()
}

/// Showdown species IDs of each Mon's own species, by nickname. A Mon whose species is still the
/// one it started as is reported with its Showdown species; forme changes are reported with
/// battler's ID, which matches Showdown's for the formes in Champions.
pub struct SpeciesNames {
    by_name: HashMap<String, (String, String)>,
    display: HashMap<String, String>,
}

impl SpeciesNames {
    pub fn new(team: &ConvertedTeam) -> Self {
        let by_name = team
            .sets
            .iter()
            .zip(&team.species)
            .map(|(set, resolved)| {
                let name = if set.name.is_empty() {
                    &set.species
                } else {
                    &set.name
                };
                (name.clone(), (to_id(&resolved.name), to_id(&set.species)))
            })
            .collect();
        let display = team
            .sets
            .iter()
            .map(|set| (to_id(&set.species), species_name(&set.species)))
            .collect();
        Self { by_name, display }
    }

    /// Showdown's species name (not ID) by nickname.
    pub fn showdown_names(&self) -> ahash::HashMap<String, String> {
        self.by_name
            .iter()
            .map(|(name, (_, showdown))| (name.clone(), self.display[showdown].clone()))
            .collect()
    }

    fn species(&self, mon: &Mon) -> String {
        let id = mon.volatile_state.species.to_string();
        match self.by_name.get(&mon.name) {
            Some((battler, showdown)) if *battler == id => showdown.clone(),
            _ => match gendered_species_name(&id, mon.gender == Gender::Female) {
                Some(name) => to_id(name),
                None => id,
            },
        }
    }
}

fn active(mon: &Mon, names: &SpeciesNames) -> Json {
    let mut volatiles = Map::new();
    for (id, state) in &mon.volatile_state.volatiles {
        volatiles.insert(id.to_string(), volatile_state(id.as_ref(), state));
    }
    json!({
        "name": mon.name,
        "species": names.species(mon),
        "hp": mon.hp,
        "maxhp": mon.max_hp,
        "status": status_id(mon),
        "statusState": status_state(mon),
        "boosts": boosts(mon),
        "volatiles": volatiles,
        "activeTurns": mon.active_turns,
        "moveActions": mon.active_move_actions,
    })
}

/// Showdown's ID for an item, from battler's name for it (battler keeps some older IDs, such as
/// `stick` for Leek).
fn item_id(context: &Context, id: &Id) -> String {
    match context.battle().dex.items.get_by_id(id) {
        Ok(item) => to_id(&item.data.name),
        Err(_) => id.to_string(),
    }
}

fn ability_id(context: &Context, id: &Id) -> String {
    match context.battle().dex.abilities.get_by_id(id) {
        Ok(ability) => to_id(&ability.data.name),
        Err(_) => id.to_string(),
    }
}

fn member(mon: &Mon, names: &SpeciesNames, context: &Context) -> Json {
    json!({
        "species": names.species(mon),
        "hp": mon.hp,
        "maxhp": mon.max_hp,
        "status": if mon.hp == 0 { String::new() } else { status_id(mon) },
        "fainted": mon.hp == 0,
        "item": mon.item.as_ref().map(|i| item_id(context, i)).unwrap_or_default(),
        "ability": ability_id(context, &mon.volatile_state.ability_slot.ability.id),
        "pp": mon.volatile_state.move_slots.iter().map(|m| json!({"id": m.id.to_string(), "pp": m.pp, "maxpp": m.max_pp})).collect::<Vec<_>>(),
    })
}

fn conditions<'a>(map: impl Iterator<Item = (&'a Id, &'a EffectState)>) -> Json {
    let mut out = Map::new();
    for (id, state) in map {
        // The marker for a pending Revival Blessing choice, not a condition in battle.
        if id.as_ref() == "revivalblessing" {
            continue;
        }
        let mut entry = Map::new();
        if let Some(duration) = duration(state) {
            entry.insert("duration".to_owned(), json!(duration));
        }
        if let Some(layers) = number(state.get("layers")) {
            entry.insert("layers".to_owned(), json!(layers));
        }
        out.insert(id.to_string(), Json::Object(entry));
    }
    Json::Object(out)
}

/// The normalized state of `battle`. `players` are battler player IDs in Showdown side order.
pub fn normalize(
    battle: &mut PublicCoreBattle,
    players: &[String],
    names: &[SpeciesNames],
) -> Result<Json> {
    let turn = battle.internal.turn();
    let field = &battle.internal.field;
    let pseudo = conditions(field.pseudo_weathers.iter());
    let weather = field.weather.as_ref().map(|w| weather_id(w.as_ref()));
    let terrain = field.terrain.as_ref().map(|t| t.to_string());
    let field_json = json!({
        "weather": weather.clone().unwrap_or_default(),
        "weatherDuration": weather.as_ref().and_then(|_| duration(&field.weather_state)),
        "terrain": terrain.clone().unwrap_or_default(),
        "terrainDuration": terrain.as_ref().and_then(|_| duration(&field.terrain_state)),
        "pseudo": pseudo,
    });

    let mut sides = Vec::new();
    for (player_id, names) in players.iter().zip(names) {
        let player_index = battle
            .internal
            .players
            .iter()
            .position(|p| &p.id == player_id)
            .ok_or_else(|| anyhow::anyhow!("no player {player_id}"))?;
        let side_index = battle.internal.players[player_index].side;
        let side = &battle.internal.sides[side_index];
        let side_conditions = conditions(side.conditions.iter());
        let positions = battle.internal.players[player_index].total_active_positions();
        let slot_conditions = (0..positions)
            .map(|slot| match side.slot_conditions.get(&slot) {
                Some(map) => conditions(map.iter()),
                None => json!({}),
            })
            .collect::<Vec<_>>();
        let active_handles: Vec<Option<MonHandle>> = (0..positions)
            .map(|i| battle.internal.players[player_index].active_mon_handle(i))
            .collect();
        let exited_handles: Vec<Option<MonHandle>> = (0..positions)
            .map(|i| battle.internal.players[player_index].active_or_exited_mon_handle(i))
            .collect();
        let handles: Vec<MonHandle> = battle.internal.players[player_index]
            .mon_handles()
            .cloned()
            .collect();
        let context = Context::new(&mut battle.internal);
        let active_json = active_handles
            .iter()
            .zip(&exited_handles)
            .map(|(handle, exited)| -> Result<Json> {
                // A Pokémon waiting for its mid-turn replacement (U-turn, Eject Button, ...) has
                // left its slot already; Showdown keeps it there until the switch.
                if handle.is_none()
                    && let Some(exited) = exited
                {
                    let mon = context.mon(*exited)?;
                    if mon.hp > 0 {
                        return Ok(json!({"name": mon.name, "switching": true}));
                    }
                }
                match handle {
                    Some(handle) => {
                        let mon = context.mon(*handle)?;
                        Ok(if mon.hp == 0 {
                            Json::Null
                        } else {
                            active(&mon, names)
                        })
                    }
                    None => Ok(Json::Null),
                }
            })
            .collect::<Result<Vec<_>>>()?;
        let mut team = Map::new();
        for handle in &handles {
            let mon = context.mon(*handle)?;
            team.insert(mon.name.clone(), member(&mon, names, &context));
        }
        sides.push(json!({
            "conditions": side_conditions,
            "slotConditions": slot_conditions,
            "active": active_json,
            "team": team,
        }));
    }

    Ok(json!({
        "turn": turn,
        "field": field_json,
        "sides": sides,
    }))
}
