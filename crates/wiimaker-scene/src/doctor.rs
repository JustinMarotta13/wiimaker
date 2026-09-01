//! Project health checks for agents and humans.

use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::project::GameProject;
use crate::scene::{load_scene, Scene};
use wiimaker_assets::{list_anim_clips, AnimClipMeta, SpriteCatalog};

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

    let catalog =
        SpriteCatalog::load_dir(&assets, |_| None).unwrap_or_else(|_| SpriteCatalog::empty());
    let mut sprite_names: Vec<String> = catalog.names().to_vec();
    if sprite_names.is_empty() {
        sprite_names = texture_names.clone();
    }


    let anim_names = list_anim_clips(&assets).unwrap_or_default();
    for aname in &anim_names {
        let path = AnimClipMeta::path(&assets, aname);
        match AnimClipMeta::load(&path) {
            Ok(meta) => {
                if meta.cells.is_empty() {
                    issues.push(Issue {
                        severity: Severity::Warning,
                        message: format!("anim '{aname}': no cells listed"),
                    });
                }
                for cell in &meta.cells {
                    if !sprite_names.iter().any(|t| t == cell) {
                        issues.push(Issue {
                            severity: Severity::Warning,
                            message: format!(
                                "anim '{aname}': cell '{cell}' missing from sprite catalog"
                            ),
                        });
                    }
                }
            }
            Err(e) => issues.push(Issue {
                severity: Severity::Error,
                message: format!("anim '{aname}': {e}"),
            }),
        }
    }

    if let Some(scene) = &scene {
        check_scene_refs(scene, &sprite_names, &anim_names, &assets, &mut issues);
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

    let ok = !issues.iter().any(|i| matches!(i.severity, Severity::Error));
    Diagnosis {
        game: project.name.clone(),
        ok,
        issues,
    }
}

fn check_scene_refs(
    scene: &Scene,
    sprites: &[String],
    anims: &[String],
    assets: &Path,
    issues: &mut Vec<Issue>,
) {
    for ent in &scene.entities {
        if let Some(sp) = &ent.components.sprite {
            if !sprites.iter().any(|t| t == &sp.texture) {
                issues.push(Issue {
                    severity: Severity::Error,
                    message: format!(
                        "entity '{}': sprite '{}' missing from assets/ (PNG stem or sheet cell)",
                        ent.name, sp.texture
                    ),
                });
            }
        }
        if let Some(tm) = &ent.components.tilemap {
            let n = (tm.width as usize).saturating_mul(tm.height as usize);
            if tm.cells.len() != n {
                issues.push(Issue {
                    severity: Severity::Warning,
                    message: format!(
                        "entity '{}': Tilemap cells len {} != {}x{} ({})",
                        ent.name,
                        tm.cells.len(),
                        tm.width,
                        tm.height,
                        n
                    ),
                });
            }
            for pal in &tm.palette {
                if let Some(sprite) = &pal.sprite {
                    if !sprites.iter().any(|t| t == sprite) {
                        issues.push(Issue {
                            severity: Severity::Error,
                            message: format!(
                                "entity '{}': tile palette id {} sprite '{}' missing from assets/",
                                ent.name, pal.id, sprite
                            ),
                        });
                    }
                }
            }
        }
        if let Some(c) = &ent.components.collider {
            let empty = match c.kind {
                crate::scene::SceneColliderKind::Aabb => c.size[0] <= 0.0 || c.size[1] <= 0.0,
                crate::scene::SceneColliderKind::Circle => c.radius <= 0.0,
            };
            if empty {
                issues.push(Issue {
                    severity: Severity::Warning,
                    message: format!("entity '{}': Collider has zero size", ent.name),
                });
            }
        }

        if let Some(a) = &ent.components.animation {
            if a.clip.is_empty() {
                issues.push(Issue {
                    severity: Severity::Warning,
                    message: format!("entity '{}': Animation has empty clip name", ent.name),
                });
            } else if !anims.iter().any(|n| n == &a.clip) {
                issues.push(Issue {
                    severity: Severity::Warning,
                    message: format!(
                        "entity '{}': Animation clip '{}' missing (expected assets/{}.anim.json)",
                        ent.name, a.clip, a.clip
                    ),
                });
            } else {
                let path = AnimClipMeta::path(assets, &a.clip);
                if let Ok(meta) = AnimClipMeta::load(&path) {
                    for cell in &meta.cells {
                        if !sprites.iter().any(|t| t == cell) {
                            issues.push(Issue {
                                severity: Severity::Warning,
                                message: format!(
                                    "entity '{}': Animation clip '{}' cell '{}' missing from assets/",
                                    ent.name, a.clip, cell
                                ),
                            });
                        }
                    }
                }
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
