mod common;

use common::*;

fn check(name: &str) {
    let fixture = load(name);
    let mut run = set_up(&fixture);
    run.play(&fixture);
    let translated = run.translated().unwrap_or_else(|e| panic!("{name}: {e:#}"));
    if let Some(diff) = diff(
        &apply_allowlist(&fixture.log),
        &apply_allowlist(&translated),
    ) {
        panic!("{name}: translated battler log differs from Showdown:\n{diff}");
    }
}

#[test]
fn basic_doubles() {
    check("basic-doubles");
}

#[test]
fn status_conditions() {
    check("status-conditions");
}

#[test]
fn boosts() {
    check("boosts");
}

#[test]
fn weather_field_sides() {
    check("weather-field-sides");
}

#[test]
fn protect_and_misses() {
    check("protect-and-misses");
}

#[test]
fn switching_and_healing() {
    check("switching-and-healing");
}

#[test]
fn mega_and_forms() {
    check("mega-and-forms");
}

#[test]
fn volatiles() {
    check("volatiles");
}

#[test]
fn items() {
    check("items");
}

#[test]
fn illusion() {
    check("illusion");
}

#[test]
fn win() {
    check("win");
}

#[test]
fn simultaneous_faints() {
    check("simultaneous-faints");
}

/// Showdown never reaches `|tie` in gen 9 by fainting (the last Pokémon to faint wins), so there is
/// no reference battle for it; the line itself is checked directly.
#[test]
fn tie_line() {
    use battler_showdown_compat::protocol::{
        Translator,
        TranslatorConfig,
    };
    let lines = Translator::translate_all(
        TranslatorConfig {
            players: vec!["p1".to_owned(), "p2".to_owned()],
            teams: Vec::new(),
        },
        ["residual", "tie"],
    )
    .unwrap();
    // The battle is over, so the unfinished residual phase prints no `|upkeep`.
    assert_eq!(lines, ["|tie"]);
}
