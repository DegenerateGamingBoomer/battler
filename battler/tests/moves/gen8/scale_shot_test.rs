use anyhow::Result;
use battler::{
    BattleType,
    CoreBattleEngineSpeedSortTieResolution,
    PublicCoreBattle,
    TeamData,
    WrapResultError,
};
use battler_test_utils::{
    LogMatch,
    TestBattleBuilder,
    assert_logs_since_turn_eq,
    static_local_data_store,
};

fn team(species: &str, level: u8) -> Result<TeamData> {
    serde_json::from_str(&format!(
        r#"{{
            "members": [
                {{
                    "name": "{species}",
                    "species": "{species}",
                    "ability": "No Ability",
                    "moves": [
                        "Scale Shot",
                        "Clanging Scales"
                    ],
                    "nature": "Hardy",
                    "level": {level}
                }}
            ]
        }}"#
    ))
    .wrap_error()
}

fn make_battle(seed: u64, team_1: TeamData, team_2: TeamData) -> Result<PublicCoreBattle<'static>> {
    TestBattleBuilder::new()
        .with_battle_type(BattleType::Singles)
        .with_seed(seed)
        .with_team_validation(false)
        .with_pass_allowed(true)
        .with_speed_sort_tie_resolution(CoreBattleEngineSpeedSortTieResolution::Keep)
        .add_player_to_side_1("player-1", "Player 1")
        .add_player_to_side_2("player-2", "Player 2")
        .with_team("player-1", team_1)
        .with_team("player-2", team_2)
        .build(static_local_data_store())
}

#[test]
fn scale_shot_changes_stats_after_all_hits() {
    let mut battle = make_battle(
        0,
        team("Kommo-o", 50).unwrap(),
        team("Blissey", 100).unwrap(),
    )
    .unwrap();
    assert_matches::assert_matches!(battle.start(), Ok(()));

    assert_matches::assert_matches!(battle.set_player_choice("player-1", "move 0"), Ok(()));
    assert_matches::assert_matches!(battle.set_player_choice("player-2", "pass"), Ok(()));

    let expected_logs = serde_json::from_str::<Vec<LogMatch>>(
        r#"[
            "move|mon:Kommo-o,player-1,1|name:Scale Shot|target:Blissey,player-2,1",
            "split|side:1",
            "damage|mon:Blissey,player-2,1|health:547/620",
            "damage|mon:Blissey,player-2,1|health:89/100",
            "animatemove|mon:Kommo-o,player-1,1|name:Scale Shot|target:Blissey,player-2,1",
            "split|side:1",
            "damage|mon:Blissey,player-2,1|health:478/620",
            "damage|mon:Blissey,player-2,1|health:78/100",
            "hitcount|hits:2",
            "unboost|mon:Kommo-o,player-1,1|stat:def|by:1",
            "boost|mon:Kommo-o,player-1,1|stat:spe|by:1",
            "residual",
            "turn|turn:2"
        ]"#,
    )
    .unwrap();
    assert_logs_since_turn_eq(&battle, 1, &expected_logs);
}

#[test]
fn clanging_scales_does_not_lower_defense_after_knocking_out_last_foe() {
    let mut battle = make_battle(
        0,
        team("Kommo-o", 100).unwrap(),
        team("Pikachu", 1).unwrap(),
    )
    .unwrap();
    assert_matches::assert_matches!(battle.start(), Ok(()));

    assert_matches::assert_matches!(battle.set_player_choice("player-1", "move 1"), Ok(()));
    assert_matches::assert_matches!(battle.set_player_choice("player-2", "pass"), Ok(()));

    let expected_logs = serde_json::from_str::<Vec<LogMatch>>(
        r#"[
            "move|mon:Kommo-o,player-1,1|name:Clanging Scales",
            "split|side:1",
            "damage|mon:Pikachu,player-2,1|health:0",
            "damage|mon:Pikachu,player-2,1|health:0",
            "faint|mon:Pikachu,player-2,1",
            "win|side:0"
        ]"#,
    )
    .unwrap();
    assert_logs_since_turn_eq(&battle, 1, &expected_logs);
}
