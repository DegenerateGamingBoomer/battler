mod common;
use common::*;

/// `FIXTURE=name cargo test -p battler-showdown-compat --test dump -- --ignored --nocapture`
/// (corpus battles: `FIXTURE=corpus-007 CORPUS=1`). Prints the context around the first difference
/// between Showdown's log and the translated battler log, and the raw battler lines nearby.
#[test]
#[ignore]
fn dump() {
    let name = std::env::var("FIXTURE").unwrap_or("basic-doubles".into());
    let fixture = if std::env::var("CORPUS").is_ok() {
        load_path(
            &std::path::PathBuf::from(format!(
                "{}/../compat/corpus/{name}.json",
                env!("CARGO_MANIFEST_DIR")
            )),
            &name,
        )
    } else {
        load(&name)
    };
    let mut run = set_up(&fixture);
    for (i, step) in fixture.steps.iter().enumerate() {
        for (player, choice) in ["p1", "p2"].iter().zip(step) {
            if choice.is_empty() {
                continue;
            }
            if let Err(e) = run.choose(player, choice) {
                if std::env::var("STEPS").is_ok() {
                    println!("!! step {i}: {player} choice rejected: {e:#}");
                }
            }
        }
    }
    let expected = apply_allowlist(&fixture.log);
    let translated = run.translated();
    let raw = run.log();
    match translated {
        Err(e) => println!("!! translate error: {e:#}"),
        Ok(translated) => {
            let actual = apply_allowlist(&translated);
            let n = expected.len().max(actual.len());
            let first = (0..n).find(|&i| expected.get(i) != actual.get(i));
            match first {
                None => println!("MATCH ({} lines)", expected.len()),
                Some(i) => {
                    let from = i.saturating_sub(8);
                    println!("first difference at line {i}");
                    for j in from..(i + 6).min(n) {
                        let mark = if expected.get(j) != actual.get(j) {
                            ">>"
                        } else {
                            "  "
                        };
                        println!(
                            "{mark} SD: {}\n{mark} BT: {}",
                            expected.get(j).map(String::as_str).unwrap_or("<none>"),
                            actual.get(j).map(String::as_str).unwrap_or("<none>")
                        );
                    }
                }
            }
        }
    }
    if std::env::var("RAW").is_ok() {
        println!("=== RAW BATTLER LOG ===\n{}", raw.join("\n"));
    }
}
