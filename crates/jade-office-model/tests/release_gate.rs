//! The release gate: docs/GAPS.md marks what a release needs with
//! **Blocker**, and a row is closed by marking it **Done** with the pull
//! request that did it. A release is ready when no **Blocker** row is left.
//!
//! Run it before a release:
//!
//! ```sh
//! cargo test --manifest-path crates/jade-office-model/Cargo.toml --test release_gate -- --ignored
//! ```
//!
//! It is ignored in CI until the last blocker closes, so the open ones
//! do not fail every other change.

use std::path::PathBuf;

fn gaps() -> String {
    let path = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/GAPS.md"));
    std::fs::read_to_string(path).expect("docs/GAPS.md")
}

/// The open blocker rows, as "Section: gap".
fn open_blockers(gaps: &str) -> Vec<String> {
    let mut section = String::new();
    let mut open = Vec::new();
    for line in gaps.lines() {
        if let Some(title) = line.strip_prefix("## ") {
            section = title.trim().to_string();
            continue;
        }
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if cells.len() > 3 && cells[1] == "**Blocker**" {
            open.push(format!("{section}: {}", cells[2]));
        }
    }
    open
}

/// Every closed row names the pull request that closed it.
fn closed_without_a_pull_request(gaps: &str) -> Vec<String> {
    gaps.lines()
        .filter_map(|line| {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            let mark = *cells.get(1)?;
            let names_one = mark
                .strip_prefix("**Done**")
                .map(|rest| rest.contains('#'))?;
            (!names_one).then(|| cells.get(2).unwrap_or(&"").to_string())
        })
        .collect()
}

#[test]
fn the_gate_reads_blockers_and_done_rows() {
    let sample = "## Excel\n\n| | Gap | Hit | Engine |\n|---|---|---|---|\n| **Blocker** | Paste Special | High | no |\n| **Done** (#75) | Conditional formats | High | yes |\n| | Format Painter | Medium | yes |\n";
    assert_eq!(open_blockers(sample), ["Excel: Paste Special"]);
    assert!(closed_without_a_pull_request(sample).is_empty());
    assert_eq!(
        closed_without_a_pull_request("| **Done** | Charts | High | yes |"),
        ["Charts"]
    );
    // The inventory itself parses: it has sections and closed rows.
    let gaps = gaps();
    assert!(gaps.contains("## Excel") && gaps.contains("## Word"));
    assert!(closed_without_a_pull_request(&gaps).is_empty());
}

#[test]
#[ignore = "run before a release: open blockers fail it until they close"]
fn no_blocker_is_left_open() {
    let open = open_blockers(&gaps());
    assert!(
        open.is_empty(),
        "{} blocker(s) left before a release:\n{}",
        open.len(),
        open.join("\n")
    );
}
