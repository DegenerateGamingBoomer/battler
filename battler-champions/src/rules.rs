//! Champions rules that are not data: stat points, PP, public health and the format.
//!
//! The engine-side rules live in `battler` ([`battler::ChampionsRules`], enabled through
//! [`engine_options`]); the pure functions here and in [`battler::battle::champions`] are shared
//! with tests and validators and checked directly against Pokémon Showdown's `champions` mod.

pub use battler::battle::champions::{
    MAX_BASE_PP,
    max_pp,
    public_health,
};
use battler::{
    ChampionsRules,
    CoreBattleEngineOptions,
};

use crate::Regulation;

/// Maximum stat points in a single stat.
pub const MAX_STAT_POINTS_PER_STAT: u16 = 32;
/// Maximum stat points across all stats.
pub const MAX_STAT_POINTS_TOTAL: u16 = 66;
/// The only level used by the Champions VGC format.
pub const LEVEL: u8 = 50;

/// Stat points expressed as the EVs that make the mainline level-50 formula, with 31 IVs, produce
/// the Champions stat: `EV = max(8 * SP - 4, 0)`.
///
/// The engine does not need this (it reads stat points directly under
/// [`ChampionsRules::stat_points`]); it exists for tools that feed mainline-formula engines.
pub fn stat_points_to_evs(stat_points: u16) -> u16 {
    (8 * stat_points).saturating_sub(4)
}

/// Engine options for Champions battles: all engine rule changes on.
pub fn engine_options(base: CoreBattleEngineOptions) -> CoreBattleEngineOptions {
    CoreBattleEngineOptions {
        champions: ChampionsRules::all(),
        ..base
    }
}

/// Format rules for a Champions VGC doubles battle (mirrors the mod's `Flat Rules` for level 50
/// play).
///
/// Terastallization is deliberately absent, so it is not available. Species and move legality for
/// the regulation (`formats-data.ts`, `learnsets.ts`) is not part of these rules.
pub fn format_rules(_regulation: Regulation) -> Vec<String> {
    [
        "Obtainable",
        "Team Preview",
        "Species Clause",
        "Item Clause",
        "Adjust Level Down = 50",
        "Min Team Size = 6",
        "Picked Team Size = Auto",
        "EV Limit = 66",
        "Mega Evolution",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}
