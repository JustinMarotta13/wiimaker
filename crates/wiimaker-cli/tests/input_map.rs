//! CLI twin: `wiimaker input map` / `--json`.

use std::process::Command;

fn wiimaker() -> Command {
    Command::new(env!("CARGO_BIN_EXE_wiimaker"))
}

#[test]
fn input_map_json_lists_sources() {
    let out = wiimaker()
        .args(["input", "map", "--json"])
        .output()
        .expect("run wiimaker");
    assert!(
        out.status.success(),
        "input map failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect(&stdout);
    assert_eq!(v["ok"], true);
    let legend = v["legend"].as_str().unwrap_or("");
    assert!(legend.contains("Wiimote"));
    assert!(legend.contains("Nunchuk"));
    assert!(v["deadzone"].as_f64().unwrap() > 0.0);
    let map = v["map"].as_array().expect("map array");
    let mut sources = Vec::new();
    for row in map {
        sources.push(row["source"].as_str().unwrap_or("").to_string());
    }
    for need in ["Keyboard", "GCN", "Wiimote", "Classic", "Nunchuk", "Host"] {
        assert!(
            sources.iter().any(|s| s == need),
            "missing {need} in {stdout}"
        );
    }
    assert!(
        legend.contains("IR"),
        "legend missing IR: {legend}"
    );
    assert!(
        map.iter().any(|r| {
            r["source"] == "Wiimote"
                && r["control"].as_str().unwrap_or("").contains("IR")
                && r["target"].as_str().unwrap_or("").contains("aim")
        }),
        "Wiimote IR → aim missing in {stdout}"
    );
    assert!(
        map.iter().any(|r| {
            r["source"] == "Host"
                && r["control"].as_str().unwrap_or("").contains("Mouse")
                && r["target"].as_str().unwrap_or("").contains("aim")
        }),
        "Host mouse → aim missing in {stdout}"
    );
    assert!(
        map.iter()
            .any(|r| r["source"] == "Nunchuk" && r["control"] == "Z" && r["target"] == "Z"),
        "Nunchuk Z → Z missing in {stdout}"
    );
    assert_eq!(v["buttons"]["a"], 1);
    assert_eq!(v["buttons"]["right"], 1 << 11);
}

#[test]
fn input_map_text_prints_legend() {
    let out = wiimaker()
        .args(["input", "map"])
        .output()
        .expect("run wiimaker");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("GCN-layout"));
    assert!(stdout.contains("Classic"));
    assert!(stdout.contains("Nunchuk"));
    assert!(stdout.contains("1"));
    assert!(stdout.contains("IR") || stdout.contains("sensor"));
    assert!(stdout.contains("Mouse") || stdout.contains("Host"));
}
