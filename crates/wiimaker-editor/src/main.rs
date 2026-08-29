//! egui scene editor for wiimaker games.
//!
//! Unity 6 docks: Hierarchy · Scene/Game · Inspector · Project/Console (dark Pro chrome)

mod app;
mod sprite_editor;
mod theme;
mod ui_console;
mod ui_hierarchy;
mod ui_inspector;
mod ui_project;
mod ui_toolbar;
mod viewport;
mod workspace;

use eframe::egui;

use app::EditorApp;
use workspace::find_root;

fn main() -> eframe::Result<()> {
    let game = std::env::args().nth(1).unwrap_or_else(|| "hello-orb".into());
    let root = find_root().expect("wiimaker workspace root");
    let state = EditorApp::open(&root, &game).unwrap_or_else(|e| {
        eprintln!("editor error: {e:#}");
        std::process::exit(1);
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_title(format!("wiimaker · {}", state.project.title)),
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
