//! egui scene editor for wiimaker games.
//!
//! Unity 6 docks: Hierarchy · Scene/Game · Inspector · Project/Console (dark Pro chrome)

mod app;
mod dock;
mod sprite_editor;
mod theme;
mod ui_animator;
mod ui_console;
mod ui_controller;
mod ui_hierarchy;
mod ui_inspector;
mod ui_project;
mod ui_timeline;
mod ui_toolbar;
mod viewport;
mod workspace;

use eframe::egui;

use app::EditorApp;
use workspace::find_root;

/// `WIIMAKER_EDITOR_SIZE=1600x1100` for screenshot runs. Default is the usual editor size.
fn editor_window_size() -> (f32, f32) {
    std::env::var("WIIMAKER_EDITOR_SIZE")
        .ok()
        .and_then(|s| {
            let (w, h) = s.trim().split_once(['x', 'X', ','])?;
            Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
        })
        .unwrap_or((1280.0, 800.0))
}

fn main() -> eframe::Result<()> {
    let game = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "hello-orb".into());
    let root = find_root().expect("wiimaker workspace root");
    let state = EditorApp::open(&root, &game).unwrap_or_else(|e| {
        eprintln!("editor error: {e:#}");
        std::process::exit(1);
    });

    let (win_w, win_h) = editor_window_size();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([win_w, win_h])
            .with_min_inner_size([1024.0, 640.0])
            .with_title(format!("wiimaker · {}", state.project.title)),
        persist_window: false,
        ..Default::default()
    };
    eframe::run_native(
        "wiimaker editor",
        options,
        Box::new(move |cc| {
            theme::apply(&cc.egui_ctx);
            Ok(Box::new(state))
        }),
    )
}
