//! Replays random Showdown battles (`compat/gen-corpus.js`) on battler and reports where the
//! translated log first differs. Informational: run with
//! `cargo test -p battler-showdown-compat --test corpus -- --ignored --nocapture`.

mod common;

use std::collections::BTreeMap;

use common::*;

fn shape(line: &str) -> String {
    line.split('|')
        .enumerate()
        .map(|(i, part)| {
            if i <= 1 {
                part.to_owned()
            } else if part.starts_with('p') && part.contains(": ") {
                "MON".to_owned()
            } else if part.contains('/') && part.chars().next().is_some_and(|c| c.is_ascii_digit())
            {
                "HP".to_owned()
            } else if part.chars().all(|c| c.is_ascii_digit()) {
                "N".to_owned()
            } else {
                part.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("|")
}

#[test]
#[ignore]
fn corpus() {
    let dir = std::env::var("CORPUS_DIR")
        .unwrap_or_else(|_| format!("{}/../compat/corpus", env!("CARGO_MANIFEST_DIR")));
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    files.sort();
    let (mut total, mut matching) = (0, 0);
    let mut kinds: BTreeMap<String, (usize, String)> = BTreeMap::new();
    for path in files {
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let fixture = load_path(&path, &name);
        total += 1;
        let result = std::panic::catch_unwind(|| {
            let mut run = set_up(&fixture);
            let mut error = None;
            for (i, step) in fixture.steps.iter().enumerate() {
                for (player, choice) in ["p1", "p2"].iter().zip(step) {
                    if choice.is_empty() {
                        continue;
                    }
                    if let Err(e) = run.choose(player, choice) {
                        error = Some(format!(
                            "step {i}: battler rejected {player} '{choice}': {e:#}"
                        ));
                        break;
                    }
                }
                if error.is_some() {
                    break;
                }
            }
            (run.translated(), error)
        });
        let (translated, error) = match result {
            Ok(r) => r,
            Err(_) => {
                kinds
                    .entry("setup/panic".to_owned())
                    .or_insert((0, name.clone()))
                    .0 += 1;
                continue;
            }
        };
        let translated = match translated {
            Ok(t) => t,
            Err(e) => {
                let key = format!("translate error: {}", e.root_cause());
                kinds.entry(key).or_insert((0, name.clone())).0 += 1;
                continue;
            }
        };
        let (expected, actual) = (apply_allowlist(&fixture.log), apply_allowlist(&translated));
        let n = expected.len().max(actual.len());
        let first = (0..n).find(|&i| expected.get(i) != actual.get(i));
        match (first, error) {
            (None, None) => matching += 1,
            (None, Some(e)) => {
                kinds
                    .entry(format!(
                        "choice error only: {}",
                        e.split(':').next().unwrap_or("")
                    ))
                    .or_insert((0, name.clone()))
                    .0 += 1;
            }
            (Some(i), _) => {
                let e = expected.get(i).map(|l| shape(l)).unwrap_or("<none>".into());
                let a = actual.get(i).map(|l| shape(l)).unwrap_or("<none>".into());
                let key = format!("SD: {e}   |   BT: {a}");
                let entry = kinds.entry(key).or_insert((0, format!("{name} line {i}")));
                entry.0 += 1;
            }
        }
    }
    println!("{matching}/{total} battles match exactly");
    let mut kinds: Vec<_> = kinds.into_iter().collect();
    kinds.sort_by_key(|(_, (n, _))| std::cmp::Reverse(*n));
    for (kind, (n, example)) in kinds.iter().take(60) {
        println!("{n:4}  {kind}   [{example}]");
    }
}
