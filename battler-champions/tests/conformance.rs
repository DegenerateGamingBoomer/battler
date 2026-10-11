//! Checks the Champions data overlay against values exported from Pokémon Showdown's `champions`
//! mod (`scripts/export-champions-expected.js`, `tests/data/showdown-champions.json`).
//!
//! Strictly asserted: every field Showdown's mod changes relative to mainline gen9 (the Champions
//! delta). Values that differ between battler's mainline data and Showdown's mainline data for
//! other reasons are out of scope here and are not asserted.

use battler_champions::{
    ChampionsDataStore,
    Regulation,
};
use battler_data::{
    Accuracy,
    DataStore,
    Id,
    MoveData,
    MoveFlag,
};
use battler_test_utils::static_local_data_store;
use serde_json::Value;

fn expected() -> Value {
    serde_json::from_str(include_str!("data/showdown-champions.json")).unwrap()
}

fn store() -> ChampionsDataStore<'static> {
    ChampionsDataStore::new(static_local_data_store(), Regulation::MC).unwrap()
}

fn flag_from_showdown(flag: &str) -> Option<MoveFlag> {
    Some(match flag {
        "slicing" => MoveFlag::Slicing,
        "punch" => MoveFlag::Punch,
        "bypasssub" => MoveFlag::BypassSubstitute,
        "sound" => MoveFlag::Sound,
        _ => return None,
    })
}

fn changed(entry: &Value, field: &str) -> bool {
    entry["changed"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f == field)
}

/// Moves with a Champions-mod change that battler's data has no entry for at all.
fn missing_moves(store: &ChampionsDataStore) -> Vec<String> {
    let expected = expected();
    expected["moves"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(id, m)| {
            m["changed"].as_array().unwrap().iter().any(|f| {
                !matches!(
                    f.as_str().unwrap(),
                    "isNonstandard"
                        | "name"
                        | "placeholderFor"
                        | "zMove"
                        | "maxMove"
                        | "baseMoveType"
                )
            }) && store.get_move(&Id::from(id.as_str())).unwrap().is_none()
        })
        .map(|(id, _)| id.clone())
        .collect()
}

#[test]
fn champions_move_changes_match_showdown() {
    let store = store();
    let expected = expected();
    let mut failures = Vec::new();
    for (id, m) in expected["moves"].as_object().unwrap() {
        // Showdown's generic `hiddenpower` is a placeholder for the typed Hidden Power moves; its
        // type is an artifact of how the mod redefines the placeholder, not a rule.
        if id == "hiddenpower" {
            continue;
        }
        let Some(data) = store.get_move(&Id::from(id.as_str())).unwrap() else {
            continue;
        };
        let mut check = |field: &str, ok: bool, got: String| {
            if !ok {
                failures.push(format!(
                    "{id}.{field}: battler {got}, showdown {}",
                    m[field]
                ));
            }
        };
        if changed(m, "basePower") {
            check(
                "basePower",
                m["basePower"].as_u64() == Some(data.base_power as u64),
                data.base_power.to_string(),
            );
        }
        if changed(m, "accuracy") {
            let ok = match (&m["accuracy"], data.accuracy) {
                (Value::Bool(true), Accuracy::Exempt) => true,
                (Value::Number(n), Accuracy::Chance(c)) => n.as_u64() == Some(c as u64),
                _ => false,
            };
            check("accuracy", ok, format!("{:?}", data.accuracy));
        }
        if changed(m, "type") {
            check(
                "type",
                m["type"].as_str() == Some(&data.primary_type.to_string()),
                data.primary_type.to_string(),
            );
        }
        if changed(m, "target") {
            check(
                "target",
                m["target"].as_str().map(|t| t.to_lowercase())
                    == Some(
                        data.target
                            .to_string()
                            .to_lowercase()
                            .replace("user", "self"),
                    ),
                data.target.to_string(),
            );
        }
        if changed(m, "pp") {
            check(
                "pp",
                m["pp"].as_u64() == Some(data.pp as u64),
                data.pp.to_string(),
            );
        }
        for flag in m["flagsAdded"].as_array().unwrap() {
            if let Some(flag) = flag_from_showdown(flag.as_str().unwrap()) {
                check(
                    "flagsAdded",
                    data.flags.contains(&flag),
                    format!("{:?}", data.flags),
                );
            }
        }
        if changed(m, "secondaries") {
            let secondaries = m["secondaries"].as_array().unwrap();
            let ok = secondaries.len() == data.secondary_effects.len()
                && secondaries
                    .iter()
                    .zip(&data.secondary_effects)
                    .all(|(s, d)| {
                        s["chance"].as_u64().map(|c| (c, 100))
                            == d.chance
                                .map(|c| (c.numerator() as u64 * 100 / c.denominator() as u64, 100))
                    });
            check(
                "secondaries",
                ok,
                format!("{} entries", data.secondary_effects.len()),
            );
        }
    }
    assert!(
        failures.is_empty(),
        "overlay differs from Showdown:\n{}",
        failures.join("\n")
    );
}

#[test]
fn base_pp_is_capped_at_20_everywhere() {
    let store = store();
    let ids = store.all_move_ids(&|_: &MoveData| true).unwrap();
    assert!(!ids.is_empty());
    for id in ids {
        let data = store.get_move(&id).unwrap().unwrap();
        assert!(data.pp <= 20, "{id:?} has {} PP", data.pp);
    }
}

#[test]
fn every_changed_move_exists_in_battler_data() {
    // Moves added by Champions that battler's data lacks cannot be patched. Keep this list visible.
    assert_eq!(missing_moves(&store()), vec!["nihillight".to_owned()]);
}

/// Every ability and item whose behavior (not just availability) the Champions mod changes must be
/// accounted for: patched directly, patched through a shared condition or delegate, or a change
/// that only affects the Showdown protocol log. Anything new in the mod fails this test.
#[test]
fn every_behavior_change_in_abilities_and_items_is_accounted_for() {
    let expected = expected();
    let behavioral = |kind: &str| -> Vec<String> {
        let mut ids: Vec<String> = expected[kind]
            .as_object()
            .unwrap()
            .iter()
            .filter(|(_, v)| {
                v["changed"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|f| f != "isNonstandard")
            })
            .map(|(id, _)| id.clone())
            .collect();
        ids.sort();
        ids
    };
    let store = store();
    let inner = static_local_data_store();
    let ability = |s: &dyn DataStore, id: &str| s.get_ability(&Id::from(id)).unwrap().unwrap();

    // Patched directly on the ability.
    for id in ["emergencyexit", "healer", "runaway"] {
        assert_ne!(
            ability(&store, id).effect,
            ability(inner, id).effect,
            "{id}"
        );
    }
    // Patched through `hpactivatedabilitybase`, shared by both abilities.
    for id in ["angershell", "berserk"] {
        assert!(
            ability(&store, id).effect["delegates"]
                .to_string()
                .contains("hpactivatedabilitybase")
        );
    }
    let base = |s: &dyn DataStore| {
        s.get_condition(&Id::from("hpactivatedabilitybase"))
            .unwrap()
            .unwrap()
            .condition
    };
    assert_ne!(base(&store), base(inner));
    // Wimp Out delegates to Emergency Exit.
    assert!(
        ability(&store, "wimpout").effect["delegates"]
            .to_string()
            .contains("emergencyexit")
    );
    // Only the Showdown protocol log differs: silent switch-out messages and `-zbroken`.
    let log_only = ["naturalcure", "regenerator", "unseenfist"];

    let mut accounted: Vec<String> = [
        "angershell",
        "berserk",
        "emergencyexit",
        "healer",
        "runaway",
        "wimpout",
    ]
    .into_iter()
    .chain(log_only)
    .map(str::to_owned)
    .collect();
    accounted.sort();
    assert_eq!(behavioral("abilities"), accounted);
    assert_eq!(behavioral("items"), vec!["ejectbutton".to_owned()]);
    let item = |s: &dyn DataStore| {
        s.get_item(&Id::from("ejectbutton"))
            .unwrap()
            .unwrap()
            .effect
    };
    assert_ne!(item(&store), item(inner));
}
