//! Showdown choices → battler choices.
//!
//! Showdown numbers move slots, team positions and switch targets from 1 and separates the choices
//! of the active Pokémon with commas (`move 1 2, move 3 -1 mega`); battler numbers from 0 and uses
//! semicolons (`move 0,2;move 2,-1,mega`). Move targets keep Showdown's meaning (positive = foe
//! position, negative = ally position, 1-based).

use anyhow::{
    Context,
    Result,
    bail,
};
use battler_choice::{
    Choice,
    MoveChoice,
    SelectChoice,
    SwitchChoice,
    TeamSelectionChoice,
    choices_to_string,
};

use crate::names::to_id;

/// Optional context for choices that name a move or Pokémon instead of numbering it.
#[derive(Debug, Default, Clone)]
pub struct ChoiceContext {
    /// Move names or IDs of each active Pokémon's move slots, in slot order.
    pub active_moves: Vec<Vec<String>>,
    /// Nicknames or species of the team, in team order.
    pub team: Vec<String>,
    /// During a switch request, the active positions (0-based) that must switch.
    ///
    /// Showdown answers a forced switch with one choice per active slot (`pass, switch 3`);
    /// battler takes one choice per Pokémon that has to switch, in order. With this set, the
    /// choices of the other slots are dropped.
    pub switch_positions: Option<Vec<usize>>,
    /// During a turn request, the active positions (0-based) that take a choice.
    ///
    /// Showdown expects a choice (`pass`) for an empty or fainted slot; battler only takes choices
    /// for the Pokémon that can act. With this set, the choices of the other slots are dropped.
    pub turn_positions: Option<Vec<usize>>,
    /// During a select request (Revival Blessing), the active positions (0-based) that select a
    /// Pokémon.
    ///
    /// Showdown answers with a switch to the fainted Pokémon to revive; battler takes a select
    /// choice. With this set, switches become selects and the choices of other slots are dropped.
    pub select_positions: Option<Vec<usize>>,
    /// Showdown's current team order (its `side.pokemon`) as battler team positions.
    ///
    /// Showdown numbers switch targets in this order, which changes as Pokémon switch in and out;
    /// battler numbers them by team position. Empty means both orders are the same.
    pub showdown_order: Vec<usize>,
}

fn parse_index(token: &str, what: &str) -> Result<usize> {
    let n: usize = token
        .parse()
        .with_context(|| format!("invalid {what} '{token}'"))?;
    if n == 0 {
        bail!("{what} numbers start at 1");
    }
    Ok(n - 1)
}

fn translate_move(args: &[&str], active_index: usize, context: &ChoiceContext) -> Result<Choice> {
    let Some((slot_token, rest)) = args.split_first() else {
        bail!("move choice needs a move");
    };
    let slot = match slot_token.parse::<usize>() {
        Ok(_) => parse_index(slot_token, "move slot")?,
        Err(_) => {
            let id = to_id(slot_token);
            context
                .active_moves
                .get(active_index)
                .and_then(|moves| moves.iter().position(|m| to_id(m) == id))
                .with_context(|| format!("no move '{slot_token}' to choose"))?
        }
    };
    let mut choice = MoveChoice {
        slot,
        ..Default::default()
    };
    for arg in rest {
        match *arg {
            "mega" => choice.mega = true,
            "zmove" => choice.z_move = true,
            "ultra" => choice.ultra = true,
            "dynamax" | "max" => choice.dyna = true,
            "terastallize" | "tera" => choice.tera = true,
            target => {
                choice.target = Some(
                    target
                        .trim_start_matches('+')
                        .parse()
                        .with_context(|| format!("invalid move option '{arg}'"))?,
                );
            }
        }
    }
    Ok(Choice::Move(choice))
}

fn translate_one(part: &str, active_index: usize, context: &ChoiceContext) -> Result<Choice> {
    let tokens: Vec<&str> = part.split_whitespace().collect();
    let Some((verb, args)) = tokens.split_first() else {
        bail!("empty choice");
    };
    match *verb {
        "move" => translate_move(args, active_index, context),
        "switch" => {
            let Some(token) = args.first() else {
                return Ok(Choice::Switch(SwitchChoice::default()));
            };
            let mon = match token.parse::<usize>() {
                Ok(_) => {
                    let index = parse_index(token, "switch target")?;
                    context.showdown_order.get(index).copied().unwrap_or(index)
                }
                Err(_) => {
                    let id = to_id(&args.join(" "));
                    context
                        .team
                        .iter()
                        .position(|name| to_id(name) == id)
                        .with_context(|| format!("no Pokémon '{}' to switch to", args.join(" ")))?
                }
            };
            if context.select_positions.is_some() {
                return Ok(Choice::Select(SelectChoice { mon: Some(mon) }));
            }
            Ok(Choice::Switch(SwitchChoice { mon: Some(mon) }))
        }
        "shift" => Ok(Choice::Shift),
        "pass" => Ok(Choice::Pass),
        "default" | "auto" => Ok(Choice::RandomAll),
        other => bail!("unsupported choice '{other}'"),
    }
}

/// Translates a full Showdown choice string for one player into a battler choice string.
pub fn translate(showdown_choice: &str, context: &ChoiceContext) -> Result<String> {
    let trimmed = showdown_choice.trim();
    if let Some(rest) = trimmed.strip_prefix("team ") {
        let digits: Vec<&str> = if rest.contains(',') || rest.contains(' ') {
            rest.split([',', ' ']).filter(|s| !s.is_empty()).collect()
        } else {
            return Ok(Choice::Team(TeamSelectionChoice {
                mons: rest
                    .chars()
                    .map(|c| parse_index(&c.to_string(), "team position"))
                    .collect::<Result<_>>()?,
            })
            .to_string());
        };
        return Ok(Choice::Team(TeamSelectionChoice {
            mons: digits
                .into_iter()
                .map(|d| parse_index(d, "team position"))
                .collect::<Result<_>>()?,
        })
        .to_string());
    }
    let choices = trimmed
        .split(',')
        .enumerate()
        .filter(|(i, _)| {
            context
                .switch_positions
                .as_ref()
                .or(context.select_positions.as_ref())
                .or(context.turn_positions.as_ref())
                .is_none_or(|positions| positions.contains(i))
        })
        .map(|(i, part)| translate_one(part, i, context))
        .collect::<Result<Vec<_>>>()?;
    Ok(choices_to_string(choices))
}

#[cfg(test)]
mod choice_test {
    use super::{
        ChoiceContext,
        translate,
    };

    fn t(s: &str) -> String {
        translate(s, &ChoiceContext::default()).unwrap()
    }

    #[test]
    fn translates_moves_and_targets() {
        assert_eq!(t("move 1"), "move 0");
        assert_eq!(t("move 1 2, move 3 -1 mega"), "move 0,2;move 2,-1,mega");
        assert_eq!(t("move 2 +1 terastallize"), "move 1,1,tera");
        assert_eq!(t("move 4 zmove"), "move 3,zmove");
    }

    #[test]
    fn translates_switches_passes_and_teams() {
        assert_eq!(t("switch 3"), "switch 2");
        assert_eq!(t("switch 3, pass"), "switch 2;pass");
        assert_eq!(t("team 1234"), "team 0 1 2 3");
        assert_eq!(t("team 1, 2, 3"), "team 0 1 2");
        assert_eq!(t("shift"), "shift");
    }

    #[test]
    fn resolves_names_from_context() {
        let context = ChoiceContext {
            active_moves: vec![vec!["Thunderbolt".to_owned(), "Protect".to_owned()]],
            team: vec!["Pikachu".to_owned(), "Charizard".to_owned()],
            ..Default::default()
        };
        assert_eq!(translate("move protect", &context).unwrap(), "move 1");
        assert_eq!(translate("switch charizard", &context).unwrap(), "switch 1");
        assert!(translate("move splash", &context).is_err());
    }

    #[test]
    fn drops_choices_for_slots_that_do_not_switch() {
        let context = ChoiceContext {
            switch_positions: Some(vec![1]),
            ..Default::default()
        };
        assert_eq!(translate("pass, switch 3", &context).unwrap(), "switch 2");
        let both = ChoiceContext {
            switch_positions: Some(vec![0, 1]),
            ..Default::default()
        };
        assert_eq!(
            translate("switch 3, switch 4", &both).unwrap(),
            "switch 2;switch 3"
        );
    }

    #[test]
    fn translates_revival_switches_to_selects() {
        let context = ChoiceContext {
            select_positions: Some(vec![1]),
            ..Default::default()
        };
        assert_eq!(translate("pass, switch 3", &context).unwrap(), "select 2");
    }

    #[test]
    fn rejects_zero_based_numbers() {
        assert!(translate("move 0", &ChoiceContext::default()).is_err());
        assert!(translate("switch 0", &ChoiceContext::default()).is_err());
    }
}
