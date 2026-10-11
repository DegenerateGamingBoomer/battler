//! Mapping battler effect names to Showdown's spelling.

use crate::names;

/// Showdown's status ID for a battler status name.
pub fn status_id(name: &str) -> String {
    match name {
        "Burn" => "brn",
        "Paralysis" => "par",
        "Sleep" => "slp",
        "Freeze" => "frz",
        "Poison" => "psn",
        "Bad Poison" => "tox",
        other => return names::to_id(other),
    }
    .to_owned()
}

/// Splits `kind:Name` into its parts (`status:Burn` → `("status", "Burn")`).
pub fn split_effect(effect: &str) -> (Option<&str>, &str) {
    match effect.split_once(':') {
        Some((kind, name))
            if matches!(
                kind,
                "status"
                    | "ability"
                    | "item"
                    | "move"
                    | "condition"
                    | "clause"
                    | "weather"
                    | "terrain"
                    | "volatile"
            ) =>
        {
            (Some(kind), name)
        }
        _ => (None, effect),
    }
}

/// The text Showdown prints after `[from]` for a battler effect.
pub fn from_label(effect: &str) -> String {
    let (kind, name) = split_effect(effect);
    match kind {
        Some("status") => status_id(name),
        Some("ability") => format!("ability: {}", names::ability_name(name)),
        Some("item") => format!("item: {}", names::item_name(name)),
        // Terrains are conditions in Showdown, not moves.
        Some("move")
            if matches!(
                name,
                "Grassy Terrain" | "Electric Terrain" | "Misty Terrain" | "Psychic Terrain"
            ) =>
        {
            name.to_owned()
        }
        Some("move") => format!("move: {}", names::move_name(name)),
        None if name == "Drain" => "drain".to_owned(),
        None if name == "Confusion" => "confusion".to_owned(),
        _ => name.to_owned(),
    }
}

/// Showdown's weather ID for a battler weather name.
pub fn weather_id(name: &str) -> String {
    match name {
        "Rain" => "RainDance",
        "Heavy Rain" => "PrimordialSea",
        "Harsh Sunlight" => "SunnyDay",
        "Extremely Harsh Sunlight" => "DesolateLand",
        "Sandstorm" => "Sandstorm",
        "Snow" => "Snowscape",
        "Hail" => "Hail",
        "Strong Winds" => "DeltaStream",
        other => other,
    }
    .to_owned()
}

/// How Showdown labels a side condition in `-sidestart` / `-sideend`.
pub fn side_condition_label(kind: &str, name: &str) -> String {
    names::label(kind, "move", name)
        .or_else(|| names::label(kind, "condition", name))
        .unwrap_or_else(|| format!("move: {}", names::move_name(name)))
}

/// How Showdown labels a field condition in `-fieldstart` / `-fieldend` / `-fieldactivate`.
pub fn field_label(kind: &str, effect_kind: &str, name: &str) -> String {
    names::label(kind, effect_kind, name)
        .unwrap_or_else(|| format!("{effect_kind}: {}", names::move_name(name)))
}

/// How Showdown labels a volatile effect in `-start`, `-end`, `-singleturn` and `-singlemove`.
pub fn volatile_label(kind: &str, effect_kind: &str, name: &str) -> String {
    names::label(kind, effect_kind, name)
        .or_else(|| names::label(kind, "condition", name))
        .unwrap_or_else(|| match effect_kind {
            "ability" => format!("ability: {}", names::ability_name(name)),
            "item" => format!("item: {}", names::item_name(name)),
            _ => format!("move: {}", names::move_name(name)),
        })
}

/// How Showdown labels the reason a Pokémon cannot move.
pub fn cant_label(name: &str) -> String {
    match name {
        "Flinch" => "flinch".to_owned(),
        other => other.to_owned(),
    }
}
