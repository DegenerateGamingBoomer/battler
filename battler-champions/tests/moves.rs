//! Move, ability and item behavior under Champions rules (Showdown `data/mods/champions`).

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

fn move_disabled(battle: &battler::PublicCoreBattle, player: &str, name: &str) -> bool {
    turn_request(battle, player).active[0]
        .moves
        .iter()
        .find(|m| m.name == name)
        .unwrap_or_else(|| panic!("no move {name}"))
        .disabled
}

fn hp(battle: &mut battler::PublicCoreBattle, player: &str, index: usize) -> u16 {
    battle.player_data(player).unwrap().mons[index].hp
}

#[test]
fn fake_out_and_first_impression_are_unselectable_after_the_first_move() {
    for name in ["Fake Out", "First Impression"] {
        let moves = [name, "Splash"];
        let foe = vec![mon("Blissey", "No Ability", None, &["Splash"])];
        let mut champions = singles(
            Rules::Champions,
            vec![mon("Pikachu", "No Ability", None, &moves)],
            foe.clone(),
        );
        assert!(
            !move_disabled(&champions, "p1", name),
            "{name} on the first turn"
        );
        choose(&mut champions, "move 0", "move 0");
        assert!(
            move_disabled(&champions, "p1", name),
            "{name} after the first move"
        );

        // Mainline leaves it selectable and only fails when used.
        let mut mainline = singles(
            Rules::Mainline,
            vec![mon("Pikachu", "No Ability", None, &moves)],
            foe,
        );
        choose(&mut mainline, "move 0", "move 0");
        assert!(!move_disabled(&mainline, "p1", name));
    }
}

#[test]
fn fake_out_counts_move_actions_not_turns() {
    // A mon that cannot act on its first turn out (fully paralyzed) may still use Fake Out later.
    // Here: the user sits out turn 1 by using Splash only on turn 1 after switching in. With a
    // move action on turn 1 the move is disabled; this guards the counter reset on switch-in.
    let mut b = battle(
        Rules::Champions,
        BattleType::Singles,
        1,
        team(vec![
            mon("Blissey", "No Ability", None, &["Splash"]),
            mon("Pikachu", "No Ability", None, &["Fake Out", "Splash"]),
        ]),
        team(vec![mon("Alakazam", "No Ability", None, &["Splash"])]),
    );
    b.start().unwrap();
    choose(&mut b, "switch 1", "move 0");
    assert!(!move_disabled(&b, "p1", "Fake Out"));
    choose(&mut b, "move 0", "move 0");
    assert!(move_disabled(&b, "p1", "Fake Out"));
    choose(&mut b, "switch 0", "move 0");
    choose(&mut b, "switch 1", "move 0");
    assert!(
        !move_disabled(&b, "p1", "Fake Out"),
        "counter resets when switched back in"
    );
}

#[test]
fn belch_and_stuff_cheeks_fail_on_use_instead_of_being_disabled() {
    for name in ["Belch", "Stuff Cheeks"] {
        let foe = vec![mon("Blissey", "No Ability", None, &["Splash"])];
        let mut champions = singles(
            Rules::Champions,
            vec![mon("Garbodor", "No Ability", None, &[name, "Splash"])],
            foe.clone(),
        );
        assert!(!move_disabled(&champions, "p1", name), "{name} selectable");
        let before = hp(&mut champions, "p2", 0);
        choose(&mut champions, "move 0", "move 0");
        assert_eq!(
            hp(&mut champions, "p2", 0),
            before,
            "{name} did nothing without a berry"
        );

        let mainline = singles(
            Rules::Mainline,
            vec![mon("Garbodor", "No Ability", None, &[name, "Splash"])],
            foe,
        );
        assert!(
            move_disabled(&mainline, "p1", name),
            "{name} disabled in mainline"
        );
    }
}

#[test]
fn belch_works_after_a_berry_was_eaten() {
    let mut b = singles(
        Rules::Champions,
        vec![mon(
            "Garbodor",
            "No Ability",
            Some("Oran Berry"),
            &["Belch", "Stuff Cheeks"],
        )],
        vec![mon("Blissey", "No Ability", None, &["Splash"])],
    );
    choose(&mut b, "move 1", "move 0");
    let before = hp(&mut b, "p2", 0);
    choose(&mut b, "move 0", "move 0");
    assert!(hp(&mut b, "p2", 0) < before, "{:#?}", log(&b));
}

#[test]
fn salt_cure_residual_is_halved() {
    for (species, divisor_mainline, divisor_champions) in
        [("Alakazam", 8u16, 16u16), ("Vaporeon", 4, 8)]
    {
        // Returns (max HP, HP lost to the residual damage on the second turn).
        let run = |rules| {
            let mut b = singles(
                rules,
                vec![mon(
                    "Garganacl",
                    "No Ability",
                    None,
                    &["Salt Cure", "Splash"],
                )],
                vec![mon(species, "No Ability", None, &["Splash"])],
            );
            fill_rng(&mut b, 0);
            choose(&mut b, "move 0", "move 0");
            let data = b.player_data("p2").unwrap();
            let (max_hp, after_hit) = (data.mons[0].max_hp, data.mons[0].hp);
            choose(&mut b, "move 1", "move 0");
            (max_hp, after_hit - b.player_data("p2").unwrap().mons[0].hp)
        };
        let (max_hp, lost) = run(Rules::Mainline);
        assert_eq!(lost, max_hp / divisor_mainline, "{species} mainline");
        let (max_hp, lost) = run(Rules::Champions);
        assert_eq!(lost, max_hp / divisor_champions, "{species} champions");
    }
}

#[test]
fn trick_room_reverses_speed_order() {
    let mut b = singles(
        Rules::Champions,
        vec![mon(
            "Slowpoke",
            "No Ability",
            None,
            &["Trick Room", "Tackle"],
        )],
        vec![mon("Blissey", "No Ability", None, &["Splash", "Tackle"])],
    );
    choose(&mut b, "move 0", "move 0");
    choose(&mut b, "move 1", "move 1");
    let log = log(&b);
    let turn2: Vec<_> = log
        .iter()
        .skip_while(|l| !l.starts_with("turn|turn:2"))
        .collect();
    let first_move = turn2.iter().find(|l| l.starts_with("move|")).unwrap();
    assert!(first_move.contains("Slowpoke"), "{turn2:#?}");
}

#[test]
fn healer_cures_an_ally_half_the_time() {
    let run = |rules, draw| {
        let mut b = battle(
            rules,
            BattleType::Doubles,
            1,
            team(vec![
                mon("Audino", "Healer", None, &["Splash"]),
                mon("Alakazam", "No Ability", None, &["Splash"]),
            ]),
            team(vec![
                mon("Blissey", "No Ability", None, &["Thunder Wave", "Splash"]),
                mon("Blissey", "No Ability", None, &["Splash"]),
            ]),
        );
        b.start().unwrap();
        fill_rng(&mut b, draw);
        choose(&mut b, "move 0;move 0", "move 0,2;move 0");
        log(&b)
    };
    let healed = |log: &[String]| count(log, "curestatus|mon:Alakazam");
    // Draw 1 passes the mainline 3/10 roll (1 % 10 < 3) but not the Champions 1/2 roll (1 % 2 !=
    // 0).
    assert_eq!(healed(&run(Rules::Mainline, 1)), 1);
    assert_eq!(healed(&run(Rules::Champions, 1)), 0);
    // Draw 0 passes both.
    assert_eq!(healed(&run(Rules::Champions, 0)), 1);
}

#[test]
fn ability_and_item_changes_are_in_the_overlay() {
    use battler::DataStore;
    use battler_data::Id;
    let overlay = &*CHAMPIONS;
    let inner = battler_test_utils::static_local_data_store();
    let get = |s: &dyn DataStore, id: &str| s.get_ability(&Id::from(id)).unwrap().unwrap().effect;
    assert_ne!(get(overlay, "healer"), get(inner, "healer"));
    assert_ne!(get(overlay, "runaway"), get(inner, "runaway"));
    assert_ne!(get(overlay, "emergencyexit"), get(inner, "emergencyexit"));
    assert_eq!(
        get(overlay, "wimpout"),
        get(inner, "wimpout"),
        "Wimp Out delegates to Emergency Exit"
    );
    let item = |s: &dyn DataStore| {
        s.get_item(&Id::from("ejectbutton"))
            .unwrap()
            .unwrap()
            .effect
    };
    assert_ne!(item(overlay), item(inner));
}

#[test]
fn encore_replacing_a_queued_move_keeps_its_turn_order() {
    let mut b = singles(
        Rules::Champions,
        vec![mon("Pikachu", "No Ability", None, &["Encore", "Splash"])],
        vec![mon("Blissey", "No Ability", None, &["Growl", "Tackle"])],
    );
    choose(&mut b, "move 1", "move 0");
    // Encore replaces Tackle with Growl, which still runs before the end of the turn.
    choose(&mut b, "move 0", "move 1");
    let log = log(&b);
    let turn2: Vec<_> = log
        .iter()
        .skip_while(|l| !l.starts_with("turn|turn:2"))
        .collect();
    let growl = turn2
        .iter()
        .position(|l| l.starts_with("move|mon:Blissey") && l.contains("name:Growl"))
        .unwrap_or_else(|| panic!("{turn2:#?}"));
    let residual = turn2.iter().position(|l| *l == "residual").unwrap();
    assert!(growl < residual, "{turn2:#?}");
    assert!(!has(&log, "name:Tackle"), "{turn2:#?}");
}
