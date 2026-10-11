//! battler battle log → Showdown protocol lines.
//!
//! [`Translator`] is stateful: Showdown lines need information battler's log only states once
//! (which Pokémon is in which slot, statuses, species details), and some Showdown tags annotate a
//! line that was already printed (`[miss]` on the `move` line).

pub(crate) mod effects;
mod line;

use ahash::HashMap;
use anyhow::{
    Context,
    Result,
    bail,
};
use effects::{
    from_label,
    split_effect,
    status_id,
};
pub use line::{
    Line,
    MonRef,
};

use crate::{
    names,
    team::ConvertedTeam,
};

const SLOTS: [char; 6] = ['a', 'b', 'c', 'd', 'e', 'f'];

/// Which log a line came from, when the log is split.
#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Whole,
    Private,
    Public,
}

#[derive(Default, Clone)]
struct MonState {
    status: Option<String>,
}

/// Configuration for a [`Translator`].
#[derive(Default, Clone)]
pub struct TranslatorConfig {
    /// battler player IDs in Showdown order: the first is `p1`, the second `p2`.
    pub players: Vec<String>,
    /// The converted teams, in the same order, to recover Showdown's species spelling.
    pub teams: Vec<ConvertedTeam>,
}

/// Translates a battler battle log into Showdown protocol lines.
pub struct Translator {
    config: TranslatorConfig,
    /// battler player ID → Showdown player ID.
    players: HashMap<String, String>,
    /// Side index → Showdown player ID.
    sides: HashMap<usize, String>,
    /// Player name by Showdown player ID, for `|win|`.
    player_names: HashMap<String, String>,
    mons: HashMap<(String, String), MonState>,
    out: Vec<String>,
    last_move: Option<usize>,
    residual: bool,
    split: Option<(String, Vec<Line>)>,
    /// Team sizes by Showdown player ID, in order of first appearance; emitted at the battle
    /// start.
    team_sizes: Vec<(String, String)>,
    last_move_user: Option<MonRef>,
    last_move_name: Option<String>,
    last_move_target: Option<MonRef>,
    /// The (Pokémon, ability) of the ability-boost group being printed.
    boost_group: Option<(String, String)>,
    /// The Pokémon and ability of the last `formechange` line, whose ability activation Showdown
    /// omits.
    last_formechange: Option<(MonRef, String)>,
    /// The user and name of the last pivot move, until its replacement switches in.
    pivot: Option<(MonRef, String)>,
    /// A Pokémon being revived, until its HP line.
    reviving: Option<MonRef>,
    revive_from: Option<String>,
    /// The item exchange of the Trick or Switcheroo being printed.
    swap: Option<ItemSwap>,
    /// Who planted the Leech Seed on a Pokémon.
    seeds: HashMap<MonRef, MonRef>,
    /// An `-anim` line Showdown prints after the boosts a move stole.
    pending_anim: Option<String>,
}

/// The pieces of an item exchange, printed together once the exchange ends.
#[derive(Default)]
struct ItemSwap {
    move_label: String,
    /// Items taken from a Pokémon.
    taken: Vec<(MonRef, String)>,
    /// Items given to a Pokémon.
    given: Vec<(MonRef, String)>,
}

impl Translator {
    pub fn new(config: TranslatorConfig) -> Self {
        let players = config
            .players
            .iter()
            .enumerate()
            .map(|(i, id)| (id.clone(), format!("p{}", i + 1)))
            .collect();
        Self {
            config,
            players,
            sides: HashMap::default(),
            player_names: HashMap::default(),
            mons: HashMap::default(),
            out: Vec::new(),
            last_move: None,
            residual: false,
            split: None,
            team_sizes: Vec::new(),
            last_move_user: None,
            last_move_name: None,
            last_move_target: None,
            boost_group: None,
            last_formechange: None,
            pivot: None,
            reviving: None,
            revive_from: None,
            swap: None,
            seeds: HashMap::default(),
            pending_anim: None,
        }
    }

    /// The translated lines so far.
    pub fn lines(&self) -> &[String] {
        &self.out
    }

    /// Translates a whole log.
    pub fn translate_all<'a>(
        config: TranslatorConfig,
        log: impl IntoIterator<Item = &'a str>,
    ) -> Result<Vec<String>> {
        let mut translator = Self::new(config);
        for line in log {
            translator
                .feed(line)
                .with_context(|| format!("while translating '{line}'"))?;
        }
        translator.flush()?;
        Ok(translator.out)
    }

    /// Call when the battle pauses to wait for a request (or the log ends). Showdown prints
    /// `|upkeep` at the end of the residual phase, before it waits for replacement switches, but
    /// battler only marks the start of the phase.
    pub fn flush(&mut self) -> Result<()> {
        self.finish_swap()?;
        self.finish_anim();
        if std::mem::take(&mut self.residual) {
            self.out.push("|upkeep".to_owned());
        }
        Ok(())
    }

    /// Prints a finished item exchange the way Showdown does: the user announces it, then the
    /// target and the user each receive an item (or silently lose theirs).
    fn finish_swap(&mut self) -> Result<()> {
        let Some(swap) = self.swap.take() else {
            return Ok(());
        };
        let user = self
            .last_move_user
            .clone()
            .context("item swap without a move")?;
        let target = swap
            .given
            .iter()
            .chain(&swap.taken)
            .map(|(mon, _)| mon)
            .find(|mon| **mon != user)
            .cloned()
            .or_else(|| self.last_move_target.clone())
            .context("item swap without a target")?;
        let item_of = |items: &[(MonRef, String)], mon: &MonRef| {
            items.iter().find(|(m, _)| m == mon).map(|(_, i)| i.clone())
        };
        let label = &swap.move_label;
        self.out.push(format!(
            "|-activate|{}|move: {label}|[of] {}",
            self.ident(&user)?,
            self.ident(&target)?
        ));
        for mon in [&target, &user] {
            match item_of(&swap.given, mon) {
                Some(item) => self.out.push(format!(
                    "|-item|{}|{item}|[from] move: {label}",
                    self.ident(mon)?
                )),
                None => {
                    // Nothing came back: the Pokémon silently loses what it held.
                    let item = item_of(&swap.taken, mon).unwrap_or_default();
                    self.out.push(format!(
                        "|-enditem|{}|{item}|[silent]|[from] move: {label}",
                        self.ident(mon)?
                    ));
                }
            }
        }
        Ok(())
    }

    fn player(&self, battler_player: &str) -> Result<&str> {
        self.players
            .get(battler_player)
            .map(String::as_str)
            .with_context(|| format!("unknown player '{battler_player}'"))
    }

    fn finish_anim(&mut self) {
        if let Some(anim) = self.pending_anim.take() {
            self.out.push(anim);
        }
    }

    /// Whether a line belongs to an item exchange.
    fn is_swap_line(line: &Line) -> bool {
        matches!(line.title.as_str(), "item" | "itemstart" | "itemend")
            && line
                .get("from")
                .is_some_and(|f| matches!(f, "move:Trick" | "move:Switcheroo"))
    }

    /// `p1a: Name` (active) or `p1: Name` (inactive).
    fn ident(&self, mon: &MonRef) -> Result<String> {
        let player = self.player(&mon.player)?;
        Ok(match mon.position {
            Some(position) => format!("{player}{}: {}", SLOTS[position - 1], mon.name),
            None => format!("{player}: {}", mon.name),
        })
    }

    fn mon_ident(&self, value: &str) -> Result<String> {
        self.ident(&MonRef::parse(value)?)
    }

    fn state(&mut self, mon: &MonRef) -> &mut MonState {
        self.mons
            .entry((mon.player.clone(), mon.name.clone()))
            .or_default()
    }

    /// The Showdown condition string: health plus status, `0 fnt` when fainted.
    fn condition(&mut self, mon: &MonRef, health: &str) -> String {
        if health == "0" {
            return "0 fnt".to_owned();
        }
        match self.state(mon).status.clone() {
            Some(status) => format!("{health} {status}"),
            None => health.to_owned(),
        }
    }

    fn details(&self, player: &str, name: &str, line: &Line) -> String {
        let species = line.get("species").unwrap_or_default();
        // Showdown's spelling of the species, which can differ for cosmetic formes.
        let mut showdown = names::species_name(species);
        if let Some(index) = self
            .players
            .get(player)
            .and_then(|p| p[1..].parse::<usize>().ok())
        {
            if let Some(team) = self.config.teams.get(index - 1) {
                for (member, resolved) in team.team.members.iter().zip(&team.species) {
                    if member.name == name && resolved.name == species {
                        showdown = resolved.showdown_name.clone();
                    }
                }
            }
        }
        // Showdown tells the genders of Meowstic's Mega formes apart.
        if species == "Meowstic-Mega" {
            showdown = match line.get("gender") {
                Some("F") => "Meowstic-F-Mega".to_owned(),
                _ => "Meowstic-M-Mega".to_owned(),
            };
        }
        let mut details = showdown;
        let level = line
            .get("level")
            .and_then(|l| l.parse::<u32>().ok())
            .unwrap_or(100);
        if level != 100 {
            details.push_str(&format!(", L{level}"));
        }
        match line.get("gender") {
            Some("M") => details.push_str(", M"),
            Some("F") => details.push_str(", F"),
            _ => (),
        }
        if line.has("shiny") {
            details.push_str(", shiny");
        }
        details
    }

    /// Feeds one battler log line.
    pub fn feed(&mut self, raw: &str) -> Result<()> {
        let line = Line::parse(raw)?;
        if self.swap.is_some() && !Self::is_swap_line(&line) {
            self.finish_swap()?;
        }
        if self.pending_anim.is_some() && line.title != "boost" {
            self.finish_anim();
        }
        if line.title == "split" {
            let side: usize = line.get("side").context("split without side")?.parse()?;
            let player = self
                .sides
                .get(&side)
                .cloned()
                .with_context(|| format!("split for unknown side {side}"))?;
            self.split = Some((player, Vec::new()));
            return Ok(());
        }
        if let Some((player, pending)) = &mut self.split {
            pending.push(line);
            if pending.len() < 2 {
                return Ok(());
            }
            let (player, pending) = (player.clone(), std::mem::take(pending));
            self.split = None;
            self.close_residual(&pending[0].title);
            if matches!(pending[0].title.as_str(), "specieschange" | "replace") {
                // Showdown announces a permanent forme change (or an Illusion reveal) once, to
                // everyone, with no HP.
                let lines = self.translate(&pending[1], View::Public)?;
                self.out
                    .extend(lines.into_iter().map(|l| match l.rsplit_once('|') {
                        Some((head, _hp))
                            if l.starts_with("|detailschange|") || l.starts_with("|replace|") =>
                        {
                            head.to_owned()
                        }
                        _ => l,
                    }));
                return Ok(());
            }
            let private = self.translate(&pending[0], View::Private)?;
            let public = self.translate(&pending[1], View::Public)?;
            if private.len() != public.len() {
                bail!(
                    "split lines translated to {} and {} lines",
                    private.len(),
                    public.len()
                );
            }
            for (private, public) in private.into_iter().zip(public) {
                self.out.push(format!("|split|{player}"));
                self.out.push(private);
                self.out.push(public);
            }
            if pending[0].title == "switch" {
                self.pivot = None;
            }
            return Ok(());
        }
        self.close_residual(&line.title);
        let lines = self.translate(&line, View::Whole)?;
        self.out.extend(lines);
        Ok(())
    }

    /// Showdown prints `|upkeep` at the end of the residual phase, before replacement switch-ins
    /// and the next turn. battler marks only the start of the phase.
    fn close_residual(&mut self, next_title: &str) {
        if self.residual && matches!(next_title, "switch" | "drag" | "turn") {
            self.residual = false;
            self.out.push("|upkeep".to_owned());
        }
    }

    /// `|[from] X|[of] Y` tags for a line's `from` and `of` attributes.
    fn from_of(&self, line: &Line) -> Result<String> {
        let mut text = String::new();
        if let Some(from) = line.get("from") {
            text.push_str(&format!("|[from] {}", from_label(from)));
        }
        if let Some(of) = line.get("of") {
            text.push_str(&format!("|[of] {}", self.mon_ident(of)?));
        }
        Ok(text)
    }

    fn mon_of(line: &Line, key: &str) -> Result<MonRef> {
        MonRef::parse(
            line.get(key)
                .with_context(|| format!("'{}' line is missing '{key}'", line.title))?,
        )
    }

    fn side_label(&self, side: &str) -> Result<String> {
        let side: usize = side.parse()?;
        let player = self.sides.get(&side).context("unknown side")?;
        Ok(format!(
            "{player}: {}",
            self.player_names.get(player).unwrap_or(player)
        ))
    }

    /// Marks the last `move` line `[still]` (blank target) if it belongs to `user` and has no other
    /// event after it.
    fn mark_last_move_still(&mut self, user: &MonRef) -> Result<()> {
        if self.last_move_user.as_ref() != Some(user) {
            return Ok(());
        }
        let Some(index) = self.last_move else {
            return Ok(());
        };
        if index + 1 != self.out.len() {
            return Ok(());
        }
        if let Some(line) = self.out.get_mut(index) {
            if line.contains("|[still]") || line.contains("|[notarget]") {
                return Ok(());
            }
            let mut parts: Vec<String> = line.split('|').map(str::to_owned).collect();
            if parts.len() >= 5 {
                parts[4] = String::new();
                parts.push("[still]".to_owned());
                *line = parts.join("|");
            }
        }
        Ok(())
    }

    fn last_move_line(&self) -> Option<&String> {
        self.last_move.and_then(|i| self.out.get(i))
    }

    /// Undoes `[still]` and the blanked target on the last `move` line.
    fn restore_last_move_animation(&mut self, target: Option<&MonRef>) -> Result<()> {
        let (Some(index), Some(user)) = (self.last_move, self.last_move_user.clone()) else {
            return Ok(());
        };
        let ident = self.ident(target.unwrap_or(&user))?;
        if let Some(line) = self.out.get_mut(index) {
            // `...|Name||[still]` → `...|Name|<user>`
            if let Some(base) = line.strip_suffix("||[still]") {
                *line = format!("{base}|{ident}");
            }
        }
        Ok(())
    }

    fn translate(&mut self, line: &Line, view: View) -> Result<Vec<String>> {
        let _ = view;
        let title = line.title.as_str();
        if !matches!(title, "boost" | "unboost") {
            self.boost_group = None;
        }
        let mut out = Vec::new();
        match title {
            "side"
            | "info"
            | "continue"
            | "time"
            | "teampreviewstart"
            | "mon"
            | "teampreview"
            | "switchout"
            | "residual_debug"
            | "addsidecondition"
            | "removesidecondition"
            | "addslotcondition"
            | "removeslotcondition"
            | "addpseudoweather"
            | "removepseudoweather"
            | "addvolatile"
            | "removevolatile" => (),
            "player" => {
                let battler_id = line.get("id").context("player id")?;
                let shown = self.player(battler_id)?.to_owned();
                let side: usize = line.get("side").context("player side")?.parse()?;
                self.sides.insert(side, shown.clone());
                if let Some(name) = line.get("name") {
                    self.player_names.insert(shown, name.to_owned());
                }
            }
            "teamsize" => {
                let player = self
                    .player(line.get("player").context("player")?)?
                    .to_owned();
                let size = line.get("size").context("size")?.to_owned();
                match self.team_sizes.iter_mut().find(|(p, _)| *p == player) {
                    Some(entry) => entry.1 = size,
                    None => self.team_sizes.push((player, size)),
                }
            }
            "battlestart" => {
                for (player, size) in std::mem::take(&mut self.team_sizes) {
                    out.push(format!("|teamsize|{player}|{size}"));
                }
                out.push("|start".to_owned());
            }
            "switch" | "drag" | "replace" => {
                let player = line.get("player").context("player")?;
                let position: usize = line.get("position").context("position")?.parse()?;
                let name = line.get("name").context("name")?;
                let mon = MonRef {
                    name: name.to_owned(),
                    player: player.to_owned(),
                    position: Some(position),
                };
                if let Some(status) = line.get("status") {
                    self.state(&mon).status = Some(status_id(status));
                }
                let details = self.details(player, name, line);
                let condition = self.condition(&mon, line.get("health").context("health")?);
                let mut tags = self.from_of(line)?;
                // A switch right after its own pivot move names the move.
                if tags.is_empty()
                    && title == "switch"
                    && self.pivot.as_ref().is_some_and(|(user, _)| {
                        user.player == mon.player && user.position == mon.position
                    })
                {
                    let (_, move_name) = self.pivot.clone().unwrap();
                    tags = format!("|[from] {move_name}");
                }
                out.push(format!(
                    "|{}|{}|{details}|{condition}{tags}",
                    title,
                    self.ident(&mon)?
                ));
            }
            "specieschange" => {
                // Permanent forme change (Mega Evolution, ...).
                let player = line.get("player").context("player")?;
                let position: usize = line.get("position").context("position")?.parse()?;
                let name = line.get("name").context("name")?;
                let mon = MonRef {
                    name: name.to_owned(),
                    player: player.to_owned(),
                    position: Some(position),
                };
                let details = self.details(player, name, line);
                let condition = self.condition(&mon, line.get("health").context("health")?);
                out.push(format!(
                    "|detailschange|{}|{details}|{condition}",
                    self.ident(&mon)?
                ));
            }
            "mega" => {
                let mon = Self::mon_of(line, "mon")?;
                let species = names::species_name(line.get("species").context("species")?);
                let stone = line
                    .get("from")
                    .map(|f| names::item_name(split_effect(f).1))
                    .unwrap_or_default();
                // `-mega` names the base species.
                let base = species.split("-Mega").next().unwrap_or(&species).to_owned();
                out.push(format!("|-mega|{}|{base}|{stone}", self.ident(&mon)?));
            }
            "formechange" => {
                let mon = Self::mon_of(line, "mon")?;
                let species = names::species_name(line.get("species").context("species")?);
                let ability = line.get("from").and_then(|f| match split_effect(f) {
                    (Some("ability"), name) => Some(names::ability_name(name)),
                    _ => None,
                });
                // Showdown only names the ability for a few formes; the message tag comes with it.
                let tags = match ability.as_deref() {
                    // Printed with no message or ability.
                    Some(
                        "Stance Change" | "Schooling" | "Shields Down" | "Zen Mode"
                        | "Power Construct",
                    )
                    | None => String::new(),
                    Some(ability @ ("Flower Gift" | "Forecast")) => {
                        format!("[msg]|[from] ability: {ability}")
                    }
                    Some(ability) => format!("|[from] ability: {ability}"),
                };
                out.push(format!(
                    "|-formechange|{}|{species}|{tags}",
                    self.ident(&mon)?
                ));
                self.last_formechange = Some((mon, ability.unwrap_or_default()));
            }
            "move" | "animatemove" => {
                let user = Self::mon_of(line, "mon")?;
                let name = names::move_name(line.get("name").context("move name")?);
                let still = line.has("noanim");
                let target = match line.get("target") {
                    Some(target) if !still => self.mon_ident(target)?,
                    Some(_) => String::new(),
                    None if still => String::new(),
                    None => self.ident(&user)?,
                };
                let mut text = format!("|move|{}|{name}|{target}", self.ident(&user)?);
                if let Some(from) = line.get("from") {
                    text.push_str(&format!("|[from] {}", from_label(from)));
                }
                for (key, value) in &line.parts {
                    match (key.as_str(), value) {
                        // A move with no target to hit is not `[still]`.
                        ("noanim", None) if !line.has("notarget") => text.push_str("|[still]"),
                        ("notarget", None) => text.push_str("|[notarget]"),
                        ("spread", Some(spread)) => {
                            let slots = spread
                                .split(';')
                                .map(|m| {
                                    let m = MonRef::parse(m)?;
                                    let ident = self.ident(&m)?;
                                    Ok(ident.split(':').next().unwrap_or_default().to_owned())
                                })
                                .collect::<Result<Vec<_>>>()?;
                            text.push_str(&format!("|[spread] {}", slots.join(",")));
                        }
                        _ => (),
                    }
                }
                self.pivot = matches!(
                    name.as_str(),
                    "U-turn"
                        | "Volt Switch"
                        | "Flip Turn"
                        | "Parting Shot"
                        | "Baton Pass"
                        | "Teleport"
                        | "Chilly Reception"
                        | "Shed Tail"
                )
                .then(|| (user.clone(), name.clone()));
                self.last_move = Some(self.out.len());
                self.last_move_user = Some(user);
                self.last_move_name = Some(name);
                self.last_move_target = line.get("target").map(MonRef::parse).transpose()?;
                out.push(text);
            }
            "miss" => {
                let target = Self::mon_of(line, "mon")?;
                let user = self.last_move_user.clone().context("miss without a move")?;
                // A missed move is not `[still]`, and it names the target that was missed.
                if !self
                    .last_move_line()
                    .is_some_and(|l| l.contains("|[spread]"))
                {
                    self.restore_last_move_animation(Some(&target))?;
                    if let Some(index) = self.last_move {
                        if let Some(moves_line) = self.out.get_mut(index) {
                            match moves_line.strip_suffix("|[miss]") {
                                // A second miss for the same move means it was a spread move that
                                // missed everything; Showdown then lists no hit slots.
                                Some(base) => *moves_line = format!("{base}|[spread] "),
                                None => moves_line.push_str("|[miss]"),
                            }
                        }
                    }
                }
                out.push(format!(
                    "|-miss|{}|{}",
                    self.ident(&user)?,
                    self.ident(&target)?
                ));
            }
            "damage" | "heal" => {
                let target = Self::mon_of(line, "mon")?;
                let condition = self.condition(&target, line.get("health").context("health")?);
                let tags = if line.get("from") == Some("move:Leech Seed") {
                    // Showdown blames the planter for the damage and keeps the healing quiet.
                    if title == "damage" {
                        let seeder = self
                            .seeds
                            .get(&target)
                            .map(|s| self.ident(s))
                            .transpose()?
                            .map(|s| format!("|[of] {s}"))
                            .unwrap_or_default();
                        format!("|[from] Leech Seed{seeder}")
                    } else {
                        "|[silent]".to_owned()
                    }
                } else {
                    self.from_of(line)?
                };
                out.push(format!(
                    "|-{title}|{}|{condition}{tags}",
                    self.ident(&target)?
                ));
            }
            "sethp" => {
                let target = Self::mon_of(line, "mon")?;
                let condition = self.condition(&target, line.get("health").context("health")?);
                // The HP a revived Pokémon comes back with is a heal in Showdown.
                if self.reviving.as_ref() == Some(&target) {
                    let reviving = self.reviving.clone().unwrap();
                    let move_label = self.revive_from.clone().unwrap_or_default();
                    out.push(format!(
                        "|-heal|{}|{condition}{move_label}",
                        self.ident(&reviving)?
                    ));
                } else {
                    let mut tags = self.from_of(line)?;
                    // Pain Split names itself and keeps the target's line quiet.
                    if tags.is_empty() && self.last_move_name.as_deref() == Some("Pain Split") {
                        tags = "|[from] move: Pain Split".to_owned();
                        if self.last_move_user.as_ref() != Some(&target) {
                            tags.push_str("|[silent]");
                        }
                    }
                    out.push(format!(
                        "|-sethp|{}|{condition}{tags}",
                        self.ident(&target)?
                    ));
                }
            }
            "faint" => {
                let target = Self::mon_of(line, "mon")?;
                self.state(&target).status = None;
                out.push(format!("|faint|{}", self.ident(&target)?));
            }
            "crit" => {
                let target = Self::mon_of(line, "mon")?;
                out.push(format!("|-crit|{}", self.ident(&target)?));
            }
            "supereffective" | "resisted" => {
                let target = Self::mon_of(line, "mon")?;
                // Showdown's Champions mod announces 4x and 0.25x with a trailing 1 or 2.
                let stages = line
                    .get("by")
                    .map(|by| format!("|{by}"))
                    .unwrap_or_default();
                out.push(format!("|-{title}|{}{stages}", self.ident(&target)?));
            }
            "immune" => {
                let target = Self::mon_of(line, "mon")?;
                out.push(format!(
                    "|-immune|{}{}",
                    self.ident(&target)?,
                    self.from_of(line)?
                ));
            }
            "fail" => {
                let target = Self::mon_of(line, "mon")?;
                let mut text = format!("|-fail|{}", self.ident(&target)?);
                let what = line.get("what");
                if let Some(what) = what {
                    text.push_str(&format!("|{what}"));
                    if what == "unboost" {
                        if let Some(boosts) = line.get("boosts") {
                            text.push_str(&format!("|{boosts}"));
                        }
                    }
                }
                // Showdown's engine marks a failed move `[still]`; moves that print their own
                // failure (Rest) keep their animation.
                if what != Some("unboost")
                    && !matches!(self.last_move_name.as_deref(), Some("Rest"))
                {
                    self.mark_last_move_still(&target)?;
                } else if what == Some("heal") {
                    self.restore_last_move_animation(None)?;
                }
                text.push_str(&self.from_of(line)?);
                if what == Some("unboost") && line.get("from").is_some() && line.get("of").is_none()
                {
                    text.push_str(&format!("|[of] {}", self.ident(&target)?));
                }
                out.push(text);
            }
            "status" => {
                let target = Self::mon_of(line, "mon")?;
                let status = status_id(line.get("status").context("status")?);
                self.state(&target).status = Some(status.clone());
                // Yawn's sleep is not attributed to the move.
                let tags = if line.get("from") == Some("move:Yawn") {
                    String::new()
                } else {
                    self.from_of(line)?
                };
                out.push(format!("|-status|{}|{status}{tags}", self.ident(&target)?));
            }
            "typechange" => {
                let mon = Self::mon_of(line, "mon")?;
                let mut types = line.get("types").context("types")?.to_owned();
                // Moves that remove a type leave an unknown type in its place.
                if !types.contains('/')
                    && matches!(line.get("from"), Some("move:Double Shock" | "move:Burn Up"))
                {
                    types = format!("???/{types}");
                }
                out.push(format!(
                    "|-start|{}|typechange|{types}{}",
                    self.ident(&mon)?,
                    self.from_of(line)?
                ));
            }
            "revive" => {
                self.reviving = Some(Self::mon_of(line, "mon")?);
                self.revive_from = line
                    .get("from")
                    .map(|f| format!("|[from] {}", from_label(f)));
            }
            "curestatus" => {
                let target = Self::mon_of(line, "mon")?;
                self.state(&target).status = None;
                let status = status_id(line.get("status").context("status")?);
                // Team cures announce each cured Pokémon with `[msg]`.
                let msg = if line.get("from").is_none()
                    && (matches!(
                        self.last_move_name.as_deref(),
                        Some("Heal Bell" | "Aromatherapy")
                    ) || matches!(status.as_str(), "slp" | "frz"))
                {
                    "|[msg]"
                } else {
                    ""
                };
                out.push(format!(
                    "|-curestatus|{}|{status}{}{msg}",
                    self.ident(&target)?,
                    self.from_of(line)?
                ));
            }
            "boost" | "unboost" => {
                let target = Self::mon_of(line, "mon")?;
                let stat = match line.get("stat").context("stat")? {
                    "acc" => "accuracy",
                    "eva" => "evasion",
                    other => other,
                };
                let by = line.get("by").context("by")?;
                let ident = self.ident(&target)?;
                let from = line.get("from");
                let (from_kind, from_name) = from.map(split_effect).unwrap_or((None, ""));
                if line.has("max")
                    && matches!((self.last_move_name.as_deref(), from_kind), (Some("Belly Drum"), _) | (_, Some("ability")) if from_name == "Anger Point" || self.last_move_name.as_deref() == Some("Belly Drum"))
                {
                    let effect = if from_name == "Anger Point" {
                        "ability: Anger Point"
                    } else {
                        "move: Belly Drum"
                    };
                    out.push(format!("|-setboost|{ident}|{stat}|6|[from] {effect}"));
                } else {
                    let mut text = format!("|-{title}|{ident}|{stat}|{by}");
                    match from_kind {
                        Some("item") => {
                            text.push_str(&format!("|[from] item: {}", names::item_name(from_name)))
                        }
                        Some("ability") => {
                            let of_self = line
                                .get("of")
                                .map(|of| MonRef::parse(of).ok().as_ref() == Some(&target))
                                .unwrap_or(true);
                            let key = (ident.clone(), from_name.to_owned());
                            if of_self && self.boost_group.as_ref() != Some(&key) {
                                out.push(format!(
                                    "|-ability|{ident}|{}|boost",
                                    names::ability_name(from_name)
                                ));
                                self.boost_group = Some(key);
                            }
                        }
                        _ => (),
                    }
                    out.push(text);
                }
            }
            "clearboosts" => {
                let target = Self::mon_of(line, "mon")?;
                out.push(format!("|-clearboost|{}", self.ident(&target)?));
            }
            "clearallboosts" => out.push("|-clearallboost".to_owned()),
            "clearpositiveboosts" => {
                let target = Self::mon_of(line, "mon")?;
                // A stealing move (Spectral Thief) names itself and animates after the boosts.
                let stealer = match (line.get("of"), line.get("from"), &self.last_move_user) {
                    (None, None, Some(user)) => {
                        self.last_move_name.clone().map(|m| (user.clone(), m))
                    }
                    _ => None,
                };
                if let Some((user, move_name)) = stealer {
                    self.mark_last_move_still(&user)?;
                    let user_ident = self.ident(&user)?;
                    self.pending_anim = Some(format!(
                        "|-anim|{user_ident}|{move_name}|{}",
                        self.ident(&target)?
                    ));
                    out.push(format!(
                        "|-clearpositiveboost|{}|{user_ident}|move: {move_name}",
                        self.ident(&target)?
                    ));
                    return Ok(out);
                }
                let of = line
                    .get("of")
                    .map(|of| self.mon_ident(of))
                    .transpose()?
                    .unwrap_or_default();
                let effect = line.get("from").map(from_label).unwrap_or_default();
                out.push(format!(
                    "|-clearpositiveboost|{}|{of}|{effect}",
                    self.ident(&target)?
                ));
            }
            "clearnegativeboosts" => {
                let target = Self::mon_of(line, "mon")?;
                // Showdown's herbs clear the boosts silently, after announcing the item.
                let tags = match line.get("from").map(split_effect) {
                    Some((Some("item"), _)) => "|[silent]".to_owned(),
                    _ => self.from_of(line)?,
                };
                out.push(format!(
                    "|-clearnegativeboost|{}{tags}",
                    self.ident(&target)?
                ));
            }
            "invertboosts" => {
                let target = Self::mon_of(line, "mon")?;
                let from = line
                    .get("from")
                    .map(|f| format!("|[from] {}", from_label(f)))
                    .unwrap_or_default();
                out.push(format!("|-invertboost|{}{from}", self.ident(&target)?));
            }
            "copyboosts" => {
                let target = Self::mon_of(line, "mon")?;
                let source = Self::mon_of(line, "source")?;
                out.push(format!(
                    "|-copyboost|{}|{}{}",
                    self.ident(&source)?,
                    self.ident(&target)?,
                    self.from_of(line)?
                ));
            }
            "swapboosts" => {
                let target = Self::mon_of(line, "mon")?;
                let other = Self::mon_of(line, "of")?;
                let stats = line
                    .get("stats")
                    .unwrap_or("atk,def,spa,spd,spe,accuracy,evasion");
                let from = line
                    .get("from")
                    .map(|f| format!("|[from] {}", from_label(f)))
                    .unwrap_or_default();
                out.push(format!(
                    "|-swapboost|{}|{}|{stats}{from}",
                    self.ident(&target)?,
                    self.ident(&other)?
                ));
            }
            "weather" => {
                let weather = effects::weather_id(line.get("weather").context("weather")?);
                if line.has("residual") {
                    out.push(format!("|-weather|{weather}|[upkeep]"));
                } else {
                    out.push(format!("|-weather|{weather}{}", self.from_of(line)?));
                }
            }
            "clearweather" => out.push("|-weather|none".to_owned()),
            "fieldstart" | "fieldend" | "fieldactivate" => {
                let effect = match (line.get("move"), line.get("condition"), line.get("weather")) {
                    (Some(m), _, _) => effects::field_label(title, "move", m),
                    (_, Some(c), _) => effects::field_label(title, "condition", c),
                    (_, _, Some(w)) => effects::weather_id(w),
                    _ => bail!("{title} without an effect"),
                };
                out.push(format!("|-{title}|{effect}{}", self.from_of(line)?));
            }
            "sidestart" | "sideend" => {
                let side = self.side_label(line.get("side").context("side")?)?;
                let name = line
                    .get("move")
                    .or(line.get("condition"))
                    .context("side condition")?;
                out.push(format!(
                    "|-{title}|{side}|{}{}",
                    effects::side_condition_label(title, name),
                    self.from_of(line)?
                ));
            }
            "singleturn" | "singlemove" => {
                let mon = Self::mon_of(line, "mon")?;
                let (kind, name) = line
                    .get("move")
                    .map(|m| ("move", m))
                    .or(line.get("condition").map(|c| ("condition", c)))
                    .context("single effect")?;
                let mut label = effects::volatile_label(title, kind, name);
                // A protecting move's condition is named `Protect` in battler's log; Showdown
                // labels it per move.
                if name == "Protect" && self.last_move_user.as_ref() == Some(&mon) {
                    if let Some(last) = &self.last_move_name {
                        if let Some(found) = names::label(title, "move", last) {
                            label = found;
                        }
                    }
                }
                out.push(format!(
                    "|-{title}|{}|{label}{}",
                    self.ident(&mon)?,
                    self.from_of(line)?
                ));
            }
            "start" | "end" => {
                let mon = Self::mon_of(line, "mon")?;
                let (kind, name) = line
                    .get("move")
                    .map(|m| ("move", m))
                    .or(line.get("condition").map(|c| ("condition", c)))
                    .or(line.get("ability").map(|a| ("ability", a)))
                    .context("start without an effect")?;
                let label = effects::volatile_label(title, kind, name);
                if title == "start" && label == "move: Leech Seed" {
                    if let Some(seeder) = self.last_move_user.clone() {
                        self.seeds.insert(mon.clone(), seeder);
                    }
                }
                let mut extra = String::new();
                for key in ["disabledmove", "mimic"] {
                    if let Some(value) = line.get(key) {
                        extra.push_str(&format!("|{}", names::move_name(value)));
                    }
                }
                let mut from_of = self.from_of(line)?;
                // Shed Tail's substitute does not say where it came from in battler's log.
                if from_of.is_empty()
                    && label == "Substitute"
                    && self.last_move_name.as_deref() == Some("Shed Tail")
                {
                    from_of = "|[from] move: Shed Tail".to_owned();
                }
                // Yawn ending into sleep is silent.
                let silent = if title == "end" && label == "move: Yawn" {
                    "|[silent]"
                } else {
                    ""
                };
                out.push(format!(
                    "|-{title}|{}|{label}{extra}{from_of}{silent}",
                    self.ident(&mon)?
                ));
            }
            "activate" => {
                if let Some(arm) = self.translate_activate(line)? {
                    out.push(arm);
                }
            }
            "item" | "itemstart" | "itemend" if Self::is_swap_line(line) => {
                let mon = Self::mon_of(line, "mon")?;
                let item = names::item_name(line.get("item").context("item")?);
                let label = split_effect(line.get("from").unwrap_or_default())
                    .1
                    .to_owned();
                let swap = self.swap.get_or_insert_with(ItemSwap::default);
                swap.move_label = names::move_name(&label);
                if title != "itemend" {
                    swap.given.push((mon, item));
                } else {
                    swap.taken.push((mon, item));
                }
            }
            "item" | "itemstart" => {
                let mon = Self::mon_of(line, "mon")?;
                let item = names::item_name(line.get("item").context("item")?);
                // An item announcing itself (Air Balloon) carries no source.
                let tags = match line.get("from").map(split_effect) {
                    Some((Some("item"), name)) if names::item_name(name) == item => String::new(),
                    _ => self.from_of(line)?,
                };
                out.push(format!("|-item|{}|{item}{tags}", self.ident(&mon)?));
            }
            "itemend" => {
                let mon = Self::mon_of(line, "mon")?;
                let item = names::item_name(line.get("item").context("item")?);
                let mut text = format!(
                    "|-enditem|{}|{item}{}",
                    self.ident(&mon)?,
                    self.from_of(line)?
                );
                if line.has("silent") {
                    text.push_str("|[silent]");
                }
                if line.has("eat") {
                    text.push_str("|[eat]");
                }
                out.push(text);
            }
            "ability" | "abilitystart" => {
                let mon = Self::mon_of(line, "mon")?;
                let ability = names::ability_name(line.get("ability").context("ability")?);
                out.push(format!(
                    "|-ability|{}|{ability}{}",
                    self.ident(&mon)?,
                    self.from_of(line)?
                ));
            }
            "cant" => {
                let mon = Self::mon_of(line, "mon")?;
                let from = line.get("from").context("cant without a reason")?;
                let (kind, name) = split_effect(from);
                let move_name = line.get("move").map(names::move_name);
                match (kind, line.get("of")) {
                    // Another Pokémon's ability stopped the move (Queenly Majesty, Dazzling, ...):
                    // Showdown reports the ability's owner and blames the attacker.
                    (Some("ability"), Some(owner)) => {
                        self.mark_last_move_still(&mon)?;
                        let owner = MonRef::parse(owner)?;
                        let mut text = format!(
                            "|cant|{}|ability: {}",
                            self.ident(&owner)?,
                            names::ability_name(name)
                        );
                        if let Some(move_name) = move_name {
                            text.push_str(&format!("|{move_name}"));
                        }
                        text.push_str(&format!("|[of] {}", self.ident(&mon)?));
                        out.push(text);
                    }
                    _ => {
                        let reason = match kind {
                            Some("status") => status_id(name),
                            Some("ability") => format!("ability: {}", names::ability_name(name)),
                            Some("move") => format!("move: {}", names::move_name(name)),
                            _ => effects::cant_label(name),
                        };
                        let mut text = format!("|cant|{}|{reason}", self.ident(&mon)?);
                        if let Some(move_name) = move_name {
                            text.push_str(&format!("|{move_name}"));
                        }
                        out.push(text);
                    }
                }
            }
            "prepare" => {
                let mon = Self::mon_of(line, "mon")?;
                let name = names::move_name(line.get("move").context("move")?);
                let target = line.get("target").map(|t| self.mon_ident(t)).transpose()?;
                match target {
                    Some(target) => {
                        out.push(format!("|-prepare|{}|{name}|{target}", self.ident(&mon)?))
                    }
                    None => out.push(format!("|-prepare|{}|{name}", self.ident(&mon)?)),
                }
            }
            "abilityend" => {
                let mon = Self::mon_of(line, "mon")?;
                out.push(format!(
                    "|-endability|{}{}",
                    self.ident(&mon)?,
                    self.from_of(line)?
                ));
            }
            "hitcount" => {
                let target = self
                    .last_move_target
                    .clone()
                    .context("hitcount without a target")?;
                out.push(format!(
                    "|-hitcount|{}|{}",
                    self.ident(&target)?,
                    line.get("hits").context("hits")?
                ));
            }
            "swap" => {
                let mon = Self::mon_of(line, "mon")?;
                let position: usize = line.get("position").context("position")?.parse()?;
                out.push(format!(
                    "|swap|{}|{}{}",
                    self.ident(&mon)?,
                    position.saturating_sub(1),
                    self.from_of(line)?
                ));
            }
            "turn" => {
                self.pivot = None;
                out.push(format!("|turn|{}", line.get("turn").context("turn")?));
            }
            "residual" => self.residual = true,
            "win" => {
                self.residual = false;
                let side: usize = line.get("side").context("side")?.parse()?;
                let player = self.sides.get(&side).context("unknown side")?;
                out.push(format!(
                    "|win|{}",
                    self.player_names.get(player).unwrap_or(player)
                ));
            }
            "tie" => {
                self.residual = false;
                out.push("|tie".to_owned());
            }
            other => bail!("unsupported log line '{other}'"),
        }
        Ok(out)
    }

    fn translate_activate(&mut self, line: &Line) -> Result<Option<String>> {
        // `activate|move:Splash` has no Pokémon; Showdown prints `-nothing`.
        let mon = match (line.get("mon"), line.get("of")) {
            (Some(mon), _) => Some(MonRef::parse(mon)?),
            (None, Some(of)) => Some(MonRef::parse(of)?),
            _ => None,
        };
        if let Some(name) = line.get("move") {
            if name == "Splash" && line.get("mon").is_none() {
                return Ok(Some("|-nothing".to_owned()));
            }
            // Spectral Thief's announcement is the `-clearpositiveboost` that follows.
            if name == "Spectral Thief" {
                return Ok(None);
            }
            let mon = mon.context("activate without a Pokémon")?;
            return Ok(Some(format!(
                "|-activate|{}|move: {}",
                self.ident(&mon)?,
                names::move_name(name)
            )));
        }
        if let Some(ability) = line.get("ability") {
            let mon = mon.context("activate without a Pokémon")?;
            let ability = names::ability_name(ability);
            // The ability behind a forme change is announced by the forme change alone.
            if self
                .last_formechange
                .as_ref()
                .is_some_and(|(m, a)| *m == mon && *a == ability)
            {
                return Ok(None);
            }
            let suffix = if ability == "Intimidate" {
                "|boost"
            } else {
                ""
            };
            if ability == "Intimidate" {
                return Ok(Some(format!(
                    "|-ability|{}|{ability}{suffix}",
                    self.ident(&mon)?
                )));
            }
            return Ok(Some(format!(
                "|-activate|{}|ability: {ability}",
                self.ident(&mon)?
            )));
        }
        if let Some(condition) = line.get("condition") {
            let mon = mon.context("activate without a Pokémon")?;
            let condition = if condition == "Confusion" {
                "confusion"
            } else {
                condition
            };
            return Ok(Some(format!(
                "|-activate|{}|{condition}",
                self.ident(&mon)?
            )));
        }
        bail!("unsupported activate line {line:?}")
    }
}
