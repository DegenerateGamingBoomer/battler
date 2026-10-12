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

fn team() -> Result<TeamData> {
    serde_json::from_str(
        r#"{
            "members": [
                {
                    "name": "Corviknight",
                    "species": "Corviknight",
                    "ability": "Mirror Armor",
                    "moves": [
                        "Swords Dance",
                        "Growl"
                    ],
                    "nature": "Hardy",
                    "level": 100
                }
            ]
        }"#,
    )
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
fn mirror_armor_reflects_stat_drops() {
    let mut battle = make_battle(0, team().unwrap(), team().unwrap()).unwrap();
    assert_matches::assert_matches!(battle.start(), Ok(()));

    assert_matches::assert_matches!(battle.set_player_choice("player-1", "move 0"), Ok(()));
    assert_matches::assert_matches!(battle.set_player_choice("player-2", "move 1"), Ok(()));

    let expected_logs = serde_json::from_str::<Vec<LogMatch>>(
        r#"[
            "move|mon:Corviknight,player-1,1|name:Swords Dance|target:Corviknight,player-1,1",
            "boost|mon:Corviknight,player-1,1|stat:atk|by:2",
            "move|mon:Corviknight,player-2,1|name:Growl|noanim",
            "ability|mon:Corviknight,player-1,1|ability:Mirror Armor",
            "unboost|mon:Corviknight,player-2,1|stat:atk|by:1|from:ability:Mirror Armor|of:Corviknight,player-1,1",
            "fail|mon:Corviknight,player-2,1",
            "residual",
            "turn|turn:2"
        ]"#,
    )
    .unwrap();
    assert_logs_since_turn_eq(&battle, 1, &expected_logs);
}

fn incineroar() -> Result<TeamData> {
    serde_json::from_str(
        r#"{
            "members": [
                {
                    "name": "Incineroar",
                    "species": "Incineroar",
                    "ability": "No Ability",
                    "moves": [
                        "Parting Shot"
                    ],
                    "nature": "Hardy",
                    "level": 100
                },
                {
                    "name": "Pikachu",
                    "species": "Pikachu",
                    "ability": "No Ability",
                    "moves": [],
                    "nature": "Hardy",
                    "level": 100
                }
            ]
        }"#,
    )
    .wrap_error()
}

#[test]
fn parting_shot_user_switches_out_when_mirror_armor_reflects_drops() {
    let mut battle = make_battle(0, incineroar().unwrap(), team().unwrap()).unwrap();
    assert_matches::assert_matches!(battle.start(), Ok(()));

    assert_matches::assert_matches!(battle.set_player_choice("player-1", "move 0"), Ok(()));
    assert_matches::assert_matches!(battle.set_player_choice("player-2", "pass"), Ok(()));
    assert_matches::assert_matches!(battle.set_player_choice("player-1", "switch 1"), Ok(()));

    let expected_logs = serde_json::from_str::<Vec<LogMatch>>(
        r#"[
            "move|mon:Incineroar,player-1,1|name:Parting Shot|target:Corviknight,player-2,1",
            "ability|mon:Corviknight,player-2,1|ability:Mirror Armor",
            "unboost|mon:Incineroar,player-1,1|stat:atk|by:1|from:ability:Mirror Armor|of:Corviknight,player-2,1",
            "unboost|mon:Incineroar,player-1,1|stat:spa|by:1|from:ability:Mirror Armor|of:Corviknight,player-2,1",
            "continue",
            "split|side:0",
            "switch|player:player-1|position:1|name:Pikachu|health:180/180|species:Pikachu|level:100|gender:U",
            "switch|player:player-1|position:1|name:Pikachu|health:100/100|species:Pikachu|level:100|gender:U",
            "residual",
            "turn|turn:2"
        ]"#,
    )
    .unwrap();
    assert_logs_since_turn_eq(&battle, 1, &expected_logs);
}
