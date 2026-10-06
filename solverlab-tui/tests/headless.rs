//! End-to-end checks of the headless mode through the real binary.

use std::process::Command;

fn run_headless(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_solverlab-tui"))
        .args(["--headless", "--quiet"])
        .args(args)
        .output()
        .expect("failed to run solverlab-tui");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// `game_id,moves,undos,checkpoints,result,score` for each detail row: the
/// columns that must not depend on CPU speed.
fn deterministic_columns(csv: &str) -> Vec<String> {
    csv.lines()
        .skip_while(|line| *line != "details")
        .skip(2)
        .map(|line| {
            let fields: Vec<&str> = line.split(',').collect();
            [0, 1, 2, 3, 4, 7].map(|i| fields[i]).join(",")
        })
        .collect()
}

#[test]
fn headless_prints_the_csv() {
    let csv = run_headless(&[
        "--game",
        "tripeaks",
        "--sims",
        "3",
        "--parallel",
        "2",
        "--seed",
        "7",
    ]);
    assert!(csv.starts_with("configuration\n"));
    assert!(csv.contains("\"TriPeaks\",\"A*\",3,2,false,-1,0.00,150"));
    assert!(csv.contains("\nsummary\n"));
    let rows = deterministic_columns(&csv);
    assert_eq!(rows.len(), 3);
    assert!(rows[0].starts_with("1,"));
    assert!(rows[2].starts_with("3,"));
}

#[test]
fn headless_writes_the_csv_to_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.csv");
    let stdout = run_headless(&[
        "--game",
        "pyramid",
        "--sims",
        "2",
        "--seed",
        "1",
        "--csv",
        path.to_str().unwrap(),
    ]);
    assert!(stdout.is_empty());
    let csv = std::fs::read_to_string(&path).unwrap();
    assert_eq!(deterministic_columns(&csv).len(), 2);
}

#[test]
fn seeded_runs_match_across_executions() {
    let args = [
        "--game",
        "tripeaks",
        "--sims",
        "4",
        "--parallel",
        "2",
        "--seed",
        "1234",
    ];
    let first = deterministic_columns(&run_headless(&args));
    let second = deterministic_columns(&run_headless(&args));
    assert_eq!(first, second);
}

#[test]
fn zero_simulations_produce_an_empty_report() {
    let csv = run_headless(&["--game", "freecell", "--sims", "0"]);
    assert!(csv.contains("\n0,00:00,0.00%,0.00,0.00,0.00,0.00s,0,0,0\n"));
    assert!(deterministic_columns(&csv).is_empty());
}
