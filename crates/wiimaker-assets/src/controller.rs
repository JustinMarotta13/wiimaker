//! Animator controller sidecars (`assets/<name>.controller.json`).
//!
//! A controller is a small state machine: named clip states, Bool/Float/Trigger
//! parameters, and transitions. Clip cells stay in `*.anim.json`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

fn default_speed() -> f32 {
    1.0
}

/// Parameter kind on an animator controller.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControllerParamType {
    Bool,
    Float,
    Trigger,
}

impl ControllerParamType {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "bool" | "boolean" => Some(Self::Bool),
            "float" | "number" => Some(Self::Float),
            "trigger" => Some(Self::Trigger),
            _ => None,
        }
    }
}

/// Authoring parameter with a default value.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ControllerParam {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: ControllerParamType,
    /// Bool / trigger default, or omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
}

impl ControllerParam {
    pub fn default_bool(&self) -> bool {
        match self.kind {
            ControllerParamType::Bool | ControllerParamType::Trigger => self
                .default
                .as_ref()
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            ControllerParamType::Float => false,
        }
    }

    pub fn default_float(&self) -> f32 {
        match self.kind {
            ControllerParamType::Float => self
                .default
                .as_ref()
                .and_then(|v| v.as_f64())
                .map(|f| f as f32)
                .unwrap_or(0.0),
            ControllerParamType::Bool | ControllerParamType::Trigger => 0.0,
        }
    }
}

/// One named state → clip stem.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ControllerState {
    pub name: String,
    pub clip: String,
    #[serde(default = "default_speed")]
    pub speed: f32,
}

/// Transition condition (AND together on a transition).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ControllerCondition {
    pub param: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equals: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub greater: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub less: Option<f32>,
}

/// Directed edge between states. `from` may be `"Any"`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ControllerTransition {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub conditions: Vec<ControllerCondition>,
    #[serde(default)]
    pub has_exit_time: bool,
}

/// Authoring animator controller (`assets/<name>.controller.json`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnimatorControllerMeta {
    #[serde(rename = "default")]
    pub default_state: String,
    #[serde(default)]
    pub parameters: Vec<ControllerParam>,
    pub states: Vec<ControllerState>,
    #[serde(default)]
    pub transitions: Vec<ControllerTransition>,
}

impl AnimatorControllerMeta {
    pub fn path(assets_dir: impl AsRef<Path>, name: &str) -> PathBuf {
        assets_dir.as_ref().join(format!("{name}.controller.json"))
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let meta: Self =
            serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
        meta.validate()?;
        Ok(meta)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        self.validate()?;
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)?;
        fs::write(path, text + "\n").with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }

    pub fn state(&self, name: &str) -> Option<&ControllerState> {
        self.states.iter().find(|s| s.name == name)
    }

    pub fn validate(&self) -> Result<()> {
        if self.states.is_empty() {
            bail!("animator controller must list at least one state");
        }
        let mut seen = HashMap::new();
        for s in &self.states {
            if s.name.is_empty() {
                bail!("animator state name must not be empty");
            }
            if s.clip.is_empty() {
                bail!("animator state '{}' has empty clip", s.name);
            }
            if seen.insert(s.name.as_str(), ()).is_some() {
                bail!("duplicate animator state '{}'", s.name);
            }
        }
        if self.default_state.is_empty() {
            bail!("animator controller default state must not be empty");
        }
        if self.state(&self.default_state).is_none() {
            bail!(
                "animator default state '{}' is not in states",
                self.default_state
            );
        }
        let mut pseen = HashMap::new();
        for p in &self.parameters {
            if p.name.is_empty() {
                bail!("animator parameter name must not be empty");
            }
            if pseen.insert(p.name.as_str(), ()).is_some() {
                bail!("duplicate animator parameter '{}'", p.name);
            }
        }
        for t in &self.transitions {
            if !is_any_state(&t.from) && self.state(&t.from).is_none() {
                bail!("animator transition from unknown state '{}'", t.from);
            }
            if self.state(&t.to).is_none() {
                bail!("animator transition to unknown state '{}'", t.to);
            }
        }
        Ok(())
    }
}

/// `Any` / `*` matches every from-state at runtime.
pub fn is_any_state(name: &str) -> bool {
    let n = name.trim();
    n.eq_ignore_ascii_case("any") || n == "*"
}

/// Write / overwrite `assets/<name>.controller.json`.
pub fn write_animator_controller(
    assets_dir: &Path,
    name: &str,
    meta: AnimatorControllerMeta,
) -> Result<(PathBuf, AnimatorControllerMeta)> {
    if name.is_empty() {
        bail!("animator controller name must not be empty");
    }
    meta.validate()?;
    let path = AnimatorControllerMeta::path(assets_dir, name);
    meta.save(&path)?;
    Ok((path, meta))
}

/// Stem names of every `*.controller.json` under `assets_dir`.
pub fn list_animator_controllers(assets_dir: &Path) -> Result<Vec<String>> {
    let mut names = Vec::new();
    if !assets_dir.is_dir() {
        return Ok(names);
    }
    for entry in fs::read_dir(assets_dir)? {
        let path = entry?.path();
        let fname = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if let Some(stem) = fname.strip_suffix(".controller.json") {
            names.push(stem.to_string());
        }
    }
    names.sort();
    Ok(names)
}

/// Lookup table of animator controllers by stem.
#[derive(Clone, Debug, Default)]
pub struct AnimatorControllerCatalog {
    by_name: HashMap<String, AnimatorControllerMeta>,
    names: Vec<String>,
}

impl AnimatorControllerCatalog {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn names(&self) -> &[String] {
        &self.names
    }

    pub fn lookup(&self, name: &str) -> Option<&AnimatorControllerMeta> {
        self.by_name.get(name)
    }

    pub fn load_dir(assets_dir: &Path) -> Result<Self> {
        let mut cat = Self::empty();
        if !assets_dir.is_dir() {
            return Ok(cat);
        }
        for name in list_animator_controllers(assets_dir)? {
            let path = AnimatorControllerMeta::path(assets_dir, &name);
            let meta = AnimatorControllerMeta::load(&path)?;
            cat.names.push(name.clone());
            cat.by_name.insert(name, meta);
        }
        Ok(cat)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn sample() -> AnimatorControllerMeta {
        AnimatorControllerMeta {
            default_state: "Idle".into(),
            parameters: vec![ControllerParam {
                name: "Moving".into(),
                kind: ControllerParamType::Bool,
                default: Some(serde_json::json!(false)),
            }],
            states: vec![
                ControllerState {
                    name: "Idle".into(),
                    clip: "idle".into(),
                    speed: 1.0,
                },
                ControllerState {
                    name: "Walk".into(),
                    clip: "walk".into(),
                    speed: 1.0,
                },
            ],
            transitions: vec![
                ControllerTransition {
                    from: "Idle".into(),
                    to: "Walk".into(),
                    conditions: vec![ControllerCondition {
                        param: "Moving".into(),
                        equals: Some(serde_json::json!(true)),
                        ..Default::default()
                    }],
                    has_exit_time: false,
                },
                ControllerTransition {
                    from: "Walk".into(),
                    to: "Idle".into(),
                    conditions: vec![ControllerCondition {
                        param: "Moving".into(),
                        equals: Some(serde_json::json!(false)),
                        ..Default::default()
                    }],
                    has_exit_time: false,
                },
            ],
        }
    }

    #[test]
    fn roundtrip_controller_json() {
        let dir = env::temp_dir().join(format!("wiimaker-ctrl-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let (path, meta) = write_animator_controller(&dir, "player", sample()).unwrap();
        assert!(path.ends_with("player.controller.json"));
        assert_eq!(meta.default_state, "Idle");
        let loaded = AnimatorControllerMeta::load(&path).unwrap();
        assert_eq!(loaded.states.len(), 2);
        assert_eq!(loaded.parameters[0].default_bool(), false);
        let names = list_animator_controllers(&dir).unwrap();
        assert_eq!(names, vec!["player"]);
        let cat = AnimatorControllerCatalog::load_dir(&dir).unwrap();
        assert!(cat.lookup("player").is_some());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_unknown_default_state() {
        let mut m = sample();
        m.default_state = "Nope".into();
        assert!(m.validate().is_err());
    }
}
