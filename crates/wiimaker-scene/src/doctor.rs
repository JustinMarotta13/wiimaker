//! Project health checks for agents and humans.

use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::project::GameProject;
use crate::scene::{load_scene, Scene};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Clone, Debug, Serialize)]
pub struct Issue {
    pub severity: Severity,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Diagnosis {
    pub game: String,
    pub ok: bool,
    pub issues: Vec<Issue>,
}

pub fn diagnose(game_dir: &Path, project: &GameProject) -> Diagnosis {
    let mut issues = Vec::new();

    let scene_path = project.scene_path(game_dir);
    let scene = match load_scene(&scene_path) {
        Ok(s) => Some(s),
        Err(e) => {
            issues.push(Issue {
                severity: Severity::Error,
                message: format!("scene {}: {e}", scene_path.display()),
            });
            None
        }
    };

    let assets = project.assets_path(game_dir);
    let mut texture_names = Vec::new();
    if assets.is_dir() {
        match fs::read_dir(&assets) {
            Ok(rd) => {
                for entry in rd.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) != Some("png") {
                        continue;
                    }
                    let stem = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("tex")
                        .to_string();
                    texture_names.push(stem.clone());
                    match image::image_dimensions(&path) {
                        Ok((w, h)) => {
                            if !w.is_power_of_two() || !h.is_power_of_two() {
                                issues.push(Issue {
                                    severity: Severity::Warning,
                                    message: format!(
                                        "{stem}.png is {w}x{h} (not power-of-two); cook will pad"
                                    ),
                                });
                            }
                        }
                        Err(e) => issues.push(Issue {
                            severity: Severity::Error,
                            message: format!("cannot read {stem}.png: {e}"),
                        }),
                    }
                }
            }
            Err(e) => issues.push(Issue {
                severity: Severity::Error,
                message: format!("assets dir {}: {e}", assets.display()),
            }),
        }
    } else {
        issues.push(Issue {
            severity: Severity::Warning,
            message: format!("assets dir missing: {}", assets.display()),
        });
    }

    if let Some(scene) = &scene {
        check_scene_refs(scene, &texture_names, &mut issues);
    }

    let wpack = project.wpack_path(game_dir);
    if !wpack.is_file() {
        issues.push(Issue {
            severity: Severity::Info,
            message: format!(
                "no cooked wpack at {} — run `wiimaker cook {}`",
                wpack.display(),
                project.name
            ),
        });
    }

    let ok = !issues
        .iter()
        .any(|i| matches!(i.severity, Severity::Error));
    Diagnosis {
        game: project.name.clone(),
        ok,
        issues,
    }
}

fn check_scene_refs(scene: &Scene, textures: &[String], issues: &mut Vec<Issue>) {
    for ent in &scene.entities {
        if let Some(sp) = &ent.components.sprite {
            if !textures.iter().any(|t| t == &sp.texture) {
                issues.push(Issue {
                    severity: Severity::Error,
                    message: format!(
                        "entity '{}': sprite texture '{}' missing from assets/",
                        ent.name, sp.texture
                    ),
                });
            }
        }
    }
    let mut names = std::collections::HashSet::new();
    for ent in &scene.entities {
        if !names.insert(ent.name.clone()) {
            issues.push(Issue {
                severity: Severity::Error,
                message: format!("duplicate entity name '{}'", ent.name),
            });
        }
    }
}
