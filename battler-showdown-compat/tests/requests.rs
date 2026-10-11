mod common;

use battler_showdown_compat::request::{
    SideInfo,
    translate,
};
use common::*;
use serde_json::Value;

fn request_for(run: &mut Run, player: &str, index: usize) -> Option<Value> {
    let multiple_requests = ["p1", "p2"]
        .iter()
        .filter(|p| run.battle.request_for_player(p).unwrap().is_some())
        .count()
        > 1;
    let request = run.battle.request_for_player(player).unwrap();
    let data = run.battle.player_data(player).unwrap();
    let info = SideInfo {
        id: format!("p{}", index + 1),
        name: player.to_owned(),
        active_count: 2,
        multiple_requests,
        species: Default::default(),
    };
    translate(request.as_ref(), &info, &data, run.battle.ended()).unwrap()
}

/// Paths at which two JSON values differ.
fn json_diff(path: &str, a: &Value, b: &Value, out: &mut Vec<String>) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for key in x.keys().chain(y.keys().filter(|k| !x.contains_key(*k))) {
                json_diff(
                    &format!("{path}.{key}"),
                    x.get(key).unwrap_or(&Value::Null),
                    y.get(key).unwrap_or(&Value::Null),
                    out,
                );
            }
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            for (i, (x, y)) in x.iter().zip(y).enumerate() {
                json_diff(&format!("{path}[{i}]"), x, y, out);
            }
        }
        _ if a != b => out.push(format!("{path}: battler {a} / showdown {b}")),
        _ => (),
    }
}

fn check(name: &str) {
    let fixture = load(name);
    let mut run = set_up(&fixture);
    for k in 0..=fixture.steps.len() {
        for (i, player) in ["p1", "p2"].iter().enumerate() {
            let expected = fixture.requests[k][i].clone();
            let actual = request_for(&mut run, player, i).unwrap_or(Value::Null);
            let mut diffs = Vec::new();
            json_diff("", &actual, &expected, &mut diffs);
            assert!(
                diffs.is_empty(),
                "{name}: request for {player} after {k} steps differs:\n{}",
                diffs.join("\n")
            );
        }
        if k < fixture.steps.len() {
            for (player, choice) in ["p1", "p2"].iter().zip(&fixture.steps[k]) {
                if !choice.is_empty() {
                    run.choose(player, choice).unwrap();
                }
            }
        }
    }
}

#[test]
fn basic_doubles_requests() {
    check("basic-doubles");
}

#[test]
fn status_condition_requests() {
    check("status-conditions");
}

#[test]
fn weather_field_side_requests() {
    check("weather-field-sides");
}

#[test]
fn switching_and_healing_requests() {
    check("switching-and-healing");
}

#[test]
fn mega_and_forms_requests() {
    check("mega-and-forms");
}

#[test]
fn volatiles_requests() {
    check("volatiles");
}

#[test]
fn items_requests() {
    check("items");
}

#[test]
fn illusion_requests() {
    check("illusion");
}

#[test]
fn win_requests() {
    check("win");
}

#[test]
fn simultaneous_faints_requests() {
    check("simultaneous-faints");
}
