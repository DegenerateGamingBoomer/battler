//! Ability, item and move effects whose behavior changes under Champions rules.

mod common;

use battler::{
    BattleType,
    PublicCoreBattle,
    Request,
};
use common::*;
use serde_json::Value;

fn singles(rules: Rules, p1: Vec<Value>, p2: Vec<Value>) -> PublicCoreBattle<'static> {
    let mut b = battle(rules, BattleType::Singles, 1, team(p1), team(p2));
    b.start().unwrap();
    b
}

fn needs_switch(battle: &PublicCoreBattle, player: &str) -> bool {
    matches!(
        battle.request_for_player(player).unwrap(),
        Some(Request::Switch(_))
    )
}

#[test]
fn eject_button_does_not_cancel_the_attackers_pivot() {
    let run = |rules| {
        let mut b = singles(
            rules,
            vec![
                mon("Scyther", "No Ability", None, &["U-turn"]),
                mon("Pikachu", "No Ability", None, &["Splash"]),
            ],
            vec![
                mon("Snorlax", "No Ability", Some("Eject Button"), &["Splash"]),
                mon("Raichu", "No Ability", None, &["Splash"]),
            ],
        );
        fill_rng(&mut b, 5);
        choose(&mut b, "move 0", "move 0");
        (needs_switch(&b, "p1"), needs_switch(&b, "p2"))
    };
    // Mainline: the Eject Button holder switches, which cancels the U-turn.
    assert_eq!(run(Rules::Mainline), (false, true));
    // Champions: both Pokémon switch.
    assert_eq!(run(Rules::Champions), (true, true));
}

#[test]
fn run_away_is_never_trapped() {
    let trapped = |rules| {
        let b = singles(
            rules,
            vec![
                mon("Rattata", "Run Away", None, &["Splash"]),
                mon("Pikachu", "No Ability", None, &["Splash"]),
            ],
            vec![mon("Dugtrio", "Arena Trap", None, &["Splash"])],
        );
        turn_request(&b, "p1").active[0].trapped
    };
    assert!(trapped(Rules::Mainline));
    assert!(!trapped(Rules::Champions));
}

#[test]
fn emergency_exit_is_still_suppressed_by_sheer_force_but_berserk_is_not() {
    let switched = |rules, attacker_ability: &str| {
        let mut b = singles(
            rules,
            vec![mon("Nidoking", attacker_ability, None, &["Flamethrower"])],
            vec![
                mon("Golisopod", "Emergency Exit", None, &["Splash"]),
                mon("Pikachu", "No Ability", None, &["Splash"]),
            ],
        );
        fill_rng(&mut b, 5);
        for _ in 0..5 {
            if needs_switch(&b, "p2") || b.ended() {
                break;
            }
            choose(&mut b, "move 0", "move 0");
        }
        // A switch request while Golisopod is still standing is Emergency Exit, not a faint.
        needs_switch(&b, "p2") && b.player_data("p2").unwrap().mons[0].hp > 0
    };
    // Without Sheer Force the Emergency Exit user leaves once it drops under half HP.
    assert!(switched(Rules::Champions, "No Ability"));
    // With Sheer Force it stays, in both rule sets.
    assert!(!switched(Rules::Mainline, "Sheer Force"));
    assert!(!switched(Rules::Champions, "Sheer Force"));
}

#[test]
fn berserk_activates_through_sheer_force() {
    let boosted = |rules| {
        let mut b = singles(
            rules,
            vec![mon("Nidoking", "Sheer Force", None, &["Flamethrower"])],
            vec![mon("Drampa", "Berserk", None, &["Splash"])],
        );
        fill_rng(&mut b, 5);
        for _ in 0..5 {
            if b.ended() {
                break;
            }
            choose(&mut b, "move 0", "move 0");
        }
        count(&log(&b), "boost|mon:Drampa")
    };
    assert_eq!(boosted(Rules::Mainline), 0);
    assert_eq!(boosted(Rules::Champions), 1);
}

#[test]
fn curse_fails_for_a_non_ghost_when_no_stat_can_change() {
    let mut b = singles(
        Rules::Champions,
        vec![mon("Snorlax", "No Ability", None, &["Curse"])],
        vec![mon("Blissey", "No Ability", None, &["Splash"])],
    );
    for _ in 0..12 {
        choose(&mut b, "move 0", "move 0");
    }
    let log = log(&b);
    // Six successful Curses cap Attack and Defense at +6 and Speed at -6, then it fails.
    assert_eq!(count(&log, "boost|mon:Snorlax"), 6 * 2 + 0, "{log:#?}");
}

#[test]
fn unseen_fist_contact_moves_hit_through_protect_at_a_quarter() {
    let damage = |rules, foe_move: &str| {
        let mut b = singles(
            rules,
            vec![mon("Urshifu", "Unseen Fist", None, &["Aqua Jet"])],
            vec![mon("Snorlax", "No Ability", None, &[foe_move])],
        );
        fill_rng(&mut b, 5);
        let before = b.player_data("p2").unwrap().mons[0].hp;
        choose(&mut b, "move 0", "move 0");
        before - b.player_data("p2").unwrap().mons[0].hp
    };
    for rules in [Rules::Mainline, Rules::Champions] {
        let plain = damage(rules, "Splash") as i32;
        let protected = damage(rules, "Protect") as i32;
        assert!(protected > 0, "{rules:?}");
        assert!(
            (plain - 4 * protected).abs() <= 4,
            "{rules:?}: plain {plain}, protected {protected}"
        );
    }
}
