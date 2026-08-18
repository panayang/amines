use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::NamedTempFile;

#[test]
fn test_cli_init_and_view_json() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap();

    // 1. Initialize easy game
    let mut cmd = Command::cargo_bin("amine-cli").unwrap();
    cmd.args(["init", "-d", "easy", "-s", path])
        .assert()
        .success()
        .stdout(predicate::str::contains("INITIALIZED"))
        .stdout(predicate::str::contains("3D_MOBIUS_STRIP"));

    // 2. View game state
    let mut cmd = Command::cargo_bin("amine-cli").unwrap();
    let assert = cmd
        .args(["view", "-s", path, "-f", "json"])
        .assert()
        .success();

    let output = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(v["status"], "Waiting");
    assert_eq!(v["total_non_mines"], 218); // 9x9x3 = 243 - 25 = 218
    assert_eq!(v["flag_count"], 0);
    assert_eq!(v["revealed_count"], 0);
}

#[test]
fn test_cli_custom_dimensions_and_validation() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap();

    // 1. Initialize custom game: 14x12x2, 28 mines
    let mut cmd = Command::cargo_bin("amine-cli").unwrap();
    cmd.args([
        "init", "-d", "custom", "-w", "14", "-H", "12", "-z", "2", "-m", "28", "-s", path,
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("14"))
    .stdout(predicate::str::contains("12"));

    // 2. View game state
    let mut cmd = Command::cargo_bin("amine-cli").unwrap();
    let assert = cmd
        .args(["view", "-s", path, "-f", "json"])
        .assert()
        .success();

    let output = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(v["total_non_mines"], 14 * 12 * 2 - 28);
}

#[test]
fn test_cli_step_reveal_flag_and_solve_step() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap();

    // 1. Init deterministic game with seed 42
    let mut cmd = Command::cargo_bin("amine-cli").unwrap();
    cmd.args(["init", "-d", "easy", "--seed", "42", "-s", path])
        .assert()
        .success();

    // 2. First reveal at center (4, 4, 1) - guaranteed 0 mine opening
    let mut cmd = Command::cargo_bin("amine-cli").unwrap();
    cmd.args([
        "step", "-a", "reveal", "-x", "4", "-y", "4", "-z", "1", "-s", path, "-f", "json",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("REVEALED_"))
    .stdout(predicate::str::contains("Playing"));

    // 3. Ask solver for next action
    let mut cmd = Command::cargo_bin("amine-cli").unwrap();
    let assert = cmd
        .args(["solve-step", "--tier", "master", "-s", path])
        .assert()
        .success();

    let output = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(v["status"], "ACTION_RECOMMENDED");
    assert!(v["action"]["type"].is_string());

    let act_type = v["action"]["type"].as_str().unwrap();
    let coord = &v["action"]["coord"];
    let x = coord["x"].as_u64().unwrap().to_string();
    let y = coord["y"].as_u64().unwrap().to_string();
    let z = coord["z"].as_u64().unwrap().to_string();

    // 4. Apply solver's recommended action
    let mut cmd = Command::cargo_bin("amine-cli").unwrap();
    let action_arg = match act_type {
        "Reveal" => "reveal",
        "Flag" => "flag",
        "Chord" => "chord",
        _ => panic!("Unknown action type"),
    };
    cmd.args([
        "step", "-a", action_arg, "-x", &x, "-y", &y, "-z", &z, "-s", path, "-f", "json",
    ])
    .assert()
    .success();
}

#[test]
fn test_cli_benchmark_command() {
    let mut cmd = Command::cargo_bin("amine-cli").unwrap();
    cmd.args([
        "benchmark",
        "--tier",
        "novice",
        "-d",
        "easy",
        "-n",
        "1",
        "--seed",
        "100",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("BENCHMARK REPORT"))
    .stdout(predicate::str::contains("win_rate_pct"));
}
