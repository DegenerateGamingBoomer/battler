//! Parsing of battler log lines.

use anyhow::{
    Result,
    bail,
};

/// One battler log line: a title followed by `key:value` attributes and bare flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub title: String,
    /// Attributes and flags in the order they appear (`None` for flags).
    pub parts: Vec<(String, Option<String>)>,
}

impl Line {
    pub fn parse(line: &str) -> Result<Self> {
        let mut parts = line.split('|');
        let Some(title) = parts.next().filter(|t| !t.is_empty()) else {
            bail!("empty log line");
        };
        let parts = parts
            .map(|part| match part.split_once(':') {
                Some((key, value)) => (key.to_owned(), Some(value.to_owned())),
                None => (part.to_owned(), None),
            })
            .collect();
        Ok(Self {
            title: title.to_owned(),
            parts,
        })
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.parts
            .iter()
            .find(|(k, v)| k == key && v.is_some())
            .and_then(|(_, v)| v.as_deref())
    }

    pub fn has(&self, flag: &str) -> bool {
        self.parts.iter().any(|(k, v)| k == flag && v.is_none())
    }
}

/// A reference to a Mon in a log line: `Name,player,position` (position absent when the Mon is not
/// active).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MonRef {
    pub name: String,
    pub player: String,
    pub position: Option<usize>,
}

impl MonRef {
    pub fn parse(value: &str) -> Result<Self> {
        let mut parts = value.rsplitn(3, ',');
        let last = parts.next();
        let middle = parts.next();
        let first = parts.next();
        match (first, middle, last) {
            // `Name,player,position`
            (Some(name), Some(player), Some(position)) if position.parse::<usize>().is_ok() => {
                Ok(Self {
                    name: name.to_owned(),
                    player: player.to_owned(),
                    position: position.parse().ok(),
                })
            }
            // `Name,player` where the Name itself may contain commas.
            (Some(name), Some(middle), Some(player)) => Ok(Self {
                name: format!("{name},{middle}"),
                player: player.to_owned(),
                position: None,
            }),
            (None, Some(name), Some(player)) => Ok(Self {
                name: name.to_owned(),
                player: player.to_owned(),
                position: None,
            }),
            _ => bail!("invalid mon reference '{value}'"),
        }
    }
}

#[cfg(test)]
mod line_test {
    use super::{
        Line,
        MonRef,
    };

    #[test]
    fn parses_lines() {
        let line =
            Line::parse("damage|mon:Pikachu,p1,1|from:status:Burn|health:130/235|silent").unwrap();
        assert_eq!(line.title, "damage");
        assert_eq!(line.get("mon"), Some("Pikachu,p1,1"));
        assert_eq!(line.get("from"), Some("status:Burn"));
        assert!(line.has("silent"));
        assert_eq!(Line::parse("residual").unwrap().parts.len(), 0);
    }

    #[test]
    fn parses_mon_refs() {
        assert_eq!(
            MonRef::parse("Pikachu,p1,2").unwrap(),
            MonRef {
                name: "Pikachu".into(),
                player: "p1".into(),
                position: Some(2)
            }
        );
        assert_eq!(
            MonRef::parse("Pikachu,p1").unwrap(),
            MonRef {
                name: "Pikachu".into(),
                player: "p1".into(),
                position: None
            }
        );
    }
}
