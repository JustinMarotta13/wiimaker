//! Play Maximized dock backup/restore (Unity Game view toolbar).

use egui_dock::DockState;
use wiimaker_scene::GamePlayMode;

use crate::dock::EditorTab;

#[derive(Default)]
pub(crate) struct PlayLayoutBackup {
    saved: Option<DockState<EditorTab>>,
}

impl PlayLayoutBackup {
    pub(crate) fn sync(
        &mut self,
        dock: &mut DockState<EditorTab>,
        in_play: bool,
        play_mode: GamePlayMode,
    ) {
        let maximize = in_play && play_mode == GamePlayMode::Maximized;
        if maximize {
            if self.saved.is_none() {
                self.saved = Some(std::mem::replace(
                    dock,
                    DockState::new(vec![EditorTab::Game]),
                ));
            }
        } else if let Some(saved) = self.saved.take() {
            *dock = saved;
        }
    }

    pub(crate) fn has_saved(&self) -> bool {
        self.saved.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dock::{default_unity_layout, EditorTab};

    #[test]
    fn maximized_play_then_stop_restores_dock() {
        let mut dock = default_unity_layout();
        let mut backup = PlayLayoutBackup::default();
        assert!(dock.find_tab(&EditorTab::Hierarchy).is_some());

        backup.sync(&mut dock, true, GamePlayMode::Maximized);
        assert!(backup.has_saved());
        assert!(dock.find_tab(&EditorTab::Hierarchy).is_none());
        assert!(dock.find_tab(&EditorTab::Game).is_some());

        backup.sync(&mut dock, false, GamePlayMode::Maximized);
        assert!(!backup.has_saved());
        assert!(dock.find_tab(&EditorTab::Hierarchy).is_some());
    }

    /// Switching scenes while Play Maximized is active must leave Edit mode and restore docks.
    #[test]
    fn scene_switch_while_maximized_play_restores_dock() {
        let mut dock = default_unity_layout();
        let mut backup = PlayLayoutBackup::default();
        backup.sync(&mut dock, true, GamePlayMode::Maximized);
        assert!(backup.has_saved());

        // Same path as `open_scene_at` calling `stop_play` before loading the new scene.
        backup.sync(&mut dock, false, GamePlayMode::Maximized);
        assert!(!backup.has_saved());
        assert!(dock.find_tab(&EditorTab::Hierarchy).is_some());
        assert!(dock.find_tab(&EditorTab::Scene).is_some());
    }
}
