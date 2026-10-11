//! Status conditions under Champions rules (Showdown `data/mods/champions/conditions.ts`).

mod common;

use battler::BattleType;
use common::*;

fn singles(
    rules: Rules,
    p1: Vec<serde_json::Value>,
    p2: Vec<serde_json::Value>,
) -> battler::PublicCoreBattle<'static> {
    let mut b = battle(rules, BattleType::Singles, 1, team(p1), team(p2));
    b.start().unwrap();
    b
}

/// Pikachu paralyzes Alakazam on turn 1; Alakazam tries to move on turn 2.
fn paralysis_run(rules: Rules, draw: u64) -> Vec<String> {
    let mut b = singles(
        rules,
        vec![mon(
            "Pikachu",
            "No Ability",
            None,
            &["Thunder Wave", "Tackle"],
        )],
        vec![mon("Alakazam", "No Ability", None, &["Lick"])],
    );
    fill_rng(&mut b, draw);
    choose(&mut b, "move 0", "move 0");
    choose(&mut b, "move 1", "move 0");
    log(&b)
}

#[test]
fn full_paralysis_is_one_in_eight() {
    // 4 % 4 == 0 would be a full-paralysis draw at 1/4 but 4 % 8 != 0 at 1/8.
    let mainline = paralysis_run(Rules::Mainline, 4);
    assert!(has(&mainline, "cant|mon:Alakazam"), "{mainline:#?}");
    let champions = paralysis_run(Rules::Champions, 4);
    assert!(!has(&champions, "cant|mon:Alakazam"), "{champions:#?}");
    assert_eq!(count(&champions, "move|mon:Alakazam"), 2);
}

#[test]
fn full_paralysis_still_happens_at_the_lowest_draw() {
    let champions = paralysis_run(Rules::Champions, 0);
    assert!(has(&champions, "cant|mon:Alakazam"), "{champions:#?}");
}

/// Parasect spores Alakazam on turn 1; count how many turns Alakazam cannot move.
fn sleep_run(rules: Rules, draw: u64) -> Vec<String> {
    let mut b = singles(
        rules,
        vec![mon("Parasect", "No Ability", None, &["Spore", "Tackle"])],
        vec![mon("Alakazam", "No Ability", None, &["Tackle"])],
    );
    fill_rng(&mut b, draw);
    choose(&mut b, "move 0", "move 0");
    for _ in 0..5 {
        if b.ended() {
            break;
        }
        choose(&mut b, "move 1", "move 0");
    }
    log(&b)
}

#[test]
fn sleep_lasts_two_or_three_turns() {
    // Draw 2: mainline `random 2 5` gives 4 turns, Champions `sample [2, 3, 3]` gives the last 3.
    let mainline = sleep_run(Rules::Mainline, 2);
    let champions = sleep_run(Rules::Champions, 2);
    let cant = |log: &[String]| count(log, "cant|mon:Alakazam");
    assert_eq!(cant(&mainline), 3, "{mainline:#?}");
    assert_eq!(cant(&champions), 2, "{champions:#?}");
    // Draw 0 picks the shortest sleep: 2 turns means a single turn of sleeping.
    let shortest = sleep_run(Rules::Champions, 0);
    assert_eq!(count(&shortest, "cant|mon:Alakazam"), 1, "{shortest:#?}");
}

/// The slower Slowpoke freezes Blissey at the end of turn 1 (draw 0 satisfies the freeze chance).
fn freeze_run(rules: Rules, draw: u64) -> Vec<String> {
    let mut b = singles(
        rules,
        vec![mon("Slowpoke", "No Ability", None, &["Ice Beam"])],
        vec![mon("Blissey", "No Ability", None, &["Tackle"])],
    );
    // Turn 1: draw 0 satisfies the freeze chance. Afterwards use `draw` for everything else.
    fill_rng(&mut b, 0);
    choose(&mut b, "move 0", "move 0");
    fill_rng(&mut b, draw);
    for _ in 0..7 {
        if b.ended() {
            break;
        }
        choose(&mut b, "move 0", "move 0");
    }
    log(&b)
}

#[test]
fn freeze_thaws_after_at_most_three_turns() {
    // Draw 6 never rolls a thaw (6 % 4 != 0 and 6 % 5 != 0). Mainline stays frozen indefinitely.
    let mainline = freeze_run(Rules::Mainline, 6);
    assert!(count(&mainline, "cant|mon:Blissey") >= 5, "{mainline:#?}");
    let champions = freeze_run(Rules::Champions, 6);
    assert_eq!(count(&champions, "cant|mon:Blissey"), 2, "{champions:#?}");
}
