#![allow(dead_code)]

use std::sync::LazyLock;

use battler::{
    BattleType,
    CoreBattleEngineSpeedSortTieResolution,
    DataStore,
    PublicCoreBattle,
    Request,
    TeamData,
};
use battler_champions::{
    ChampionsDataStore,
    Regulation,
};
use battler_test_utils::{
    TestBattleBuilder,
    get_controlled_rng_for_battle,
    static_local_data_store,
};
use serde_json::{
    Value,
    json,
};

pub static CHAMPIONS: LazyLock<ChampionsDataStore<'static>> =
    LazyLock::new(|| ChampionsDataStore::new(static_local_data_store(), Regulation::MC).unwrap());

/// Which rules a test battle runs with.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rules {
    /// Mainline battler data and engine behavior.
    Mainline,
    /// Champions data overlay (and engine rules, once they exist).
    Champions,
}

pub fn mon(species: &str, ability: &str, item: Option<&str>, moves: &[&str]) -> Value {
    json!({
        "name": species,
        "species": species,
        "ability": ability,
        "item": item,
        "moves": moves,
        "nature": "Hardy",
        "gender": "M",
        "level": 50,
    })
}

pub fn team(members: Vec<Value>) -> TeamData {
    serde_json::from_value(json!({ "members": members })).unwrap()
}

pub fn battle(
    rules: Rules,
    battle_type: BattleType,
    seed: u64,
    team_1: TeamData,
    team_2: TeamData,
) -> PublicCoreBattle<'static> {
    let data: &'static dyn DataStore = match rules {
        Rules::Mainline => static_local_data_store(),
        Rules::Champions => &*CHAMPIONS,
    };
    let mut builder = TestBattleBuilder::new()
        .with_battle_type(battle_type)
        .with_seed(seed)
        .with_team_validation(false)
        .with_mega_evolution(true)
        .with_bag_items(true)
        .with_infinite_bags(true)
        .with_pass_allowed(true)
        .with_controlled_rng(true)
        .with_speed_sort_tie_resolution(CoreBattleEngineSpeedSortTieResolution::Keep)
        .add_player_to_side_1("p1", "Player 1")
        .add_player_to_side_2("p2", "Player 2")
        .with_team("p1", team_1)
        .with_team("p2", team_2);
    if rules == Rules::Champions {
        builder = builder.with_champions_rules();
    }
    builder.build(data).unwrap()
}

/// Makes every random draw from now on return `value` (for the next few thousand draws).
pub fn fill_rng(battle: &mut PublicCoreBattle, value: u64) {
    let rng = get_controlled_rng_for_battle(battle).unwrap();
    rng.insert_fake_values_relative_to_sequence_count((1..4000).map(|i| (i, value)));
}

pub fn choose(battle: &mut PublicCoreBattle, p1: &str, p2: &str) {
    battle.set_player_choice("p1", p1).unwrap();
    battle.set_player_choice("p2", p2).unwrap();
}

pub fn log(battle: &PublicCoreBattle) -> Vec<String> {
    battle.full_log().map(str::to_owned).collect()
}

pub fn count(log: &[String], prefix: &str) -> usize {
    log.iter().filter(|l| l.starts_with(prefix)).count()
}

pub fn has(log: &[String], needle: &str) -> bool {
    log.iter().any(|l| l.contains(needle))
}

pub fn turn_request(battle: &PublicCoreBattle, player: &str) -> battler::TurnRequest {
    match battle.request_for_player(player).unwrap() {
        Some(Request::Turn(request)) => request,
        other => panic!("expected a turn request, got {other:?}"),
    }
}
