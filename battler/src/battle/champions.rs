//! Rule changes made by Pokémon Champions, mirroring Pokémon Showdown's `champions` mod.
//!
//! Data differences (move power, status odds, ability behavior, ...) are not here: they are applied
//! by the `battler-champions` data overlay. This module holds the rules that live in the engine
//! itself, behind [`ChampionsRules`] so that mainline behavior stays the default.

use alloc::{
    format,
    string::String,
};

use serde::{
    Deserialize,
    Serialize,
};

/// Maximum base PP of any move in Champions.
pub const MAX_BASE_PP: u8 = 20;

/// Engine rule changes made by Pokémon Champions.
///
/// Every toggle defaults to `false` (mainline behavior). [`ChampionsRules::all`] turns on
/// everything.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export))]
pub struct ChampionsRules {
    /// A Mon's EVs are stat points, and stats are `base + points + 20` (HP: `base + points + 75`),
    /// before the nature. IVs and level do not matter.
    #[serde(default)]
    pub stat_points: bool,

    /// Maximum PP is `(base PP / 5 + 1) * 4`, with no PP Ups.
    #[serde(default)]
    pub pp: bool,

    /// Public health is `floor(100 * hp / max_hp)` (minimum 1) with `y`/`r`/`g` color suffixes at
    /// exactly 20% and 50%, and `0` for a fainted Mon (the `fnt` suffix is a protocol detail).
    #[serde(default)]
    pub public_health: bool,

    /// Mega Evolutions are not reverted when the Mon faints.
    #[serde(default)]
    pub mega_persists_on_faint: bool,

    /// Attack counters (used by Rage Fist) reset whenever a Mon's volatile state is cleared
    /// (switch out, faint), and count every hit of a multi-hit move.
    #[serde(default)]
    pub reset_times_attacked: bool,

    /// Sheer Force does not suppress after-secondary-effect events (Eject Button, Berserk, Anger
    /// Shell, Pickpocket, ...).
    #[serde(default)]
    pub sheer_force_keeps_after_secondary_events: bool,

    /// A Parental Bond move that only hits once does not log a hit count.
    #[serde(default)]
    pub hide_single_parental_bond_hit_count: bool,

    /// A move that is stopped without a failure message (blocked by Protect, type immunity, ...)
    /// keeps its target in the log and is not marked `noanim`. Only moves that fail with a failure
    /// message are, as in Showdown, where `[still]` always accompanies `-fail`.
    #[serde(default)]
    pub stopped_moves_keep_animation: bool,

    /// `supereffective` and `resisted` log lines carry `by:1` or `by:2` (double effectiveness), as
    /// Showdown's Champions mod announces 4x and 0.25x.
    #[serde(default)]
    pub effectiveness_stages: bool,
}

impl ChampionsRules {
    /// All Champions rule changes.
    pub const fn all() -> Self {
        Self {
            stat_points: true,
            pp: true,
            public_health: true,
            mega_persists_on_faint: true,
            reset_times_attacked: true,
            sheer_force_keeps_after_secondary_events: true,
            hide_single_parental_bond_hit_count: true,
            stopped_moves_keep_animation: true,
            effectiveness_stages: true,
        }
    }
}

/// The maximum PP of a move with the given base PP under [`ChampionsRules::pp`].
pub fn max_pp(base_pp: u8) -> u8 {
    (base_pp.min(MAX_BASE_PP) / 5 + 1) * 4
}

/// Formats public health under [`ChampionsRules::public_health`].
pub fn public_health(hp: u16, max_hp: u16) -> (u32, String) {
    if hp == 0 || max_hp == 0 {
        return (0, String::from("0"));
    }
    let percent = ((100 * hp as u32) / max_hp as u32).max(1);
    let color = match percent {
        20 => Some(if hp as u32 * 5 > max_hp as u32 {
            'y'
        } else {
            'r'
        }),
        50 => Some(if hp as u32 * 2 > max_hp as u32 {
            'g'
        } else {
            'y'
        }),
        _ => None,
    };
    let text = match color {
        Some(color) => format!("{percent}/100{color}"),
        None => format!("{percent}/100"),
    };
    (percent, text)
}

#[cfg(test)]
mod champions_test {
    use super::{
        max_pp,
        public_health,
    };

    #[test]
    fn computes_max_pp() {
        assert_eq!(max_pp(5), 8);
        assert_eq!(max_pp(10), 12);
        assert_eq!(max_pp(15), 16);
        assert_eq!(max_pp(20), 20);
        assert_eq!(max_pp(40), 20);
    }

    #[test]
    fn formats_public_health() {
        assert_eq!(public_health(0, 100).1, "0");
        assert_eq!(public_health(1, 500).1, "1/100");
        assert_eq!(public_health(100, 100).1, "100/100");
        assert_eq!(public_health(99, 100).1, "99/100");
        assert_eq!(public_health(20, 100).1, "20/100r");
        assert_eq!(public_health(41, 200).1, "20/100y");
        assert_eq!(public_health(50, 100).1, "50/100y");
        assert_eq!(public_health(101, 200).1, "50/100g");
    }
}
