//! Transplant: builds the state of a battler battle from Showdown's serialized battle
//! (`battle.toJSON()`, `sim/state.ts` `serializeBattle` at the pinned commit) without replaying
//! its history.
//!
//! The battle must be created, not started, with the teams from [`teams`]: each side's Pokémon in
//! Showdown's current order, so that battler's team positions are Showdown's positions. Then
//! [`apply_state`]:
//! 1. starts the battle silently (no entry effects, random draws or logs);
//! 2. patches the turn, active positions, forme, HP, status and its counters, boosts, volatiles and
//!    their effect states (durations, counters, sources, effect orders);
//! 3. patches items, abilities and their states, PP, side and slot conditions, weather, terrain and
//!    pseudo-weather with their remaining turns, Mega formes (stats recalculated) and fainted
//!    members;
//! 4. ends the previous turn the way battler would, which disables and locks moves and makes the
//!    pending request.
//!
//! Only turn boundaries (Showdown's `requestState` is `move`, with an empty queue) can be
//! transplanted. Effect states battler does not know how to describe are an error, never a guess.

use anyhow::{
    Context as _,
    Result,
    anyhow,
    bail,
};
use battler::{
    BoostTable,
    Fraction,
    Id,
    Identifiable,
    PublicCoreBattle,
    Type,
    battle::{
        Context,
        CoreBattle,
        EffectiveAbility,
        Mon,
        MonExitType,
        MonHandle,
        MonSpecialFormeChangeType,
        MoveHandle,
        MoveOutcome,
        RecalculateStatsHpPolicy,
        ReceivedAttackEntry,
    },
    effect::{
        AppliedEffectHandle,
        AppliedEffectLocation,
        EffectHandle,
        LinkedEffectsManager,
        fxlang::{
            EffectState,
            Value,
        },
    },
};
use battler_data::DataStore;
use serde_json::Value as Json;

use crate::{
    names::to_id,
    team::{
        ConvertOptions,
        ConvertedTeam,
        ShowdownSet,
        convert,
    },
};

/// Showdown weather → battler weather condition.
fn weather_id(id: &str) -> &str {
    match id {
        "raindance" => "rainweather",
        "primordialsea" => "heavyrainweather",
        "sunnyday" => "harshsunlight",
        "desolateland" => "extremelyharshsunlight",
        "sandstorm" => "sandstormweather",
        "hail" => "hailweather",
        "snowscape" => "snowweather",
        "deltastream" => "strongwinds",
        other => other,
    }
}

/// A Showdown set (`PokemonSet`) as serialized in the battle state.
pub fn set_from_json(set: &Json) -> Result<ShowdownSet> {
    let string = |key: &str| set[key].as_str().unwrap_or_default().to_owned();
    let stats = |key: &str, default: u16| -> [u16; 6] {
        let mut out = [default; 6];
        for (i, stat) in ["hp", "atk", "def", "spa", "spd", "spe"].iter().enumerate() {
            if let Some(value) = set[key][stat].as_u64() {
                out[i] = value as u16;
            }
        }
        out
    };
    Ok(ShowdownSet {
        name: string("name"),
        species: string("species"),
        item: string("item"),
        ability: string("ability"),
        moves: set["moves"]
            .as_array()
            .context("set has no moves")?
            .iter()
            .filter_map(|m| m.as_str().map(str::to_owned))
            .collect(),
        nature: string("nature"),
        evs: stats("evs", 0),
        gender: string("gender"),
        ivs: stats("ivs", 31),
        shiny: set["shiny"].as_bool().unwrap_or(false),
        level: set["level"].as_u64().map(|l| l as u8),
        happiness: set["happiness"].as_u64().map(|h| h as u8),
        hp_type: string("hpType"),
        pokeball: string("pokeball"),
    })
}

/// Each side's team, in Showdown's current order (`side.pokemon`), from the serialized state.
pub fn teams(
    data: &dyn DataStore,
    state: &Json,
    options: ConvertOptions,
) -> Result<Vec<ConvertedTeam>> {
    state["sides"]
        .as_array()
        .context("state has no sides")?
        .iter()
        .map(|side| {
            let sets = side["pokemon"]
                .as_array()
                .context("side has no pokemon")?
                .iter()
                .map(|p| set_from_json(&p["set"]))
                .collect::<Result<Vec<_>>>()?;
            convert(data, &sets, options)
        })
        .collect()
}

/// `[Kind:id]` → `(Kind, id)`.
fn parse_ref(value: &Json) -> Option<(&str, &str)> {
    let text = value.as_str()?;
    let inner = text.strip_prefix('[')?.strip_suffix(']')?;
    inner.split_once(':')
}

/// The kind and ID of an effect reference, which is a `[Kind:id]` string or a serialized active
/// move (`{"move": "[Move:id]", ...}`).
fn effect_ref(value: &Json) -> Option<(String, String)> {
    if let Some((kind, id)) = parse_ref(value) {
        return Some((kind.to_owned(), id.to_owned()));
    }
    if let Some((kind, id)) = parse_ref(&value["move"]) {
        return Some((kind.to_owned(), id.to_owned()));
    }
    None
}

/// Showdown's slot name (`p2b`) → (side, position).
fn parse_slot(slot: &str) -> Option<(usize, usize)> {
    let mut chars = slot.chars();
    if chars.next()? != 'p' {
        return None;
    }
    let side = chars.next()?.to_digit(10)? as usize;
    let position = chars.next()? as usize;
    if side == 0 || !(('a' as usize)..=('f' as usize)).contains(&position) {
        return None;
    }
    Some((side - 1, position - 'a' as usize))
}

fn uint(value: &Json) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_f64().filter(|f| *f >= 0.0).map(|f| f as u64))
}

fn number_value(n: u64) -> Value {
    Value::UFraction(Fraction::from(n))
}

/// A non-negative number that may have a fractional part (Wish's HP is half the maximum).
fn fraction_value(value: &Json) -> Option<Value> {
    if let Some(n) = value.as_u64() {
        return Some(number_value(n));
    }
    let f = value.as_f64().filter(|f| *f >= 0.0)?;
    for denominator in [2u64, 4, 8, 16, 3, 6] {
        let numerator = f * denominator as f64;
        if numerator.fract() == 0.0 {
            return Some(Value::UFraction(Fraction::new(
                numerator as u64,
                denominator,
            )));
        }
    }
    None
}

/// battler handles of every Showdown Pokémon, by side and position.
struct Refs {
    mons: Vec<Vec<MonHandle>>,
}

impl Refs {
    fn at(&self, side: usize, position: usize) -> Result<MonHandle> {
        self.mons
            .get(side)
            .and_then(|mons| mons.get(position))
            .cloned()
            .ok_or_else(|| anyhow!("no Pokémon at side {side} position {position}"))
    }

    /// A `[Pokemon:p1c]` reference.
    fn mon(&self, value: &Json) -> Result<Option<MonHandle>> {
        match parse_ref(value) {
            Some(("Pokemon", slot)) => {
                let (side, position) =
                    parse_slot(slot).ok_or_else(|| anyhow!("bad Pokémon reference {slot}"))?;
                Ok(Some(self.at(side, position)?))
            }
            _ => Ok(None),
        }
    }
}

/// The battler effect handle for a Showdown effect reference.
fn effect_handle(context: &mut Context, kind: &str, id: &str) -> Result<Option<EffectHandle>> {
    let dex = &context.battle().dex;
    Ok(match kind {
        "Move" => Some(EffectHandle::InactiveMove(
            dex.moves
                .get(id)
                .with_context(|| format!("move {id}"))?
                .id()
                .clone(),
        )),
        "Ability" => Some(EffectHandle::Ability(
            dex.abilities
                .get(id)
                .with_context(|| format!("ability {id}"))?
                .id()
                .clone(),
        )),
        "Item" => Some(EffectHandle::Item(
            dex.items
                .get(id)
                .with_context(|| format!("item {id}"))?
                .id()
                .clone(),
        )),
        "Condition" => Some(
            context
                .battle_mut()
                .get_effect_handle_by_id(&Id::from(weather_id(id)))?
                .clone(),
        ),
        _ => None,
    })
}

/// A new effect state carrying Showdown's generic fields: source, source slot, source effect,
/// duration and effect order.
fn effect_state(
    context: &mut Context,
    refs: &Refs,
    sd: &Json,
    target: Option<MonHandle>,
) -> Result<EffectState> {
    let source = refs.mon(&sd["source"])?;
    let source_effect = match effect_ref(&sd["sourceEffect"]) {
        Some((kind, id)) => effect_handle(context, &kind, &id)?,
        None => None,
    };
    let mut state =
        EffectState::initial_effect_state(context, source_effect.as_ref(), target, source)?;
    // The slot the source was in when the effect started, which can differ from where it is now.
    if let Some((side, position)) = sd["sourceSlot"].as_str().and_then(parse_slot) {
        *state.get_mut("source_side") = Value::Side(side);
        state.set_source_position(position)?;
    }
    if let Some(duration) = uint(&sd["duration"]) {
        state.set_duration(duration as u8);
    }
    if let Some(order) = uint(&sd["effectOrder"]) {
        state.set_effect_order(order as u32);
    }
    state.mark_started();
    Ok(state)
}

fn move_id(context: &Context, sd: &Json) -> Result<Id> {
    let id = sd.as_str().context("expected a move ID")?;
    Ok(context
        .battle()
        .dex
        .moves
        .get(id)
        .with_context(|| format!("move {id}"))?
        .id()
        .clone())
}

/// Copies the fields specific to one volatile. Returns the battler volatiles linked to it (a
/// charging move's own volatile).
fn volatile_values(
    context: &Context,
    id: &str,
    sd: &Json,
    state: &mut EffectState,
) -> Result<Vec<Id>> {
    let mut linked = Vec::new();
    match id {
        "stall" => {
            *state.get_mut("counter") =
                number_value(uint(&sd["counter"]).context("stall without counter")?);
        }
        "choicelock" | "encore" | "disable" => {
            *state.get_mut("move") = Value::String(move_id(context, &sd["move"])?.to_string());
        }
        "twoturnmove" => {
            let mov = move_id(context, &sd["move"])?;
            *state.get_mut("move") = Value::String(mov.to_string());
            linked.push(mov);
        }
        "lockedmove" => {
            *state.get_mut("move") = Value::String(move_id(context, &sd["move"])?.to_string());
            // Showdown keeps the turns left in `trueDuration`; battler counts them in `duration`.
            let turns = uint(&sd["trueDuration"]).context("lockedmove without trueDuration")?;
            state.set_duration(turns as u8);
        }
        "confusion" => {
            *state.get_mut("time") = number_value(uint(&sd["time"]).context("confusion time")?);
        }
        "substitute" => {
            *state.get_mut("hp") = number_value(uint(&sd["hp"]).context("substitute hp")?);
        }
        "partiallytrapped" => {
            *state.get_mut("divisor") =
                number_value(uint(&sd["boundDivisor"]).context("partiallytrapped divisor")?);
        }
        "helpinghand" => {
            *state.get_mut("multiplier") = number_value(uint(&sd["multiplier"]).unwrap_or(1));
        }
        "taunt" | "throatchop" | "healblock" | "noretreat" | "charge" | "glaiverush" | "yawn"
        | "leechseed" | "unburden" | "flashfire" | "flinch" | "roost" | "trapped" => (),
        _ => bail!("unsupported volatile {id}"),
    }
    Ok(linked)
}

fn species_id(context: &Context, value: &Json) -> Result<Id> {
    let (_, id) = parse_ref(value).context("expected a species reference")?;
    let species = &context.battle().dex.species;
    if let Ok(species) = species.get(id) {
        return Ok(species.id().clone());
    }
    // Formes battler names differently (gendered formes, Paldean Tauros breeds, ...).
    for alternate in crate::species::alternates(id).0 {
        if let Ok(species) = species.get(&alternate) {
            return Ok(species.id().clone());
        }
    }
    bail!("species {id} not found")
}

fn ability_id(context: &Context, id: &str) -> Result<Id> {
    Ok(context
        .battle()
        .dex
        .abilities
        .get(id)
        .with_context(|| format!("ability {id}"))?
        .id()
        .clone())
}

fn item_id(context: &Context, id: &str) -> Result<Option<Id>> {
    if id.is_empty() {
        return Ok(None);
    }
    Ok(Some(
        context
            .battle()
            .dex
            .items
            .get(id)
            .with_context(|| format!("item {id}"))?
            .id()
            .clone(),
    ))
}

fn types(values: &Json) -> Result<Vec<Type>> {
    values
        .as_array()
        .context("expected types")?
        .iter()
        .map(|t| {
            let name = t.as_str().context("expected a type")?;
            // A Pokémon that lost its only type (Burn Up).
            let name = if name == "???" { "None" } else { name };
            serde_json::from_value::<Type>(Json::String(name.to_owned()))
                .with_context(|| format!("type {name}"))
        })
        .collect()
}

fn boosts(sd: &Json) -> BoostTable {
    let get = |key: &str| sd[key].as_i64().unwrap_or(0) as i8;
    BoostTable {
        atk: get("atk"),
        def: get("def"),
        spa: get("spa"),
        spd: get("spd"),
        spe: get("spe"),
        acc: get("accuracy"),
        eva: get("evasion"),
    }
}

/// Registers a move as a Mon's last move (a serialized active move), so that effects referring to
/// it (Encore, Disable, choice lock) find it in the move registry.
fn register_last_move(
    context: &mut Context,
    user: MonHandle,
    sd: &Json,
) -> Result<Option<MoveHandle>> {
    let Some((_, id)) = effect_ref(sd) else {
        return Ok(None);
    };
    let id = move_id(context, &Json::String(id))?;
    Ok(Some(CoreBattle::register_active_move_by_id(
        context, &id, user,
    )?))
}

/// Patches one Pokémon: forme, stats, HP, status, item, ability, PP, boosts and per-turn state.
/// Volatiles come later, once every Pokémon is in place.
fn patch_mon(
    context: &mut Context,
    refs: &Refs,
    handle: MonHandle,
    sd: &Json,
    turn: u64,
) -> Result<()> {
    let transformed = sd["transformed"].as_bool().unwrap_or(false);
    let mut context = context.mon_context(handle)?;

    // Forme. A permanent forme change (Mega Evolution, Disguise, Zero to Hero) changes the base
    // species; a temporary one only the species.
    let base_species = species_id(context.as_battle_context(), &sd["baseSpecies"])?;
    let species = species_id(context.as_battle_context(), &sd["species"])?;
    if base_species != context.mon().base_species {
        let base_ability = ability_id(
            context.as_battle_context(),
            sd["baseAbility"].as_str().unwrap_or_default(),
        )?;
        Mon::set_base_species(&mut context, &base_species, &base_ability)?;
        let is_mega = context
            .battle()
            .dex
            .species
            .get_by_id(&base_species)?
            .data
            .mega();
        if is_mega {
            context.mon_mut().special_forme_change_type =
                Some(MonSpecialFormeChangeType::MegaEvolution);
        }
        Mon::recalculate_base_stats(&mut context, RecalculateStatsHpPolicy::DoNotUpdate)?;
    }
    Mon::set_species(
        &mut context,
        &species,
        RecalculateStatsHpPolicy::DoNotUpdate,
    )?;
    if transformed {
        // Transform copied the target's stats (except HP), types and moves (with 5 PP at most),
        // as `transform_into` does.
        context.mon_mut().volatile_state.transformed = true;
        let stored = &sd["storedStats"];
        let stat = |key: &str| -> Result<u16> {
            Ok(uint(&stored[key]).with_context(|| format!("transformed {key}"))? as u16)
        };
        let (atk, def, spa, spd, spe) = (
            stat("atk")?,
            stat("def")?,
            stat("spa")?,
            stat("spd")?,
            stat("spe")?,
        );
        let own_hp = context.mon().base_stored_stats.hp;
        let stats = &mut context.mon_mut().volatile_state.stats;
        stats.hp = own_hp;
        stats.atk = atk;
        stats.def = def;
        stats.spa = spa;
        stats.spd = spd;
        stats.spe = spe;
    }

    // HP.
    let hp = uint(&sd["hp"]).context("hp")? as u16;
    let max_hp = uint(&sd["maxhp"]).context("maxhp")? as u16;
    if context.mon().max_hp != max_hp {
        bail!(
            "{}: max HP {} differs from Showdown's {max_hp}",
            context.mon().name,
            context.mon().max_hp
        );
    }
    context.mon_mut().hp = hp;
    context.mon_mut().base_max_hp = uint(&sd["baseMaxhp"]).unwrap_or(max_hp as u64) as u16;

    // Types (Protean, Soak) differ from the species' only when changed by an effect.
    let sd_types = types(&sd["types"])?;
    if sd_types != context.mon().volatile_state.types {
        context.mon_mut().volatile_state.types = sd_types;
    }
    if let Some(added) = sd["addedType"].as_str().filter(|t| !t.is_empty()) {
        context.mon_mut().volatile_state.added_type =
            Some(serde_json::from_value(Json::String(added.to_owned()))?);
    }

    // Ability and item.
    let base_ability = ability_id(
        context.as_battle_context(),
        sd["baseAbility"].as_str().unwrap_or_default(),
    )?;
    context.mon_mut().base_ability = base_ability;
    let ability = ability_id(
        context.as_battle_context(),
        sd["ability"].as_str().unwrap_or_default(),
    )?;
    context.mon_mut().volatile_state.ability_slot.ability = EffectiveAbility {
        id: ability,
        sub_abilities: Vec::new(),
    };
    let item = item_id(
        context.as_battle_context(),
        sd["item"].as_str().unwrap_or_default(),
    )?;
    context.mon_mut().item = item;
    context.mon_mut().last_item = item_id(
        context.as_battle_context(),
        sd["lastItem"].as_str().unwrap_or_default(),
    )?;
    context.mon_mut().ate_item = sd["ateBerry"].as_bool().unwrap_or(false);
    // Once-per-battle flags kept on the Pokémon across switches.
    for (sd_key, key) in [
        ("syrupTriggered", "syrup_triggered"),
        ("swordBoost", "sword_boost"),
        ("shieldBoost", "shield_boost"),
        ("heroMessageDisplayed", "displayed"),
    ] {
        if let Some(value) = sd[sd_key].as_bool() {
            *context.mon_mut().mon_effect_state.get_mut(key) = Value::Boolean(value);
        }
    }

    // PP of the Pokémon's own moves (Showdown serializes `baseMoveSlots` only when they differ
    // from `moveSlots`, after Transform).
    let base_slots = if sd["baseMoveSlots"].is_array() {
        &sd["baseMoveSlots"]
    } else {
        &sd["moveSlots"]
    };
    for slot in base_slots.as_array().context("moveSlots")? {
        let id = move_id(context.as_battle_context(), &slot["id"])?;
        let pp = uint(&slot["pp"]).context("pp")? as u8;
        let max_pp = uint(&slot["maxpp"]).context("maxpp")? as u8;
        // Used since the Pokémon switched in (Last Resort).
        let used = slot["used"].as_bool().unwrap_or(false);
        let mon = context.mon_mut();
        let mut found = false;
        let volatile_slots: &mut [battler::battle::MoveSlot] = if transformed {
            &mut []
        } else {
            &mut mon.volatile_state.move_slots
        };
        for slot in mon
            .base_move_slots
            .iter_mut()
            .chain(volatile_slots.iter_mut())
            .filter(|s| s.id == id)
        {
            if slot.max_pp != max_pp {
                bail!(
                    "{}: max PP of {id} is {} in battler, {max_pp} in Showdown",
                    mon.name,
                    slot.max_pp
                );
            }
            slot.pp = pp;
            slot.used = used;
            found = true;
        }
        if !found {
            bail!("{}: no move slot for {id}", context.mon().name);
        }
    }
    if transformed {
        let mut slots = Vec::new();
        for slot in sd["moveSlots"].as_array().context("moveSlots")? {
            let id = move_id(context.as_battle_context(), &slot["id"])?;
            let mov = context.battle().dex.moves.get_by_id(&id)?;
            let mut copied = battler::battle::MoveSlot::new_simulated(
                id.clone(),
                mov.data.name.clone(),
                uint(&slot["pp"]).context("pp")? as u8,
                uint(&slot["maxpp"]).context("maxpp")? as u8,
                mov.data.target,
                mov.data.primary_type,
            );
            copied.used = slot["used"].as_bool().unwrap_or(false);
            slots.push(copied);
        }
        context.mon_mut().volatile_state.move_slots = slots;
    }

    context.mon_mut().volatile_state.boosts = boosts(&sd["boosts"]);

    // Counters. Per-turn values describe the end of the previous turn, which the transplant ends.
    let active = sd["isActive"].as_bool().unwrap_or(false);
    let active_turns = uint(&sd["activeTurns"]).unwrap_or(0) as u32;
    context.mon_mut().active_turns = if active {
        active_turns.saturating_sub(1)
    } else {
        active_turns
    };
    context.mon_mut().active_move_actions = uint(&sd["activeMoveActions"]).unwrap_or(0) as u32;
    let times_attacked = uint(&sd["timesAttacked"]).unwrap_or(0);
    context.mon_mut().times_attacked = times_attacked;
    context.mon_mut().volatile_state.times_attacked = times_attacked;
    context.mon_mut().volatile_state.move_this_turn_outcome = match sd.get("moveLastTurnResult") {
        Some(Json::Bool(true)) => Some(MoveOutcome::Succeeded),
        Some(Json::Bool(false)) => Some(MoveOutcome::Failed),
        _ => None,
    };
    if let Some(location) = sd["lastMoveTargetLoc"].as_i64() {
        context.mon_mut().volatile_state.last_move_target_location = Some(location as isize);
    }
    let mut received = Vec::new();
    for attack in sd["attackedBy"].as_array().into_iter().flatten() {
        let Some(source) = refs.mon(&attack["source"])? else {
            continue;
        };
        let (source_side, source_position) = attack["slot"]
            .as_str()
            .and_then(parse_slot)
            .context("attackedBy without slot")?;
        received.push(ReceivedAttackEntry {
            source,
            source_side,
            source_position,
            damage: uint(&attack["damage"]).unwrap_or(0) as u16,
            turn: if attack["thisTurn"].as_bool().unwrap_or(false) {
                turn
            } else {
                turn - 1
            },
        });
    }
    context.mon_mut().volatile_state.received_attacks = received;

    // Status.
    let status = sd["status"].as_str().unwrap_or_default();
    if sd["fainted"].as_bool().unwrap_or(false) || status == "fnt" {
        context.mon_mut().hp = 0;
        context.mon_mut().exited = Some(MonExitType::Fainted);
        context.mon_mut().status = Some(Id::from_known("fnt"));
    } else if !status.is_empty() {
        let mut state = effect_state(
            context.as_battle_context_mut(),
            refs,
            &sd["statusState"],
            Some(handle),
        )?;
        let ss = &sd["statusState"];
        match status {
            "tox" => *state.get_mut("stage") = number_value(uint(&ss["stage"]).unwrap_or(0)),
            "slp" => {
                *state.get_mut("time") = number_value(uint(&ss["time"]).context("slp time")?);
                *state.get_mut("total_time") =
                    number_value(uint(&ss["startTime"]).context("slp startTime")?);
            }
            "frz" => {
                if let Some(time) = uint(&ss["time"]) {
                    *state.get_mut("time") = number_value(time);
                }
            }
            "par" | "brn" | "psn" => (),
            other => bail!("unsupported status {other}"),
        }
        context.mon_mut().status = Some(Id::from(status));
        context.mon_mut().status_state = state;
    }

    // Last moves, for Encore, Disable, Instruct and moves that cannot be used twice in a row.
    if let Some(last_move) =
        register_last_move(context.as_battle_context_mut(), handle, &sd["lastMove"])?
    {
        context.mon_mut().volatile_state.last_move = Some(last_move);
        let id = context
            .as_battle_context()
            .active_move(last_move)?
            .id()
            .clone();
        context.mon_mut().volatile_state.last_move_selected = Some(id);
    }
    if let Some(last_move_used) =
        register_last_move(context.as_battle_context_mut(), handle, &sd["lastMoveUsed"])?
    {
        context.mon_mut().volatile_state.last_move_used = Some(last_move_used);
    }
    Ok(())
}

/// Ability, item and volatile states of a Pokémon in battle.
fn patch_active(context: &mut Context, refs: &Refs, handle: MonHandle, sd: &Json) -> Result<()> {
    let mut context = context.mon_context(handle)?;

    // The ability and item started when the Pokémon switched in.
    let ability_order = uint(&sd["abilityState"]["effectOrder"]).unwrap_or(0) as u32;
    {
        let state = &mut context.mon_mut().volatile_state.ability_slot.effect_state;
        state.set_effect_order(ability_order);
        state.mark_started();
    }
    let ability_state = &sd["abilityState"];
    for (sd_key, key) in [
        ("unnerved", "unnerved"),
        ("protean", "activated"),
        ("libero", "activated"),
        ("berryWeaken", "berry_weaken"),
    ] {
        if let Some(value) = ability_state[sd_key].as_bool() {
            *context
                .mon_mut()
                .volatile_state
                .ability_slot
                .effect_state
                .get_mut(key) = Value::Boolean(value);
        }
    }
    if context.mon().item.is_some() {
        let item_order = uint(&sd["itemState"]["effectOrder"]).unwrap_or(0) as u32;
        let state = &mut context.mon_mut().volatile_state.item_state;
        state.set_effect_order(item_order);
        state.mark_started();
    }

    let volatiles = sd["volatiles"].as_object().context("volatiles")?;
    // The move a `twoturnmove` volatile is charging has a volatile of its own (Solar Beam).
    let charging = volatiles
        .get("twoturnmove")
        .and_then(|v| v["move"].as_str())
        .map(to_id);
    let mut links = Vec::new();
    for (id, vs) in volatiles {
        // Showdown's bookkeeping on the trapper for Block or Mean Look; battler keeps the trapper
        // as the source of the target's `trapped` volatile.
        if id == "trapper" {
            continue;
        }
        if charging.as_deref() == Some(id.as_str()) {
            let state = effect_state(context.as_battle_context_mut(), refs, vs, Some(handle))?;
            if let Some(location) = vs["targetLoc"].as_i64() {
                context.mon_mut().volatile_state.last_move_target_location =
                    Some(location as isize);
            }
            let battler_id = move_id(context.as_battle_context(), &Json::String(id.clone()))?;
            context
                .mon_mut()
                .volatile_state
                .volatiles
                .insert(battler_id, state);
            continue;
        }
        let mut state = effect_state(context.as_battle_context_mut(), refs, vs, Some(handle))?;
        let linked = volatile_values(context.as_battle_context(), id, vs, &mut state)?;
        // Abilities that keep their own flag beside the volatile.
        if matches!(id.as_str(), "unburden" | "flashfire") {
            *context
                .mon_mut()
                .volatile_state
                .ability_slot
                .effect_state
                .get_mut("activated") = Value::Boolean(true);
        }
        if id == "trapped" {
            // Block, Mean Look: the trapper is the Pokémon the volatile is linked to.
            if let Some(trapper) = vs["linkedPokemon"].as_array().and_then(|l| l.first()) {
                if let Some(trapper) = refs.mon(trapper)? {
                    state.set_source(trapper);
                }
            }
        }
        context
            .mon_mut()
            .volatile_state
            .volatiles
            .insert(Id::from(id.as_str()), state);
        for linked in linked {
            links.push((Id::from(id.as_str()), linked));
        }
    }
    // A charging move's own volatile (Solar Beam), linked to `twoturnmove`. Showdown serializes it
    // as a volatile of its own, which was copied above.
    for (from, to) in links {
        if !context.mon().volatile_state.volatiles.contains_key(&to) {
            bail!("{from} without its linked volatile {to}");
        }
        let from_handle = context.battle_mut().get_effect_handle_by_id(&from)?.clone();
        let to_handle = context.battle_mut().get_effect_handle_by_id(&to)?.clone();
        LinkedEffectsManager::link(
            context.as_battle_context_mut(),
            &AppliedEffectHandle::new(from_handle, AppliedEffectLocation::MonVolatile(handle)),
            &AppliedEffectHandle::new(to_handle, AppliedEffectLocation::MonVolatile(handle)),
        )?;
    }
    Ok(())
}

fn patch_conditions(
    context: &mut Context,
    refs: &Refs,
    conditions: &Json,
    kind: &str,
    turn: u64,
) -> Result<Vec<(Id, EffectState)>> {
    let mut out = Vec::new();
    for (id, sd) in conditions.as_object().into_iter().flatten() {
        let mut state = effect_state(context, refs, sd, None)?;
        // battler's source effect for a move's side, slot or field condition is the move, which
        // Showdown does not record (Wish names it when it heals).
        if state.source_effect().is_none()
            && let Ok(mov) = context.battle().dex.moves.get(id)
        {
            state.set_source_effect(EffectHandle::InactiveMove(mov.id().clone()));
        }
        match (kind, id.as_str()) {
            ("side", "toxicspikes" | "spikes") => {
                *state.get_mut("layers") = number_value(uint(&sd["layers"]).unwrap_or(1));
            }
            (
                "side",
                "tailwind" | "stealthrock" | "stickyweb" | "auroraveil" | "lightscreen" | "reflect"
                | "safeguard" | "mist" | "wideguard" | "quickguard",
            ) => (),
            ("slot", "wish") => {
                *state.get_mut("hp") = fraction_value(&sd["hp"]).context("wish hp")?;
                // Showdown heals at the end of the turn after the one Wish started on (its
                // `startingTurn` is that turn minus one); battler counts a duration of 2 down at
                // each end of turn.
                let starting = uint(&sd["startingTurn"]).context("wish startingTurn")?;
                state.set_duration((starting + 3).saturating_sub(turn) as u8);
            }
            ("pseudo", "trickroom" | "gravity" | "magicroom" | "wonderroom" | "fairylock") => (),
            _ => bail!("unsupported {kind} condition {id}"),
        }
        out.push((Id::from(id.as_str()), state));
    }
    Ok(out)
}

/// Builds the battle's state from Showdown's serialized battle. See the module documentation.
pub fn apply_state(battle: &mut PublicCoreBattle, state: &Json) -> Result<()> {
    let request_state = state["requestState"].as_str().unwrap_or_default();
    if request_state != "move" {
        bail!("only turn boundaries can be transplanted (requestState is '{request_state}')");
    }
    if state["queue"].as_array().is_some_and(|q| !q.is_empty()) {
        bail!("only turn boundaries can be transplanted (the queue is not empty)");
    }
    let turn = uint(&state["turn"]).context("turn")?;
    let sides = state["sides"].as_array().context("sides")?;

    battle.start_transplant()?;
    let core = &mut battle.internal;
    let players = (0..sides.len())
        .map(|side| {
            core.players
                .iter()
                .position(|p| p.side == side)
                .ok_or_else(|| anyhow!("no player on side {side}"))
        })
        .collect::<Result<Vec<_>>>()?;
    let refs = Refs {
        mons: players
            .iter()
            .map(|&p| core.players[p].mon_handles().cloned().collect())
            .collect(),
    };
    for (side, sd) in sides.iter().enumerate() {
        let count = sd["pokemon"].as_array().map(|p| p.len()).unwrap_or(0);
        if refs.mons[side].len() != count {
            bail!(
                "side {side} has {} Pokémon in battler, {count} in Showdown",
                refs.mons[side].len()
            );
        }
    }

    let mut context = core.context();
    for (side, sd) in sides.iter().enumerate() {
        let pokemon = sd["pokemon"].as_array().context("pokemon")?;
        for (position, p) in pokemon.iter().enumerate() {
            let handle = refs.at(side, position)?;
            patch_mon(&mut context, &refs, handle, p, turn)
                .with_context(|| format!("p{}: {}", side + 1, p["set"]["name"]))?;
        }
        // Illusion: the Pokémon whose appearance it has.
        for (position, p) in pokemon.iter().enumerate() {
            if let Some(disguise) = refs.mon(&p["illusion"])? {
                let details = Mon::physical_details(&context.mon_context(disguise)?)?;
                context
                    .mon_mut(refs.at(side, position)?)?
                    .volatile_state
                    .illusion = Some(details);
            }
        }
        // Active positions: Showdown keeps its active Pokémon first.
        let active = sd["active"].as_array().context("active")?;
        for (position, reference) in active.iter().enumerate() {
            let Some(handle) = refs.mon(reference)? else {
                continue;
            };
            let fainted = pokemon[position]["fainted"].as_bool().unwrap_or(false);
            let mut context = context.mon_context(handle)?;
            CoreBattle::place_mon_for_transplant(&mut context, position, fainted)?;
        }
        for (position, p) in pokemon.iter().enumerate() {
            let handle = refs.at(side, position)?;
            context.mon_mut(handle)?.effective_team_position = position;
            if p["isActive"].as_bool().unwrap_or(false) && !p["fainted"].as_bool().unwrap_or(false)
            {
                patch_active(&mut context, &refs, handle, p)
                    .with_context(|| format!("p{}: {}", side + 1, p["set"]["name"]))?;
            }
        }

        // Mega Evolution is once per side.
        let mut mega_used = false;
        for handle in &refs.mons[side] {
            mega_used |= context.mon(*handle)?.special_forme_change_type
                == Some(MonSpecialFormeChangeType::MegaEvolution);
        }
        let player = players[side];
        context.battle_mut().player_mut(player)?.can_mega_evolve &= !mega_used;
        // Per-turn values describe the end of the previous turn.
        context.battle_mut().player_mut(player)?.fainted_this_turn =
            !sd["faintedLastTurn"].is_null();
        context.battle_mut().side_mut(side)?.total_fainted = uint(&sd["totalFainted"]).unwrap_or(0);

        let conditions =
            patch_conditions(&mut context, &refs, &sd["sideConditions"], "side", turn)?;
        context.battle_mut().side_mut(side)?.conditions = conditions.into_iter().collect();
        context.battle_mut().side_mut(side)?.slot_conditions.clear();
        for (slot, sc) in sd["slotConditions"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            let conditions = patch_conditions(&mut context, &refs, sc, "slot", turn)?;
            if !conditions.is_empty() {
                context
                    .battle_mut()
                    .side_mut(side)?
                    .slot_conditions
                    .insert(slot, conditions.into_iter().collect());
            }
        }
    }

    // Field.
    let field = &state["field"];
    let weather = field["weather"].as_str().unwrap_or_default();
    if !weather.is_empty() {
        let state = effect_state(&mut context, &refs, &field["weatherState"], None)?;
        let field = &mut context.battle_mut().field;
        field.weather = Some(Id::from(weather_id(weather)));
        field.weather_state = state;
    }
    let terrain = field["terrain"].as_str().unwrap_or_default();
    if !terrain.is_empty() {
        let state = effect_state(&mut context, &refs, &field["terrainState"], None)?;
        let field = &mut context.battle_mut().field;
        field.terrain = Some(Id::from(terrain));
        field.terrain_state = state;
    }
    let pseudo = patch_conditions(&mut context, &refs, &field["pseudoWeather"], "pseudo", turn)?;
    context.battle_mut().field.pseudo_weathers = pseudo.into_iter().collect();

    // The battle's last move (Copycat).
    if let Some((_, id)) = effect_ref(&state["lastMove"]) {
        let user = refs.at(0, 0)?;
        let id = move_id(&context, &Json::String(id))?;
        let handle = CoreBattle::register_active_move_by_id(&mut context, &id, user)?;
        context.battle_mut().set_last_move(Some(handle));
    }

    if let Some(order) = uint(&state["effectOrder"]) {
        let next = (order as u32).max(context.battle().peek_next_effect_order());
        context.battle_mut().set_next_effect_order(next);
    }
    CoreBattle::invalidate_effect_caches(&mut context)?;
    for handle in context
        .battle()
        .all_active_mon_handles()
        .collect::<Vec<_>>()
    {
        Mon::update_speed(&mut context.mon_context(handle)?)?;
    }
    drop(context);

    battle.finish_transplant(turn)?;
    Ok(())
}
