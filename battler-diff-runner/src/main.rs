//! battler side of the Showdown differential harness (spec P1c).
//!
//! Reads one JSON command per line on stdin and answers with one JSON line on stdout:
//!
//! - `{"op": "start", "seed": u32, "teams": [packed, packed]}` starts a Champions doubles battle.
//! - `{"op": "choose", "choices": [p1, p2]}` makes Showdown-syntax choices (`""` for none).
//!
//! Every answer carries the draws and speed ties since the last answer, the new Showdown protocol
//! lines (translated by `battler-showdown-compat`), both translated requests and the normalized
//! state. The driver is `compat/diff/driver.js`.

mod semantic;
mod state;

use std::{
    any::Any,
    io::{
        BufRead,
        Write,
    },
    sync::LazyLock,
};

use anyhow::{
    Context as _,
    Result,
    bail,
};
use battler::{
    BattleType,
    CoreBattleEngineSpeedSortTieResolution,
    PublicCoreBattle,
};
use battler_champions::{
    ChampionsDataStore,
    Regulation,
};
use battler_local_data::LocalDataStore;
use battler_showdown_compat::{
    choice::{
        self,
        ChoiceContext,
    },
    protocol::{
        Translator,
        TranslatorConfig,
    },
    request::{
        self,
        SideInfo,
    },
    team::{
        ConvertOptions,
        ConvertedTeam,
        convert_text,
    },
};
use battler_test_utils::TestBattleBuilder;
use semantic::SemanticRng;
use serde::Deserialize;
use serde_json::{
    Value,
    json,
};

/// battler's data directory: `DATA_DIR`, or the repository's `battle-data/data`.
static LOCAL_DATA: LazyLock<LocalDataStore> = LazyLock::new(|| {
    let dir = std::env::var("DATA_DIR")
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../battle-data/data").to_owned());
    LocalDataStore::new(dir).expect("battle data directory")
});

static CHAMPIONS: LazyLock<ChampionsDataStore<'static>> =
    LazyLock::new(|| ChampionsDataStore::new(&*LOCAL_DATA, Regulation::MC).unwrap());

const PLAYERS: [&str; 2] = ["p1", "p2"];

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
enum Command {
    Start { seed: u32, teams: Vec<String> },
    Choose { choices: Vec<String> },
}

struct Session {
    battle: PublicCoreBattle<'static>,
    species: Vec<state::SpeciesNames>,
    translator: Translator,
    log_pos: usize,
    out_pos: usize,
}

fn semantic_rng<'b>(battle: &'b mut PublicCoreBattle) -> &'b mut SemanticRng {
    (battle.internal.prng.as_mut().as_any_mut() as &mut dyn Any)
        .downcast_mut::<SemanticRng>()
        .expect("battle uses the semantic random source")
}

impl Session {
    fn start(seed: u32, teams: &[String]) -> Result<Self> {
        let data: &'static ChampionsDataStore<'static> = &CHAMPIONS;
        let options = ConvertOptions {
            default_level: 100,
            force_level: None,
        };
        let teams: Vec<ConvertedTeam> = teams
            .iter()
            .map(|t| convert_text(data, t, options))
            .collect::<Result<_>>()?;
        let mut builder = TestBattleBuilder::new()
            .with_battle_type(BattleType::Doubles)
            .with_seed(seed as u64)
            .with_rng_factory(|seed| Box::new(SemanticRng::new(seed.unwrap_or(0) as u32)))
            .with_team_validation(false)
            .with_mega_evolution(true)
            .with_bag_items(true)
            .with_infinite_bags(true)
            .with_champions_rules()
            .with_speed_sort_tie_resolution(CoreBattleEngineSpeedSortTieResolution::Random)
            .add_player_to_side_1("p1", "p1")
            .add_player_to_side_2("p2", "p2")
            .with_team("p1", teams[0].team.clone())
            .with_team("p2", teams[1].team.clone());
        for rule in ["Team Preview", "Picked Team Size = Auto"] {
            builder = builder.with_rule(rule);
        }
        let mut battle = builder.build(data)?;
        battle.start()?;
        let species = teams.iter().map(state::SpeciesNames::new).collect();
        let translator = Translator::new(TranslatorConfig {
            players: PLAYERS.iter().map(|p| p.to_string()).collect(),
            teams,
        });
        Ok(Self {
            battle,
            species,
            translator,
            log_pos: 0,
            out_pos: 0,
        })
    }

    fn choose(&mut self, choices: &[String]) -> Result<()> {
        for (player, choice) in PLAYERS.iter().zip(choices) {
            if choice.is_empty() {
                continue;
            }
            let mut context = ChoiceContext::default();
            match self.battle.request_for_player(player) {
                Ok(Some(battler::Request::Switch(request))) => {
                    context.switch_positions = Some(request.needs_switch.clone());
                }
                Ok(Some(battler::Request::Turn(_))) => {
                    let mons = self.battle.player_data(player)?.mons;
                    context.turn_positions = Some(
                        mons.iter()
                            .filter(|mon| mon.active && mon.hp > 0)
                            .filter_map(|mon| mon.player_active_position)
                            .collect(),
                    );
                }
                _ => (),
            }
            let mut mons = self.battle.player_data(player)?.mons;
            mons.sort_by_key(|mon| mon.player_effective_team_position);
            context.showdown_order = mons.iter().map(|mon| mon.player_team_position).collect();
            let translated = choice::translate(choice, &context)
                .with_context(|| format!("translating {player} choice '{choice}'"))?;
            self.battle
                .set_player_choice(player, &translated)
                .with_context(|| format!("{player} choice '{choice}' ('{translated}')"))?;
        }
        Ok(())
    }

    fn request(&mut self, index: usize) -> Result<Value> {
        let player = PLAYERS[index];
        let multiple_requests = PLAYERS
            .iter()
            .filter(|p| matches!(self.battle.request_for_player(p), Ok(Some(_))))
            .count()
            > 1;
        let request = self.battle.request_for_player(player)?;
        let data = self.battle.player_data(player)?;
        let info = SideInfo {
            id: format!("p{}", index + 1),
            name: player.to_owned(),
            active_count: 2,
            multiple_requests,
            species: self.species[index].showdown_names(),
        };
        Ok(
            request::translate(request.as_ref(), &info, &data, self.battle.ended())?
                .unwrap_or(Value::Null),
        )
    }

    /// Everything that happened since the last report.
    fn report(&mut self) -> Result<Value> {
        let log: Vec<String> = self
            .battle
            .full_log()
            .skip(self.log_pos)
            .map(str::to_owned)
            .collect();
        self.log_pos += log.len();
        let mut translate_error = None;
        for line in &log {
            if let Err(error) = self.translator.feed(line) {
                translate_error = Some(format!("translating '{line}': {error:#}"));
                break;
            }
        }
        if translate_error.is_none() {
            if let Err(error) = self.translator.flush() {
                translate_error = Some(format!("{error:#}"));
            }
        }
        let protocol = self.translator.lines()[self.out_pos..].to_vec();
        self.out_pos = self.translator.lines().len();

        let requests = (0..2)
            .map(|i| self.request(i))
            .collect::<Result<Vec<_>>>()?;
        let players: Vec<String> = PLAYERS.iter().map(|p| p.to_string()).collect();
        let state = state::normalize(&mut self.battle, &players, &self.species)?;
        let ended = self.battle.ended();
        let rng = semantic_rng(&mut self.battle);
        let draws = std::mem::take(&mut rng.draws);
        let ties = std::mem::take(&mut rng.ties);
        let raw_draws = std::mem::take(&mut rng.raw_draws);
        Ok(json!({
            "ok": true,
            "draws": draws,
            "ties": ties,
            "rawDraws": raw_draws,
            "log": log,
            "protocol": protocol,
            "translateError": translate_error,
            "requests": requests,
            "state": state,
            "ended": ended,
        }))
    }
}

fn handle(session: &mut Option<Session>, line: &str) -> Result<Value> {
    let command: Command = serde_json::from_str(line).context("invalid command")?;
    match command {
        Command::Start { seed, teams } => {
            let mut new = Session::start(seed, &teams)?;
            let report = new.report()?;
            *session = Some(new);
            Ok(report)
        }
        Command::Choose { choices } => {
            let Some(session) = session.as_mut() else {
                bail!("no battle started");
            };
            if let Err(error) = session.choose(&choices) {
                // Report what happened before the error too, so the driver sees partial progress.
                let mut report = session.report().unwrap_or_else(|_| json!({}));
                report["ok"] = json!(false);
                report["error"] = json!(format!("{error:#}"));
                return Ok(report);
            }
            session.report()
        }
    }
}

fn main() -> Result<()> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    let mut session = None;
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            handle(&mut session, &line)
        })) {
            Ok(Ok(response)) => response,
            Ok(Err(error)) => json!({"ok": false, "error": format!("{error:#}")}),
            Err(panic) => {
                session = None;
                let message = panic
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default();
                json!({"ok": false, "error": format!("panic: {message}")})
            }
        };
        writeln!(stdout, "{response}")?;
        stdout.flush()?;
    }
    Ok(())
}
