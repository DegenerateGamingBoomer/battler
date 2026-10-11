#![allow(dead_code)]

use std::{
    fs,
    path::PathBuf,
};

use battler::{
    BattleType,
    PublicCoreBattle,
};
use battler_champions::{
    ChampionsDataStore,
    Regulation,
};
use battler_showdown_compat::{
    choice::{
        ChoiceContext,
        translate,
    },
    team::{
        ConvertOptions,
        ConvertedTeam,
        convert_text,
    },
};
use battler_test_utils::{
    TestBattleBuilder,
    get_controlled_rng_for_battle,
    static_local_data_store,
};
use serde_json::Value;

/// A draw that is `n - 1` modulo every `n` the engine uses (it is divisible by 1..=40 and 100), so
/// that battler's `next % n` picks the largest value, as Showdown does for its largest draw.
pub const MAX_DRAW: u64 = 5342931457063200 - 1;

pub static CHAMPIONS: std::sync::LazyLock<ChampionsDataStore<'static>> =
    std::sync::LazyLock::new(|| {
        ChampionsDataStore::new(static_local_data_store(), Regulation::MC).unwrap()
    });

/// A Showdown reference run (`compat/gen-fixtures.js`).
pub struct Fixture {
    pub name: String,
    pub gametype: String,
    pub teams: Vec<String>,
    pub steps: Vec<[String; 2]>,
    pub log: Vec<String>,
    pub requests: Vec<Vec<Value>>,
    pub rng: String,
    pub raw: Value,
}

pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../compat/fixtures")
}

pub fn fixture_names() -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(fixtures_dir())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            e.path()
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
        })
        .collect();
    names.sort();
    names
}

pub fn load(name: &str) -> Fixture {
    load_path(&fixtures_dir().join(format!("{name}.json")), name)
}

pub fn load_path(path: &std::path::Path, name: &str) -> Fixture {
    let raw: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    let strings = |v: &Value| -> Vec<String> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_owned())
            .collect()
    };
    Fixture {
        name: name.to_owned(),
        rng: raw["rng"].as_str().unwrap_or("zero").to_owned(),
        gametype: raw["gametype"].as_str().unwrap().to_owned(),
        teams: strings(&raw["teams"]),
        steps: raw["steps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| {
                [
                    s[0].as_str().unwrap().to_owned(),
                    s[1].as_str().unwrap().to_owned(),
                ]
            })
            .collect(),
        log: strings(&raw["log"]),
        requests: raw["requests"]
            .as_array()
            .map(|requests| {
                requests
                    .iter()
                    .map(|r| r.as_array().unwrap().clone())
                    .collect()
            })
            .unwrap_or_default(),
        raw,
    }
}

/// A battler battle set up like the fixture's Showdown battle.
pub struct Run {
    pub battle: PublicCoreBattle<'static>,
    pub teams: Vec<ConvertedTeam>,
}

pub fn set_up(fixture: &Fixture) -> Run {
    let data: &'static ChampionsDataStore<'static> = &CHAMPIONS;
    let options = ConvertOptions {
        default_level: 100,
        force_level: None,
    };
    let teams: Vec<ConvertedTeam> = fixture
        .teams
        .iter()
        .map(|t| convert_text(data, t, options).unwrap())
        .collect();
    let battle_type = match fixture.gametype.as_str() {
        "singles" => BattleType::Singles,
        "doubles" => BattleType::Doubles,
        other => panic!("unsupported gametype {other}"),
    };
    let mut builder = TestBattleBuilder::new()
        .with_battle_type(battle_type)
        .with_seed(1)
        .with_team_validation(false)
        .with_mega_evolution(true)
        .with_bag_items(true)
        .with_infinite_bags(true)
        .with_controlled_rng(true)
        .with_champions_rules()
        .with_speed_sort_tie_resolution(battler::CoreBattleEngineSpeedSortTieResolution::Keep)
        .add_player_to_side_1("p1", "p1")
        .add_player_to_side_2("p2", "p2")
        .with_team("p1", teams[0].team.clone())
        .with_team("p2", teams[1].team.clone());
    for rule in ["Team Preview", "Picked Team Size = Auto"] {
        builder = builder.with_rule(rule);
    }
    let mut battle = builder.build(data).unwrap();
    get_controlled_rng_for_battle(&mut battle)
        .unwrap()
        .set_constant(Some(if fixture.rng == "max" { MAX_DRAW } else { 0 }));
    battle.start().unwrap();
    Run { battle, teams }
}

impl Run {
    /// The context for translating `player`'s next choice, from battler's current request.
    pub fn choice_context(&self, player: &str) -> ChoiceContext {
        let mut context = ChoiceContext::default();
        if let Ok(Some(battler::Request::Switch(request))) = self.battle.request_for_player(player)
        {
            context.switch_positions = Some(request.needs_switch.clone());
        }
        context
    }

    /// Makes one player's choice from a Showdown choice string.
    pub fn choose(&mut self, player: &str, choice: &str) -> anyhow::Result<()> {
        let context = self.choice_context(player);
        let translated = translate(choice, &context)?;
        self.battle.set_player_choice(player, &translated)
    }

    /// Plays the fixture's steps. Returns after the last step or when the battle ends.
    pub fn play(&mut self, fixture: &Fixture) {
        for (i, step) in fixture.steps.iter().enumerate() {
            for (player, choice) in ["p1", "p2"].iter().zip(step) {
                if choice.is_empty() {
                    continue;
                }
                self.choose(player, choice)
                    .unwrap_or_else(|e| panic!("step {i}: {player} choice '{choice}': {e:#}"));
            }
        }
    }

    pub fn log(&self) -> Vec<String> {
        self.battle.full_log().map(str::to_owned).collect()
    }
}

use battler_showdown_compat::protocol::{
    Translator,
    TranslatorConfig,
};

impl Run {
    /// The battler log translated to Showdown protocol lines.
    pub fn translated(&self) -> anyhow::Result<Vec<String>> {
        let config = TranslatorConfig {
            players: vec!["p1".to_owned(), "p2".to_owned()],
            teams: self.teams.clone(),
        };
        let log = self.log();
        Translator::translate_all(config, log.iter().map(String::as_str))
    }
}

/// Differences from Showdown that are intentional and documented in `compat/allowlist.md`.
///
/// Both logs are normalized before comparison; the rules only blank out information that battler
/// cannot reproduce.
pub fn apply_allowlist(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        // R2: Showdown's `-hint` lines are advice for the client, not battle events.
        .filter(|line| !line.starts_with("|-hint|"))
        .map(|line| {
            let parts: Vec<&str> = line.split('|').collect();
            // R1: Showdown picks the printed target of a spread or side-targeting move with a
            // random draw that battler does not make.
            let side_targeting = ["Spikes", "Stealth Rock", "Toxic Spikes", "Sticky Web"];
            if parts.get(1) == Some(&"move")
                && parts.len() > 4
                && (line.contains("|[spread] ")
                    // R3: a move with no target left names the fainted Pokémon it had picked, which
                    // battler does not log.
                    || line.ends_with("|[notarget]")
                    || side_targeting.contains(&parts[3]))
            {
                let mut parts: Vec<String> = parts.iter().map(|s| s.to_string()).collect();
                parts[4] = "*".to_owned();
                return parts.join("|");
            }
            line.clone()
        })
        .collect()
}

/// A readable line diff of two logs, or `None` if they are equal.
pub fn diff(expected: &[String], actual: &[String]) -> Option<String> {
    if expected == actual {
        return None;
    }
    let mut out = String::new();
    let max = expected.len().max(actual.len());
    let mut shown = 0;
    for i in 0..max {
        let (e, a) = (expected.get(i), actual.get(i));
        if e != a {
            out.push_str(&format!(
                "line {i}:\n  showdown: {}\n  battler:  {}\n",
                e.map(String::as_str).unwrap_or("<none>"),
                a.map(String::as_str).unwrap_or("<none>")
            ));
            shown += 1;
            if shown >= 12 {
                out.push_str("...\n");
                break;
            }
        }
    }
    Some(out)
}
