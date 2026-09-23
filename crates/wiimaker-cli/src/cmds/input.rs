use anyhow::Result;
use serde::Serialize;
use wiimaker_core::wiimote_map::{
    MapRow, BTN_A, BTN_B, BTN_DOWN, BTN_L, BTN_LEFT, BTN_R, BTN_RIGHT, BTN_START, BTN_UP, BTN_X,
    BTN_Y, BTN_Z, INPUT_LEGEND, MAP_ROWS, STICK_IDLE_DEADZONE,
};

use crate::args::InputCmd;

#[derive(Serialize)]
struct MapOut<'a> {
    ok: bool,
    lingua: &'a str,
    legend: &'a str,
    deadzone: f32,
    buttons: ButtonsOut,
    map: &'a [MapRowJson<'a>],
}

#[derive(Serialize)]
struct ButtonsOut {
    a: u32,
    b: u32,
    x: u32,
    y: u32,
    start: u32,
    z: u32,
    l: u32,
    r: u32,
    up: u32,
    down: u32,
    left: u32,
    right: u32,
}

#[derive(Serialize)]
struct MapRowJson<'a> {
    source: &'a str,
    control: &'a str,
    target: &'a str,
}

fn buttons_out() -> ButtonsOut {
    ButtonsOut {
        a: BTN_A,
        b: BTN_B,
        x: BTN_X,
        y: BTN_Y,
        start: BTN_START,
        z: BTN_Z,
        l: BTN_L,
        r: BTN_R,
        up: BTN_UP,
        down: BTN_DOWN,
        left: BTN_LEFT,
        right: BTN_RIGHT,
    }
}

fn rows_json() -> Vec<MapRowJson<'static>> {
    MAP_ROWS
        .iter()
        .map(|r: &MapRow| MapRowJson {
            source: r.source,
            control: r.control,
            target: r.target,
        })
        .collect()
}

pub fn input_cmd(cmd: InputCmd, json: bool) -> Result<()> {
    match cmd {
        InputCmd::Map => input_map(json),
    }
}

fn input_map(json: bool) -> Result<()> {
    let rows = rows_json();
    if json {
        let out = MapOut {
            ok: true,
            lingua: "GCN-layout Input",
            legend: INPUT_LEGEND,
            deadzone: STICK_IDLE_DEADZONE,
            buttons: buttons_out(),
            map: &rows,
        };
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!("GCN-layout Input  ·  {INPUT_LEGEND}");
        println!(
            "idle deadzone {:.2}  (leave GCN stick alone; D-pad fills main if still idle)",
            STICK_IDLE_DEADZONE
        );
        println!();
        println!("{:<10} {:<36} {}", "source", "control", "target");
        for r in MAP_ROWS {
            println!("{:<10} {:<36} {}", r.source, r.control, r.target);
        }
    }
    Ok(())
}
