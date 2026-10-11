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

/// Whether the Pokémon's current forme is a permanent one (Showdown rewrites its details).
fn permanent_forme(mon: &MonBattleData) -> bool {
    ["-Mega", "-Primal", "-Ultra"]
        .iter()
        .any(|marker| mon.species.contains(marker))
}

fn details(mon: &MonBattleData, species_override: Option<&str>) -> String {
    let species = match species_override {
        Some(s) => s.to_owned(),
        None if permanent_forme(mon) => species_name(&mon.species),
        None => species_name(&mon.summary.species),
    };
    details_with_species(mon, species)
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
                "details": match side.species.get(&mon.summary.name) {
                    Some(species) if !permanent_forme(mon) => details_with_species(mon, species.clone()),
                    _ => details(mon, None),
                },
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
                "baseAbility": to_id(if permanent_forme(mon) { &mon.ability } else { &mon.summary.ability }),
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
                                    "disabled": m.disabled,
                                })
                            })
                            .collect()
                    };
                    let from_request = turn.active.iter().find(|r| r.team_position == mon.player_team_position);
                    let mut entry = json!({"moves": moves(from_request.map(|r| r.moves.as_slice()).unwrap_or(&mon.moves))});
                    if let Some(r) = from_request {
                        if r.trapped {
                            entry["trapped"] = json!(true);
                        }
                        if r.can_mega_evolve {
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
