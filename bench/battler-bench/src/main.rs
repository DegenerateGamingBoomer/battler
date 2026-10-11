//! Battler-side P0 benchmarks: (a) new battle + start(), (b) two-turn rollouts, (c) battler-calc
//! probes.
//!
//! Each Showdown position is converted into a *fresh* battler battle from the same teams (the four
//! Pokémon each side brought to the position, in pick order). This is a stand-in for restoring the
//! position, which battler cannot do yet (no state loading until P2).
//!
//! Usage: battler-bench [positions_dir] [reps] [out.json] [calc-probes.json]
//! Requires DATA_DIR (battle data directory) in the environment.

use std::{
    fs,
    path::PathBuf,
    time::Instant,
};

use anyhow::{
    Context,
    Result,
};
use battler::{
    BattleType,
    PublicCoreBattle,
    TeamData,
};
use battler_test_utils::{
    static_local_data_store,
    TestBattleBuilder,
};
use serde_json::{
    json,
    Value,
};

struct Position {
    id: u64,
    seed: u64,
    teams: [TeamData; 2],
}

/// Reverse of the stat-point mapping in the spec: EV = max(8*SP - 4, 0).
fn sp_to_ev(sp: u64) -> u64 {
    (8 * sp).saturating_sub(4)
}

fn stat_table(v: &Value, f: impl Fn(u64) -> u64) -> Value {
    let mut out = serde_json::Map::new();
    for key in ["hp", "atk", "def", "spa", "spd", "spe"] {
        out.insert(key.to_owned(), json!(f(v[key].as_u64().unwrap_or(0))));
    }
    Value::Object(out)
}

/// Maps a Showdown species name to battler's naming (P0 stand-in; P1b will own this properly).
fn map_species(name: &str) -> String {
    if let Some(base) = name.strip_suffix("-F") {
        return base.to_owned();
    }
    match name {
        "Maushold-Four" => "Maushold".to_owned(),
        n if n.starts_with("Tauros-Paldea-") => format!("{n}-Breed"),
        n if n.starts_with("Squawkabilly-") && !n.ends_with("-Plumage") => format!("{n}-Plumage"),
        n => n.to_owned(),
    }
}

fn team_from_showdown(sets: &Value, picks: &[usize]) -> Result<(TeamData, Vec<String>)> {
    let sets = sets.as_array().context("team is not an array")?;
    let mut members = Vec::new();
    let mut names = Vec::new();
    for &pick in picks {
        let set = &sets[pick - 1];
        names.push(set["species"].as_str().unwrap_or("").to_owned());
        members.push(json!({
            "name": set["name"],
            "species": map_species(set["species"].as_str().unwrap_or("")),
            "ability": set["ability"],
            "moves": set["moves"],
            "item": set["item"].as_str().filter(|i| !i.is_empty()),
            "nature": "Hardy",
            "gender": if set["species"].as_str().unwrap_or("").ends_with("-F") { "F" } else { match set["gender"].as_str() { Some("F") => "F", Some("M") => "M", _ => "U" } },
            "level": 50,
            "evs": stat_table(&set["evs"], sp_to_ev),
            "ivs": stat_table(&set["ivs"], |v| v),
            "shiny": set["shiny"].as_bool().unwrap_or(false),
        }));
    }
    let team: TeamData = serde_json::from_value(json!({ "members": members }))?;
    Ok((team, names))
}

fn picks_from_log(log: &Value, side: &str) -> Vec<usize> {
    let prefix = format!(">{side} team ");
    log.as_array()
        .into_iter()
        .flatten()
        .filter_map(|l| l.as_str())
        .find_map(|l| l.strip_prefix(&prefix))
        .map(|rest| {
            rest.chars()
                .filter_map(|c| c.to_digit(10).map(|d| d as usize))
                .collect()
        })
        .unwrap_or_default()
}

fn load_positions(dir: &PathBuf) -> Result<Vec<Position>> {
    let mut files: Vec<_> = fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    let mut out = Vec::new();
    for path in files {
        let v: Value = serde_json::from_str(&fs::read_to_string(&path)?)?;
        let (t1, _) = team_from_showdown(&v["teams"][0], &picks_from_log(&v["inputLog"], "p1"))?;
        let (t2, _) = team_from_showdown(&v["teams"][1], &picks_from_log(&v["inputLog"], "p2"))?;
        out.push(Position {
            id: v["id"].as_u64().unwrap_or(0),
            seed: v["battleSeed"][1].as_u64().unwrap_or(0) + 1,
            teams: [t1, t2],
        });
    }
    Ok(out)
}

fn new_battle(p: &Position) -> Result<PublicCoreBattle<'static>> {
    TestBattleBuilder::new()
        .with_battle_type(BattleType::Doubles)
        .with_seed(p.seed)
        .with_team_validation(false)
        .add_player_to_side_1("p1", "p1")
        .add_player_to_side_2("p2", "p2")
        .with_team("p1", p.teams[0].clone())
        .with_team("p2", p.teams[1].clone())
        .build(static_local_data_store())
}

/// Makes random choices for every player with an open request. Returns the number of choices made.
fn choose_all(battle: &mut PublicCoreBattle<'static>) -> Result<usize> {
    let players: Vec<String> = battle.active_requests().map(|(id, _)| id).collect();
    let n = players.len();
    for id in players {
        battle.set_player_choice(&id, "randomall")?;
    }
    Ok(n)
}

fn stats(v: &mut Vec<f64>) -> Value {
    if v.is_empty() {
        return Value::Null;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q = |q: f64| v[(((q * v.len() as f64).ceil() as usize).max(1) - 1).min(v.len() - 1)];
    json!({"n": v.len(), "median": q(0.5), "p90": q(0.9), "min": v[0], "max": v[v.len() - 1]})
}

fn status_name(s: &str) -> Option<&'static str> {
    Some(match s {
        "brn" => "Burn",
        "par" => "Paralysis",
        "slp" => "Sleep",
        "frz" => "Freeze",
        "psn" => "Poison",
        "tox" => "Bad Poison",
        _ => return None,
    })
}

fn calc_mon(v: &Value) -> Result<battler_calc::state::Mon> {
    use battler_data::{
        BoostTable,
        Fraction,
        Nature,
        StatTable,
    };
    let boosts = &v["boosts"];
    let b = |k: &str| boosts[k].as_i64().unwrap_or(0) as i8;
    Ok(battler_calc::state::Mon {
        name: map_species(&v["name"].as_str().unwrap_or("").replace('\u{2019}', "'")),
        level: v["level"].as_u64().unwrap_or(50),
        health: Some(Fraction::new(
            v["hp"].as_u64().unwrap_or(1),
            v["maxhp"].as_u64().unwrap_or(1).max(1),
        )),
        ability: v["ability"].as_str().map(str::to_owned),
        item: v["item"].as_str().map(str::to_owned),
        nature: Some(serde_json::from_value::<Nature>(v["nature"].clone())?),
        ivs: Some(serde_json::from_value::<StatTable>(v["ivs"].clone())?),
        evs: Some(serde_json::from_value::<StatTable>(v["evs"].clone())?),
        boosts: BoostTable {
            atk: b("atk"),
            def: b("def"),
            spa: b("spa"),
            spd: b("spd"),
            spe: b("spe"),
            acc: b("accuracy"),
            eva: b("evasion"),
        },
        status: v["status"]
            .as_str()
            .and_then(status_name)
            .map(str::to_owned),
        ..Default::default()
    })
}

fn calc_side(conds: &Value) -> battler_calc::state::Side {
    battler_calc::state::Side {
        conditions: conds
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| c.as_str().map(str::to_owned))
            .collect(),
    }
}

/// Times battler-calc on the damage probes exported from the Showdown positions.
fn bench_calc(path: &str, reps: usize) -> Result<Value> {
    use battler_calc::{
        simulate::{
            simulate_move,
            MoveSimulatorInput,
            MoveSimulatorInputFlags,
        },
        state::{
            Field,
            Move,
        },
    };
    let probes: Vec<Value> = serde_json::from_str(&fs::read_to_string(path)?)?;
    let build = |p: &Value| -> Result<MoveSimulatorInput<'static>> {
        Ok(MoveSimulatorInput {
            data: static_local_data_store(),
            field: Field {
                battle_type: "Doubles".to_owned(),
                weather: p["field"]["weather"].as_str().map(str::to_owned),
                terrain: p["field"]["terrain"].as_str().map(str::to_owned),
                attacker_side: calc_side(&p["attackerSide"]),
                defender_side: calc_side(&p["defenderSide"]),
                ..Default::default()
            },
            attacker: calc_mon(&p["attacker"])?,
            defender: calc_mon(&p["defender"])?,
            mov: Move {
                name: p["move"].as_str().unwrap_or("").to_owned(),
                spread: p["spread"].as_bool().unwrap_or(false),
                ..Default::default()
            },
            flags: MoveSimulatorInputFlags::default(),
        })
    };

    // Warm-up and correctness-in-range check (single pass).
    let mut errors: std::collections::BTreeMap<String, usize> = Default::default();
    let mut ok = 0usize;
    let mut in_range = 0usize;
    let mut comparable = 0usize;
    let mut usable: Vec<&Value> = Vec::new();
    for p in &probes {
        match simulate_move(build(p)?) {
            Ok(out) => {
                ok += 1;
                usable.push(p);
                if let Some(sd) = p["sdDamage"].as_u64() {
                    let range = out.hits.iter().fold(None, |acc: Option<(u64, u64)>, h| {
                        let r = h.damage.damage.value().min_max_range();
                        match (acc, r) {
                            (Some((a, b)), Some(r)) => Some((a + r.a(), b + r.b())),
                            (None, Some(r)) => Some((r.a(), r.b())),
                            (acc, None) => acc,
                        }
                    });
                    if let Some((lo, hi)) = range {
                        comparable += 1;
                        if sd >= lo && sd <= hi {
                            in_range += 1;
                        }
                    }
                }
            }
            Err(err) => {
                *errors.entry(format!("{err:#}")).or_default() += 1;
            }
        }
    }

    let mut times = Vec::new();
    for _ in 0..reps {
        for p in &usable {
            let input = build(p)?;
            let t = Instant::now();
            let _ = simulate_move(input);
            times.push(t.elapsed().as_secs_f64() * 1e6);
        }
    }
    let mut top_errors: Vec<_> = errors.into_iter().collect();
    top_errors.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    top_errors.truncate(10);
    Ok(json!({
        "unit": "us",
        "note": "battler_calc::simulate::simulate_move on the same (attacker, target, move) triples as Showdown getDamage; input construction excluded; includes by-name data lookups",
        "probes": probes.len(),
        "probes_ok": ok,
        "probes_error": probes.len() - ok,
        "top_errors": top_errors.iter().map(|(e, n)| json!({"error": e, "count": n})).collect::<Vec<_>>(),
        "showdown_roll_within_calc_range": {"comparable": comparable, "in_range": in_range, "note": "informational only: one random Showdown roll vs calc [min,max], summed over hits; stat/ruleset differences not controlled"},
        "per_call": stats(&mut times),
    }))
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let dir = PathBuf::from(args.get(1).map(String::as_str).unwrap_or("bench/positions"));
    let reps: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(7);
    let out = args
        .get(3)
        .map(String::as_str)
        .unwrap_or("docs/benchmarks/p0-battler.json");

    let all_positions = load_positions(&dir)?;

    // Cold start: data store construction, first battle, and its start().
    let t = Instant::now();
    let _ = static_local_data_store();
    let data_store_us = t.elapsed().as_secs_f64() * 1e6;
    let t = Instant::now();
    let mut first = new_battle(&all_positions[0])?;
    let first_new_us = t.elapsed().as_secs_f64() * 1e6;
    let t = Instant::now();
    first.start()?;
    let first_start_us = t.elapsed().as_secs_f64() * 1e6;
    drop(first);

    // Second battle (different position) to expose the fxlang parse cost of new effects.
    let t = Instant::now();
    let mut second = new_battle(&all_positions[1])?;
    second.start()?;
    let second_total_us = t.elapsed().as_secs_f64() * 1e6;
    drop(second);

    // Showdown names that battler's data does not resolve (or other build failures) exclude a
    // position from the battler-side benchmark; every exclusion is recorded.
    let mut unsupported: Vec<Value> = Vec::new();
    let mut positions = Vec::new();
    for p in all_positions {
        match new_battle(&p).and_then(|mut b| b.start()) {
            Ok(()) => positions.push(p),
            Err(err) => unsupported.push(json!({"position": p.id, "error": format!("{err:#}")})),
        }
    }
    anyhow::ensure!(positions.len() >= 2, "too few usable positions");

    // Warm-up pass over all positions.
    for p in &positions {
        let mut b = new_battle(p)?;
        b.start()?;
    }

    let mut new_us = Vec::new();
    let mut start_us = Vec::new();
    let mut new_start_us = Vec::new();
    let mut rollout_us = Vec::new();
    let mut per_turn = Vec::new();
    let mut per_choice = Vec::new();
    let mut ended = 0usize;
    let mut turns_played = 0u64;
    let mut errors = 0usize;

    for _ in 0..reps {
        for p in &positions {
            let t0 = Instant::now();
            let mut b = new_battle(p)?;
            let t1 = Instant::now();
            b.start()?;
            let t2 = Instant::now();
            new_us.push((t1 - t0).as_secs_f64() * 1e6);
            start_us.push((t2 - t1).as_secs_f64() * 1e6);
            new_start_us.push((t2 - t0).as_secs_f64() * 1e6);

            // Two turns of random legal choices from the fresh battle.
            let start_turn = b.turn();
            let mut choices = 0usize;
            let mut guard = 0;
            let mut failed = false;
            while !b.ended() && b.turn() < start_turn + 2 && guard < 50 {
                guard += 1;
                match choose_all(&mut b) {
                    Ok(0) => break,
                    Ok(n) => choices += n,
                    Err(_) => {
                        failed = true;
                        break;
                    }
                }
            }
            let t3 = Instant::now();
            if failed {
                errors += 1;
                continue;
            }
            let played = b.turn() - start_turn;
            if b.ended() {
                ended += 1;
            }
            turns_played += played;
            let adv = (t3 - t2).as_secs_f64() * 1e6;
            rollout_us.push((t3 - t0).as_secs_f64() * 1e6);
            if played > 0 {
                per_turn.push(adv / played as f64);
            }
            if choices > 0 {
                per_choice.push(adv / choices as f64);
            }
        }
    }

    let calc = bench_calc(
        args.get(4)
            .map(String::as_str)
            .unwrap_or("bench/calc-probes.json"),
        reps,
    )?;

    let result = json!({
        "engine": "battler",
        "damage": calc,
        "positions_total": positions.len() + unsupported.len(),
        "positions": positions.len(),
        "positions_unsupported": unsupported,
        "reps": reps,
        "restore_standin": {
            "unit": "us",
            "note": "new battle from the same teams (4 picked mons per side, leads = first two picks) + start(); NOT a restore of the Showdown position",
            "first_in_process": {
                "static_data_store_us": data_store_us,
                "new_battle_us": first_new_us,
                "start_us": first_start_us,
                "second_battle_new_plus_start_us": second_total_us,
            },
            "new_battle": stats(&mut new_us),
            "start": stats(&mut start_us),
            "new_plus_start": stats(&mut new_start_us),
        },
        "rollout2": {
            "unit": "us",
            "note": "new battle + start() + 2 turns of engine-side `randomall` choices from turn 1 (not the Showdown position). Choice generation is inside the engine and is not separable.",
            "new_start_plus_two_turns": stats(&mut rollout_us),
            "advance_per_turn_incl_choice_gen": stats(&mut per_turn),
            "advance_per_choice_incl_choice_gen": stats(&mut per_choice),
            "rollouts_ended_early": ended,
            "turns_played": turns_played,
            "rollouts_with_errors_skipped": errors,
        },
    });
    fs::create_dir_all(std::path::Path::new(out).parent().unwrap())?;
    fs::write(out, serde_json::to_string_pretty(&result)?)?;
    println!("wrote {out}");
    Ok(())
}
