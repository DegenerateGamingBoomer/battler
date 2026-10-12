//! Translation between battler and the Pokémon Showdown wire formats.
//!
//! - [`team`]: Showdown packed and exported teams → battler [`battler::TeamData`].
//! - [`choice`]: Showdown choice strings (1-based) → battler choice strings (0-based).
//! - [`request`]: battler requests → Showdown `|request|` JSON.
//! - [`protocol`]: battler battle logs → Showdown protocol lines.
//!
//! Pokémon Showdown is the reference. Intentional differences from its output are listed, with a
//! reason, in `compat/allowlist.md`.

pub mod choice;
pub mod names;
pub mod protocol;
pub mod request;
pub mod species;
pub mod team;
pub mod transplant;
