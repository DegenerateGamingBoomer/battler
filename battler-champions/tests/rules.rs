//! Engine rules (stat points, PP, public health) checked against values exported from Pokémon
//! Showdown's `champions` mod (`scripts/export-champions-rule-cases.js`).

mod common;

use battler::{
    BattleType,
    battle::champions,
};
use common::*;
use serde_json::{
    Value,
    json,
};

fn cases() -> Value {
    serde_json::from_str(include_str!("data/rule-cases.json")).unwrap()
}

fn one_mon_battle(rules: Rules, member: Value) -> battler::PublicCoreBattle<'static> {
    let mut b = battle(
        rules,
        BattleType::Singles,
        1,
        team(vec![member]),
        team(vec![mon("Pikachu", "No Ability", None, &["Splash"])]),
    );
    b.start().unwrap();
    b
}

#[test]
fn stat_points_match_showdown() {
    let cases = cases();
    let (mut checked, mut skipped) = (0, Vec::new());
    for case in cases["stats"].as_array().unwrap() {
        let species = case["species"].as_str().unwrap();
        let mut member = mon(species, "No Ability", None, &["Splash"]);
        member["nature"] = case["nature"].clone();
        member["evs"] = case["evs"].clone();
        // IVs must not matter, so use the engine default (zero).
        let data: &dyn battler::DataStore = &*CHAMPIONS;
        let known = battler::Id::from(species);
        if data.get_species(&known).unwrap().is_none() {
            skipped.push(species.to_owned());
            continue;
        }
        let mut b = one_mon_battle(Rules::Champions, member);
        let mon_data = &b.player_data("p1").unwrap().mons[0];
        let expected = &case["expected"];
        let stat = |s: battler::Stat| mon_data.stats.get(&s).copied().unwrap_or_default();
        let got = json!({
            "hp": mon_data.max_hp, "atk": stat(battler::Stat::Atk), "def": stat(battler::Stat::Def),
            "spa": stat(battler::Stat::SpAtk), "spd": stat(battler::Stat::SpDef),
            "spe": stat(battler::Stat::Spe),
        });
        assert_eq!(&got, expected, "{case}");
        checked += 1;
    }
    assert!(
        checked >= 150,
        "only {checked} cases checked; skipped species: {skipped:?}"
    );
}

#[test]
fn max_pp_matches_showdown() {
    let cases = cases();
    let pp = cases["pp"].as_object().unwrap();
    let data: &dyn battler::DataStore = &*CHAMPIONS;
    let mut ids: Vec<&String> = pp
        .keys()
        .filter(|id| {
            data.get_move(&battler::Id::from(id.as_str()))
                .unwrap()
                .is_some()
        })
        .collect();
    ids.sort();
    let mut mismatches = Vec::new();
    for chunk in ids.chunks(4) {
        let moves: Vec<&str> = chunk
            .iter()
            .map(|id| pp[*id]["name"].as_str().unwrap())
            .collect();
        let mut b = one_mon_battle(Rules::Champions, mon("Pikachu", "No Ability", None, &moves));
        let slots = &b.player_data("p1").unwrap().mons[0].moves;
        for (id, slot) in chunk.iter().zip(slots) {
            let expected = pp[*id]["max"].as_u64().unwrap();
            if slot.max_pp as u64 != expected || slot.pp != slot.max_pp {
                mismatches.push(format!(
                    "{id}: battler {}/{}, showdown {expected}",
                    slot.pp, slot.max_pp
                ));
            }
        }
    }
    assert!(ids.len() > 400, "only {} moves compared", ids.len());
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn public_health_matches_showdown() {
    let cases = cases();
    for case in cases["health"].as_array().unwrap() {
        let hp = case["hp"].as_u64().unwrap() as u16;
        let max_hp = case["maxhp"].as_u64().unwrap() as u16;
        assert_eq!(
            champions::public_health(hp, max_hp).1,
            case["shared"].as_str().unwrap(),
            "{case}"
        );
    }
}

#[test]
fn engine_logs_champions_public_health() {
    let mut b = battle(
        Rules::Champions,
        BattleType::Singles,
        1,
        team(vec![mon("Alakazam", "No Ability", None, &["Seismic Toss"])]),
        team(vec![mon("Blissey", "No Ability", None, &["Splash"])]),
    );
    b.start().unwrap();
    choose(&mut b, "move 0", "move 0");
    let log = log(&b);
    // Public lines carry a percentage without a decimal; private lines carry exact HP.
    let public: Vec<_> = log
        .iter()
        .filter(|l| l.starts_with("damage|") && l.contains("/100"))
        .collect();
    assert_eq!(public.len(), 1, "{log:#?}");
}

#[test]
fn mainline_rules_are_untouched_by_default() {
    let options = battler::CoreBattleEngineOptions::default();
    assert_eq!(options.champions, battler::ChampionsRules::default());
    let mainline = one_mon_battle(
        Rules::Mainline,
        mon("Pikachu", "No Ability", None, &["Splash"]),
    );
    let mut mainline = mainline;
    // Mainline level-50 Pikachu with zero EVs/IVs: (2 * 35 + 0 + 0) * 50 / 100 + 50 + 10.
    assert_eq!(mainline.player_data("p1").unwrap().mons[0].max_hp, 95);
}
