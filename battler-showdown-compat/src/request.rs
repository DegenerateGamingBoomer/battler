//! battler requests → Showdown `|request|` JSON.
//!
//! The JSON is built from a battler [`Request`] and the player's [`PlayerBattleData`], and matches
//! the object Showdown sends (`active` with move details, `forceSwitch`, `teamPreview`, `wait`, and
//! `side.pokemon`). The request ID (`rqid`) is added by the caller if it wants one.

use anyhow::Result;
use battler::{
    MonBattleData,
    PlayerBattleData,
    Request,
};
use battler_data::{
    Gender,
    MoveTarget,
};
use serde_json::{
    Value,
    json,
};

use crate::{
    names::{
        species_name,
        to_id,
    },
    protocol::effects::status_id,
};

/// Showdown's spelling of a move target.
pub fn target_id(target: &MoveTarget) -> &'static str {
    match target {
        MoveTarget::AdjacentAlly => "adjacentAlly",
        MoveTarget::AdjacentAllyOrUser => "adjacentAllyOrSelf",
        MoveTarget::AdjacentFoe => "adjacentFoe",
        MoveTarget::All => "all",
        MoveTarget::AllAdjacent => "allAdjacent",
        MoveTarget::AllAdjacentAllies => "allAdjacentAllies",
        MoveTarget::AllAdjacentFoes => "allAdjacentFoes",
        MoveTarget::Allies => "allies",
        MoveTarget::AlliesExceptUser => "allies",
        MoveTarget::AllySide => "allySide",
        MoveTarget::AllyTeam => "allyTeam",
        MoveTarget::Any => "any",
        MoveTarget::Field => "all",
        MoveTarget::FoeSide => "foeSide",
        MoveTarget::Normal => "normal",
        MoveTarget::RandomNormal => "randomNormal",
        MoveTarget::Scripted => "scripted",
        MoveTarget::User => "self",
    }
}

/// How many Pokémon are active on a side.
pub struct SideInfo {
    /// Showdown's player ID (`p1`).
    pub id: String,
    /// The player's name.
    pub name: String,
    /// Number of active positions on the side.
    pub active_count: usize,
    /// Whether more than one side has something to answer. Showdown only forbids cancelling a
    /// choice when the other side is not choosing at the same time.
    pub multiple_requests: bool,
    /// Showdown's species name by nickname, for formes battler folds into their base species
    /// (cosmetic formes). Empty if unknown.
    pub species: ahash::HashMap<String, String>,
}

/// The species Showdown shows in the Pokémon's details: its base species, which changes on
/// permanent forme changes (Mega Evolution, Busted Mimikyu, Hero Palafin).
fn details_species(side: &SideInfo, mon: &MonBattleData) -> String {
    let base_species = if mon.base_species.is_empty() {
        &mon.summary.species
    } else {
        &mon.base_species
    };
    if to_id(base_species) == to_id(&mon.summary.species) {
        if let Some(species) = side.species.get(&mon.summary.name) {
            return species.clone();
        }
    }
    if let Some(name) =
        crate::names::gendered_species_name(base_species, mon.summary.gender == Gender::Female)
    {
        return name.to_owned();
    }
    species_name(base_species)
}

fn details_with_species(mon: &MonBattleData, species: String) -> String {
    let mut out = species;
    if mon.summary.level != 100 {
        out.push_str(&format!(", L{}", mon.summary.level));
    }
    match mon.summary.gender {
        Gender::Male => out.push_str(", M"),
        Gender::Female => out.push_str(", F"),
        Gender::Unknown => (),
    }
    if mon.summary.shiny {
        out.push_str(", shiny");
    }
    out
}

fn condition(mon: &MonBattleData) -> String {
    if mon.hp == 0 {
        return "0 fnt".to_owned();
    }
    match &mon.status {
        Some(status) => format!("{}/{} {}", mon.hp, mon.max_hp, status_id(status)),
        None => format!("{}/{}", mon.hp, mon.max_hp),
    }
}

/// The `side` object of a request.
pub fn side(side: &SideInfo, player: &PlayerBattleData, team_preview: bool) -> Value {
    let mut mons: Vec<&MonBattleData> = player.mons.iter().collect();
    mons.sort_by_key(|m| m.player_effective_team_position);
    let pokemon: Vec<Value> = mons
        .iter()
        .enumerate()
        .map(|(i, mon)| {
            let stat = |s: battler::Stat| mon.stats.get(&s).copied();
            // Team preview lists the first positions as active.
            let active = if team_preview { i < side.active_count } else { mon.active };
            json!({
                "ident": format!("{}: {}", side.id, mon.summary.name),
                "details": details_with_species(mon, details_species(side, mon)),
                "condition": condition(mon),
                "active": active,
                "stats": {
                    "atk": stat(battler::Stat::Atk),
                    "def": stat(battler::Stat::Def),
                    "spa": stat(battler::Stat::SpAtk),
                    "spd": stat(battler::Stat::SpDef),
                    "spe": stat(battler::Stat::Spe),
                },
                "moves": mon.moves.iter().map(|m| m.id.to_string()).collect::<Vec<_>>(),
                "baseAbility": to_id(if mon.base_ability.is_empty() { &mon.summary.ability } else { &mon.base_ability }),
                "item": mon.item.as_deref().map(to_id).unwrap_or_default(),
                "pokeball": mon.summary.ball.as_deref().map(to_id).filter(|b| !b.is_empty()).unwrap_or_else(|| "pokeball".to_owned()),
                "ability": to_id(&mon.ability),
                "commanding": false,
                "reviving": false,
            })
        })
        .collect();
    json!({"name": side.name, "id": side.id, "pokemon": pokemon})
}

/// Translates a request. `request` is `None` when the player has nothing to answer.
///
/// Returns `None` if the battle is over and there is nothing to send.
pub fn translate(
    request: Option<&Request>,
    side_info: &SideInfo,
    player: &PlayerBattleData,
    battle_ended: bool,
) -> Result<Option<Value>> {
    let Some(request) = request else {
        if battle_ended {
            return Ok(None);
        }
        return Ok(Some(
            json!({"wait": true, "side": side(side_info, player, false)}),
        ));
    };
    let value = match request {
        Request::TeamPreview(preview) => json!({
            "teamPreview": true,
            "maxChosenTeamSize": preview.max_team_size.unwrap_or(player.mons.len()),
            "side": side(side_info, player, true),
        }),
        Request::Turn(turn) => {
            // One entry per active position, in position order.
            let mut actives: Vec<&MonBattleData> =
                player.mons.iter().filter(|m| m.active).collect();
            actives.sort_by_key(|m| m.player_active_position);
            let entries = actives
                .iter()
                .map(|mon| {
                    let moves = |slots: &[battler::MonMoveSlotData]| -> Vec<Value> {
                        slots
                            .iter()
                            .map(|m| {
                                json!({
                                    "move": crate::names::move_name(&m.name),
                                    "id": m.id.to_string(),
                                    "pp": m.pp,
                                    "maxpp": m.max_pp,
                                    "target": target_id(&m.target),
                                    // Showdown disables moves without PP.
                                    "disabled": m.disabled || m.pp == 0,
                                })
                            })
                            .collect()
                    };
                    let from_request = turn.active.iter().find(|r| r.team_position == mon.player_team_position);
                    let mut entry = json!({"moves": moves(from_request.map(|r| r.moves.as_slice()).unwrap_or(&mon.moves))});
                    if let Some(r) = from_request {
                        // A Pokémon locked into a move (charging, recharging, rampaging) only shows
                        // that move's name and ID, and is trapped.
                        if r.locked_into_move {
                            entry["moves"] = json!(r.moves.iter().map(|m| json!({
                                "move": crate::names::move_name(&m.name),
                                "id": m.id.to_string(),
                            })).collect::<Vec<_>>());
                        }
                        if r.trapped || r.locked_into_move {
                            entry["trapped"] = json!(true);
                        }
                        // Without a usable move, the only option is Struggle, which also rules out
                        // Mega Evolution.
                        let struggle = !r.locked_into_move
                            && r.moves.iter().all(|m| m.disabled || m.pp == 0);
                        if struggle {
                            entry["moves"] = json!([{
                                "move": "Struggle",
                                "id": "struggle",
                                "target": "randomNormal",
                                "disabled": false,
                            }]);
                        }
                        if r.can_mega_evolve && !r.locked_into_move && !struggle {
                            entry["canMegaEvo"] = json!(true);
                        }
                    }
                    entry
                })
                .collect::<Vec<_>>();
            json!({"active": entries, "side": side(side_info, player, false)})
        }
        Request::Switch(switch) => {
            let flags: Vec<bool> = (0..side_info.active_count)
                .map(|i| switch.needs_switch.contains(&i))
                .collect();
            let mut value = json!({"forceSwitch": flags, "side": side(side_info, player, false)});
            if !side_info.multiple_requests {
                value["noCancel"] = json!(true);
            }
            value
        }
        other => anyhow::bail!("request {other:?} has no Showdown equivalent"),
    };
    Ok(Some(value))
}
