//! battler requests → Showdown `|request|` JSON.
//!
//! The JSON is built from a battler [`Request`] and the player's [`PlayerBattleData`], and matches
//! the object Showdown sends (`active` with move details, `forceSwitch`, `teamPreview`, `wait`, and
//! `side.pokemon`). The request ID (`rqid`) is added by the caller if it wants one.

use anyhow::{
    Context,
    Result,
};
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
    /// What Showdown reveals about trapping (see [`trap_info`]).
    pub trap: TrapInfo,
}

/// What Showdown's requests reveal about trapping, by active position.
///
/// Showdown keeps a trap from an ability (Shadow Tag, Arena Trap, Magnet Pull) hidden from the
/// last active Pokémon, and marks it `maybeTrapped` when a foe's species could have such an
/// ability, so that cancelling a switch reveals nothing (`sim/battle.ts` `endTurn`,
/// `sim/pokemon.ts` `getMoveRequestData`).
#[derive(Debug, Default, Clone)]
pub struct TrapInfo {
    /// Positions trapped by something visible (Block, Mean Look, binding moves, Ingrain, No
    /// Retreat, Octolock, Fairy Lock), as opposed to an ability.
    pub visible: Vec<usize>,
    /// Positions a foe could trap with a trapping ability of its species or its current one.
    pub maybe: Vec<usize>,
}

const VISIBLE_TRAPS: [&str; 5] = [
    "trapped",
    "partiallytrapped",
    "ingrain",
    "noretreat",
    "octolock",
];

/// Whether a trapping ability would trap `mon` (Showdown's `onFoeMaybeTrapPokemon`).
fn ability_may_trap(
    context: &mut battler::battle::Context,
    ability: &str,
    mon: battler::battle::MonHandle,
) -> Result<bool> {
    let mut context = context.mon_context(mon)?;
    Ok(match ability {
        "shadowtag" => {
            context
                .mon()
                .volatile_state
                .ability_slot
                .ability
                .id
                .as_ref()
                != "shadowtag"
        }
        "arenatrap" => battler::battle::mon_states::is_grounded(&mut context),
        "magnetpull" => battler::battle::mon_states::has_type(&mut context, battler::Type::Steel),
        _ => false,
    })
}

/// [`TrapInfo`] for a player's active Pokémon.
pub fn trap_info(battle: &mut battler::PublicCoreBattle, player: &str) -> Result<TrapInfo> {
    let mut info = TrapInfo::default();
    let core = &mut battle.internal;
    let player_index = core
        .players
        .iter()
        .position(|p| p.id == player)
        .with_context(|| format!("no player {player}"))?;
    let side = core.players[player_index].side;
    let positions = core.players[player_index].total_active_positions();
    let actives: Vec<(usize, battler::battle::MonHandle)> = (0..positions)
        .filter_map(|i| {
            core.players[player_index]
                .active_mon_handle(i)
                .map(|h| (i, h))
        })
        .collect();
    let foes: Vec<battler::battle::MonHandle> = core
        .players
        .iter()
        .filter(|p| p.side != side)
        .flat_map(|p| p.active_mon_handles().cloned().collect::<Vec<_>>())
        .collect();
    let fairy_lock = core
        .field
        .pseudo_weathers
        .contains_key(&battler::Id::from_known("fairylock"));
    let mut context = core.context();
    for (position, handle) in actives {
        let mon = context.mon(handle)?;
        if mon.hp == 0 {
            continue;
        }
        if fairy_lock
            || VISIBLE_TRAPS.iter().any(|v| {
                mon.volatile_state
                    .volatiles
                    .contains_key(&battler::Id::from(*v))
            })
        {
            info.visible.push(position);
        }
        // A Pokémon immune to trapping (Ghost) is never marked.
        if battler::battle::mon_states::has_type(
            &mut context.mon_context(handle)?,
            battler::Type::Ghost,
        ) {
            continue;
        }
        let item = context.mon(handle)?.item.clone();
        let escapes = item.as_ref().is_some_and(|i| i.as_ref() == "shedshell")
            || context
                .mon(handle)?
                .volatile_state
                .ability_slot
                .ability
                .id
                .as_ref()
                == "runaway";
        let mut maybe = false;
        for foe in &foes {
            let foe_mon = context.mon(*foe)?;
            if foe_mon.hp == 0 {
                continue;
            }
            let current = foe_mon.volatile_state.ability_slot.ability.id.to_string();
            let species = context
                .battle()
                .dex
                .species
                .get_by_id(&foe_mon.volatile_state.species)?;
            let mut abilities: Vec<String> = species
                .data
                .abilities
                .iter()
                .chain(species.data.hidden_ability.iter())
                .map(|a| crate::names::to_id(a))
                .collect();
            drop(species);
            abilities.dedup();
            // The foe's own ability (the event Shed Shell and Run Away answer), then every ability
            // its species could have (which they do not).
            if !escapes && ability_may_trap(&mut context, &current, handle)? {
                maybe = true;
            }
            for ability in abilities {
                if ability_may_trap(&mut context, &ability, handle)? {
                    maybe = true;
                }
            }
        }
        if maybe {
            info.maybe.push(position);
        }
    }
    Ok(info)
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
            // Requests show the Mon's own stats, not the stats it transformed into.
            let stat = |s: battler::Stat| {
                if mon.transformed {
                    Some(mon.summary.stats.get(s))
                } else {
                    mon.stats.get(&s).copied()
                }
            };
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
                    // Showdown still describes a fainted Pokémon left in its slot, including whether
                    // it could Mega Evolve.
                    if from_request.is_none() && mon.can_mega_evolve {
                        entry["canMegaEvo"] = json!(true);
                    }
                    if let Some(r) = from_request {
                        // battler locks a Pokémon without usable moves into Struggle; Showdown
                        // offers Struggle as its only move.
                        let locked_into_struggle = r.locked_into_move
                            && r.moves.len() == 1
                            && r.moves[0].id.as_ref() == "struggle";
                        // A Pokémon locked into a move (charging, recharging, rampaging) only shows
                        // that move's name and ID, and is trapped.
                        if r.locked_into_move && !locked_into_struggle {
                            entry["moves"] = json!(r.moves.iter().map(|m| json!({
                                "move": crate::names::move_name(&m.name),
                                "id": m.id.to_string(),
                            })).collect::<Vec<_>>());
                        }
                        // Showdown hides an ability's trap from the last active Pokémon and marks
                        // it as maybe trapped instead (see `TrapInfo`).
                        let hard_locked = r.locked_into_move && !locked_into_struggle;
                        let position = mon.player_active_position.unwrap_or(0);
                        let last_active = !actives.iter().any(|other| {
                            other.player_active_position.unwrap_or(0) > position && other.hp > 0
                        });
                        let can_switch_in = player.mons.iter().any(|m| !m.active && m.hp > 0);
                        if hard_locked || !last_active {
                            if (r.trapped || hard_locked) && (hard_locked || can_switch_in) {
                                entry["trapped"] = json!(true);
                            }
                        } else if can_switch_in {
                            if r.trapped && side_info.trap.visible.contains(&position) {
                                entry["trapped"] = json!(true);
                            } else if side_info.trap.maybe.contains(&position) {
                                entry["maybeTrapped"] = json!(true);
                            }
                        }
                        // Without a usable move, the only option is Struggle, which also rules out
                        // Mega Evolution.
                        let struggle = locked_into_struggle
                            || (!r.locked_into_move
                                && r.moves.iter().all(|m| m.disabled || m.pp == 0));
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
        // Revival Blessing: Showdown asks the reviving slot to "switch" to a fainted Pokémon.
        Request::Select(select) => {
            let positions: Vec<usize> = select.positions.iter().map(|p| p.position).collect();
            let flags: Vec<bool> = (0..side_info.active_count)
                .map(|i| positions.contains(&i))
                .collect();
            let mut side = side(side_info, player, false);
            if let Some(pokemon) = side["pokemon"].as_array_mut() {
                let mut mons: Vec<&MonBattleData> = player.mons.iter().collect();
                mons.sort_by_key(|m| m.player_effective_team_position);
                for (entry, mon) in pokemon.iter_mut().zip(mons) {
                    if mon.active
                        && mon
                            .player_active_position
                            .is_some_and(|position| positions.contains(&position))
                    {
                        entry["reviving"] = json!(true);
                    }
                }
            }
            let mut value = json!({"forceSwitch": flags, "side": side});
            if !side_info.multiple_requests {
                value["noCancel"] = json!(true);
            }
            value
        }
        other => anyhow::bail!("request {other:?} has no Showdown equivalent"),
    };
    Ok(Some(value))
}
