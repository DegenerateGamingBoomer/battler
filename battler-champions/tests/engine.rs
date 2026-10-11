//! Engine rule changes under Champions rules (`battler::ChampionsRules`), each compared against
//! mainline behavior.

mod common;

use battler::{
    BattleType,
    PublicCoreBattle,
};
use common::*;
use serde_json::Value;

fn singles(rules: Rules, p1: Vec<Value>, p2: Vec<Value>) -> PublicCoreBattle<'static> {
    let mut b = battle(rules, BattleType::Singles, 1, team(p1), team(p2));
    b.start().unwrap();
    b
}

fn species(battle: &mut PublicCoreBattle, player: &str, index: usize) -> String {
    battle.player_data(player).unwrap().mons[index]
        .species
        .clone()
}

fn hp(battle: &mut PublicCoreBattle, player: &str, index: usize) -> u16 {
    battle.player_data(player).unwrap().mons[index].hp
}

#[test]
fn mega_evolution_persists_after_fainting() {
    let run = |rules| {
        let mut b = singles(
            rules,
            vec![
                mon(
                    "Charizard",
                    "No Ability",
                    Some("Charizardite X"),
                    &["Splash"],
                ),
                mon("Pikachu", "No Ability", None, &["Splash"]),
            ],
            vec![mon("Garchomp", "No Ability", None, &["Stone Edge"])],
        );
        choose(&mut b, "move 0,mega", "move 0");
        assert_eq!(species(&mut b, "p1", 0), "Charizard-Mega-X");
        for _ in 0..4 {
            if b.player_data("p1").unwrap().mons[0].hp == 0 {
                break;
            }
            choose(&mut b, "move 0", "move 0");
        }
        assert_eq!(hp(&mut b, "p1", 0), 0, "Charizard should have fainted");
        species(&mut b, "p1", 0)
    };
    assert_eq!(run(Rules::Mainline), "Charizard");
    assert_eq!(run(Rules::Champions), "Charizard-Mega-X");
}

/// Rage Fist damage dealt to a fixed target after `setup` has happened to the user.
fn rage_fist_damage(rules: Rules, foe_move: &str, switch_out_and_in: bool, turns: usize) -> u16 {
    let mut b = singles(
        rules,
        vec![
            mon("Primeape", "No Ability", None, &["Rage Fist", "Splash"]),
            mon("Pikachu", "No Ability", None, &["Splash"]),
        ],
        vec![mon("Dragonite", "No Ability", None, &[foe_move, "Splash"])],
    );
    fill_rng(&mut b, 0);
    for _ in 0..turns {
        choose(&mut b, "move 1", "move 0");
    }
    if switch_out_and_in {
        choose(&mut b, "switch 1", "move 1");
        choose(&mut b, "switch 0", "move 1");
    }
    let before = hp(&mut b, "p2", 0);
    choose(&mut b, "move 0", "move 1");
    before - hp(&mut b, "p2", 0)
}

#[test]
fn rage_fist_counter_resets_when_the_user_switches_out() {
    let fresh = rage_fist_damage(Rules::Champions, "Tackle", false, 0);
    // Hit twice, then switch out and back in.
    let after_switch = rage_fist_damage(Rules::Champions, "Tackle", true, 2);
    let without_switch = rage_fist_damage(Rules::Champions, "Tackle", false, 2);
    assert_eq!(after_switch, fresh);
    assert!(without_switch > fresh);
}

#[test]
fn rage_fist_counter_counts_every_hit_of_a_multi_hit_move() {
    // One Double Kick (2 hits) counts like two Tackles under Champions, and like one in mainline.
    let two_tackles = rage_fist_damage(Rules::Champions, "Tackle", false, 2);
    let one_double_hit = rage_fist_damage(Rules::Champions, "Double Kick", false, 1);
    assert_eq!(one_double_hit, two_tackles);
    let mainline_two_tackles = rage_fist_damage(Rules::Mainline, "Tackle", false, 2);
    let mainline_one_double_hit = rage_fist_damage(Rules::Mainline, "Double Kick", false, 1);
    assert!(mainline_one_double_hit < mainline_two_tackles);
}

#[test]
fn eject_button_works_against_sheer_force() {
    let run = |rules| {
        let mut b = singles(
            rules,
            vec![mon("Nidoking", "Sheer Force", None, &["Flamethrower"])],
            vec![
                mon("Snorlax", "No Ability", Some("Eject Button"), &["Splash"]),
                mon("Pikachu", "No Ability", None, &["Splash"]),
            ],
        );
        choose(&mut b, "move 0", "move 0");
        log(&b)
    };
    assert!(
        !has(&run(Rules::Mainline), "Eject Button"),
        "{:#?}",
        run(Rules::Mainline)
    );
    assert!(
        has(&run(Rules::Champions), "Eject Button"),
        "{:#?}",
        run(Rules::Champions)
    );
}

#[test]
fn parental_bond_single_hit_logs_no_hit_count() {
    let run = |rules| {
        let mut b = singles(
            rules,
            vec![mon("Kangaskhan", "Parental Bond", None, &["Hyper Beam"])],
            vec![mon("Pikachu", "No Ability", None, &["Splash"])],
        );
        fill_rng(&mut b, 0);
        choose(&mut b, "move 0", "move 0");
        log(&b)
    };
    let mainline = run(Rules::Mainline);
    let champions = run(Rules::Champions);
    // Pikachu faints to the first hit, so Parental Bond hits once.
    assert!(has(&mainline, "faint|mon:Pikachu"), "{mainline:#?}");
    assert!(has(&mainline, "hitcount"), "{mainline:#?}");
    assert!(!has(&champions, "hitcount"), "{champions:#?}");
}
