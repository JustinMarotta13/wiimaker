//! Pure layout and hit-testing for the Animator window graph.
//!
//! Missing positions use a deterministic auto-layout. Stored positions win.
//! Coordinates are graph space (top-left origin, +x right, +y down).

use serde::Serialize;

use crate::{
    is_any_state, is_base_layer_name, AnimatorControllerMeta, ControllerGraph, ControllerState,
    ControllerTransition, BASE_LAYER_NAME,
};

pub const STATE_W: f32 = 132.0;
pub const STATE_H: f32 = 28.0;
pub const SPECIAL_W: f32 = 108.0;
pub const SPECIAL_H: f32 = 22.0;
/// Hit slop around a transition segment, in graph units.
pub const TRANSITION_SLOP: f32 = 8.0;

#[derive(Clone, Debug, PartialEq)]
pub enum GraphNodeId {
    Entry,
    Any,
    Exit,
    State(String),
}

impl GraphNodeId {
    pub fn key(&self) -> &str {
        match self {
            Self::Entry => "Entry",
            Self::Any => "Any",
            Self::Exit => "Exit",
            Self::State(name) => name,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeRole {
    Entry,
    Any,
    Exit,
    State,
    Default,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphNode {
    pub id: GraphNodeId,
    pub label: String,
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub role: NodeRole,
    pub authored: bool,
    pub blend: bool,
}

impl GraphNode {
    pub fn center(&self) -> [f32; 2] {
        [
            self.pos[0] + self.size[0] * 0.5,
            self.pos[1] + self.size[1] * 0.5,
        ]
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.pos[0]
            && y >= self.pos[1]
            && x <= self.pos[0] + self.size[0]
            && y <= self.pos[1] + self.size[1]
    }
}

pub fn auto_entry() -> [f32; 2] {
    [8.0, 36.0]
}

pub fn auto_any() -> [f32; 2] {
    [8.0, 4.0]
}

pub fn auto_exit() -> [f32; 2] {
    [8.0, 68.0]
}

/// Column of states to the right of Entry / Any / Exit. Index 0 is the top.
pub fn auto_state_pos(index: usize) -> [f32; 2] {
    let col = (index / 4) as f32;
    let row = (index % 4) as f32;
    [150.0 + col * 160.0, 8.0 + row * 40.0]
}

fn finite_pos(p: [f32; 2]) -> bool {
    p[0].is_finite() && p[1].is_finite()
}

/// Lay out one layer. Special nodes first, then states in file order.
pub fn layout_layer(
    default_state: &str,
    states: &[ControllerState],
    graph: &ControllerGraph,
) -> Vec<GraphNode> {
    let mut nodes = Vec::new();
    let (any_pos, any_authored) = match graph.any {
        Some(p) if finite_pos(p) => (p, true),
        _ => (auto_any(), false),
    };
    let (entry_pos, entry_authored) = match graph.entry {
        Some(p) if finite_pos(p) => (p, true),
        _ => (auto_entry(), false),
    };
    let (exit_pos, exit_authored) = match graph.exit {
        Some(p) if finite_pos(p) => (p, true),
        _ => (auto_exit(), false),
    };
    nodes.push(special(
        GraphNodeId::Any,
        "Any State",
        any_pos,
        any_authored,
        NodeRole::Any,
    ));
    nodes.push(special(
        GraphNodeId::Entry,
        "Entry",
        entry_pos,
        entry_authored,
        NodeRole::Entry,
    ));
    nodes.push(special(
        GraphNodeId::Exit,
        "Exit",
        exit_pos,
        exit_authored,
        NodeRole::Exit,
    ));
    for (i, state) in states.iter().enumerate() {
        let (pos, authored) = match state.position {
            Some(p) if finite_pos(p) => (p, true),
            _ => (auto_state_pos(i), false),
        };
        let role = if state.name == default_state {
            NodeRole::Default
        } else {
            NodeRole::State
        };
        nodes.push(GraphNode {
            id: GraphNodeId::State(state.name.clone()),
            label: state.name.clone(),
            pos,
            size: [STATE_W, STATE_H],
            role,
            authored,
            blend: state.blend_tree.is_some(),
        });
    }
    nodes
}

fn special(
    id: GraphNodeId,
    label: &str,
    pos: [f32; 2],
    authored: bool,
    role: NodeRole,
) -> GraphNode {
    GraphNode {
        id,
        label: label.to_string(),
        pos,
        size: [SPECIAL_W, SPECIAL_H],
        role,
        authored,
        blend: false,
    }
}

/// Topmost node under the point. Later nodes win so states sit above Entry/Any/Exit.
pub fn hit_node(nodes: &[GraphNode], x: f32, y: f32) -> Option<GraphNodeId> {
    nodes
        .iter()
        .rev()
        .find(|n| n.contains(x, y))
        .map(|n| n.id.clone())
}

pub fn point_segment_dist(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let abx = b[0] - a[0];
    let aby = b[1] - a[1];
    let len2 = abx * abx + aby * aby;
    if len2 <= 1e-8 {
        let dx = p[0] - a[0];
        let dy = p[1] - a[1];
        return (dx * dx + dy * dy).sqrt();
    }
    let t = (((p[0] - a[0]) * abx + (p[1] - a[1]) * aby) / len2).clamp(0.0, 1.0);
    let qx = a[0] + abx * t;
    let qy = a[1] + aby * t;
    let dx = p[0] - qx;
    let dy = p[1] - qy;
    (dx * dx + dy * dy).sqrt()
}

fn node_by_key<'a>(nodes: &'a [GraphNode], key: &str) -> Option<&'a GraphNode> {
    nodes.iter().find(|n| n.id.key() == key)
}

/// Closest transition segment within `slop`. Entry→default is not a transition.
pub fn hit_transition(
    nodes: &[GraphNode],
    transitions: &[ControllerTransition],
    x: f32,
    y: f32,
    slop: f32,
) -> Option<usize> {
    let p = [x, y];
    let mut best: Option<(usize, f32)> = None;
    for (i, t) in transitions.iter().enumerate() {
        let from_key = if is_any_state(&t.from) {
            "Any"
        } else {
            t.from.as_str()
        };
        let Some(from) = node_by_key(nodes, from_key) else {
            continue;
        };
        let Some(to) = node_by_key(nodes, &t.to) else {
            continue;
        };
        let d = point_segment_dist(p, from.center(), to.center());
        if d <= slop && best.map(|(_, bd)| d < bd).unwrap_or(true) {
            best = Some((i, d));
        }
    }
    best.map(|(i, _)| i)
}

/// Arrowhead triangle at the midpoint of `a → b`, in graph space.
pub fn arrow_head(a: [f32; 2], b: [f32; 2], size: f32) -> [[f32; 2]; 3] {
    let mx = (a[0] + b[0]) * 0.5;
    let my = (a[1] + b[1]) * 0.5;
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let len = (dx * dx + dy * dy).sqrt().max(1e-4);
    let ux = dx / len;
    let uy = dy / len;
    let px = -uy;
    let py = ux;
    let tip = [mx + ux * size, my + uy * size];
    let left = [
        mx - ux * size * 0.6 + px * size * 0.55,
        my - uy * size * 0.6 + py * size * 0.55,
    ];
    let right = [
        mx - ux * size * 0.6 - px * size * 0.55,
        my - uy * size * 0.6 - py * size * 0.55,
    ];
    [tip, left, right]
}

pub fn unique_label(existing: &[String], base: &str) -> String {
    if !existing.iter().any(|n| n == base) {
        return base.to_string();
    }
    let mut i = 1u32;
    loop {
        let name = format!("{base} {i}");
        if !existing.iter().any(|n| n == &name) {
            return name;
        }
        i += 1;
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct GraphNodeDump {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub authored: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub blend: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct GraphCondDump {
    pub param: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub equals: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub greater: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub less: Option<f32>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GraphTransitionDump {
    pub index: usize,
    pub from: String,
    pub to: String,
    pub has_exit_time: bool,
    pub conditions: Vec<GraphCondDump>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LayerGraphView {
    pub controller: String,
    pub layer: String,
    #[serde(rename = "default")]
    pub default_state: String,
    pub weight: f32,
    pub nodes: Vec<GraphNodeDump>,
    pub transitions: Vec<GraphTransitionDump>,
}

/// Resolved graph for one layer, including auto-layout positions (`authored: false`).
pub fn layer_graph_view(
    controller: &str,
    meta: &AnimatorControllerMeta,
    layer: &str,
) -> anyhow::Result<LayerGraphView> {
    let (name, weight, default_state, states, transitions, graph) = if is_base_layer_name(layer) {
        (
            BASE_LAYER_NAME.to_string(),
            meta.weight,
            meta.default_state.as_str(),
            meta.states.as_slice(),
            meta.transitions.as_slice(),
            &meta.graph,
        )
    } else {
        let Some(found) = meta.layer(layer) else {
            let mut names = vec![BASE_LAYER_NAME.to_string()];
            names.extend(meta.layers.iter().map(|l| l.name.clone()));
            anyhow::bail!(
                "animator controller '{controller}' has no layer '{layer}' (valid: {})",
                names.join(", ")
            );
        };
        (
            found.name.clone(),
            found.weight,
            found.default_state.as_str(),
            found.states.as_slice(),
            found.transitions.as_slice(),
            &found.graph,
        )
    };
    let nodes = layout_layer(default_state, states, graph)
        .into_iter()
        .map(|n| GraphNodeDump {
            id: n.id.key().to_string(),
            x: n.pos[0],
            y: n.pos[1],
            authored: n.authored,
            blend: n.blend,
        })
        .collect();
    let transitions = transitions
        .iter()
        .enumerate()
        .map(|(index, t)| GraphTransitionDump {
            index,
            from: t.from.clone(),
            to: t.to.clone(),
            has_exit_time: t.has_exit_time,
            conditions: t
                .conditions
                .iter()
                .map(|c| GraphCondDump {
                    param: c.param.clone(),
                    equals: c.equals.clone(),
                    greater: c.greater,
                    less: c.less,
                })
                .collect(),
        })
        .collect();
    Ok(LayerGraphView {
        controller: controller.to_string(),
        layer: name,
        default_state: default_state.to_string(),
        weight,
        nodes,
        transitions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ControllerState;

    fn states(n: usize) -> Vec<ControllerState> {
        (0..n)
            .map(|i| ControllerState::plain(format!("S{i}"), "clip"))
            .collect()
    }

    #[test]
    fn auto_layout_is_deterministic_and_authored_positions_win() {
        let graph = ControllerGraph::default();
        let a = layout_layer("S0", &states(2), &graph);
        let b = layout_layer("S0", &states(2), &graph);
        assert_eq!(a, b);
        assert_eq!(a[0].pos, auto_any());
        assert_eq!(a[1].pos, auto_entry());
        assert_eq!(a[2].pos, auto_exit());
        assert!(!a[3].authored);
        assert_eq!(a[3].pos, auto_state_pos(0));
        assert_eq!(a[3].role, NodeRole::Default);
        assert_eq!(a[4].pos, auto_state_pos(1));
        assert_eq!(a[4].role, NodeRole::State);

        let mut custom = states(1);
        custom[0].position = Some([400.0, 12.0]);
        let mut g = ControllerGraph::default();
        g.entry = Some([3.0, 9.0]);
        let laid = layout_layer("S0", &custom, &g);
        assert_eq!(laid[1].pos, [3.0, 9.0]);
        assert!(laid[1].authored);
        assert_eq!(laid[3].pos, [400.0, 12.0]);
        assert!(laid[3].authored);
        assert_eq!(laid[0].pos, auto_any());
    }

    #[test]
    fn non_finite_stored_position_falls_back_to_auto_layout() {
        let mut custom = states(1);
        custom[0].position = Some([f32::NAN, 1.0]);
        let laid = layout_layer("S0", &custom, &ControllerGraph::default());
        assert_eq!(laid[3].pos, auto_state_pos(0));
        assert!(!laid[3].authored);
    }

    #[test]
    fn hit_node_picks_the_topmost_rect() {
        let nodes = layout_layer("S0", &states(1), &ControllerGraph::default());
        let s = &nodes[3];
        let hit = hit_node(&nodes, s.pos[0] + 2.0, s.pos[1] + 2.0).unwrap();
        assert_eq!(hit, GraphNodeId::State("S0".into()));
        assert!(hit_node(&nodes, -10.0, -10.0).is_none());
        let entry = &nodes[1];
        assert_eq!(
            hit_node(&nodes, entry.pos[0] + 1.0, entry.pos[1] + 1.0).unwrap(),
            GraphNodeId::Entry
        );
    }

    #[test]
    fn hit_transition_uses_segment_distance() {
        let mut sts = states(2);
        sts[0].position = Some([0.0, 0.0]);
        sts[1].position = Some([200.0, 0.0]);
        let nodes = layout_layer("", &sts, &ControllerGraph::default());
        let transitions = vec![ControllerTransition {
            from: "S0".into(),
            to: "S1".into(),
            conditions: vec![],
            has_exit_time: false,
        }];
        let a = nodes[3].center();
        let b = nodes[4].center();
        let mid = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
        assert_eq!(
            hit_transition(&nodes, &transitions, mid[0], mid[1] + 3.0, TRANSITION_SLOP),
            Some(0)
        );
        assert_eq!(
            hit_transition(&nodes, &transitions, mid[0], mid[1] + 40.0, TRANSITION_SLOP),
            None
        );
        let head = arrow_head(a, b, 8.0);
        assert!(head[0][0] > mid[0]);
    }

    #[test]
    fn any_transition_hits_the_any_node() {
        let sts = states(1);
        let nodes = layout_layer("S0", &sts, &ControllerGraph::default());
        let transitions = vec![ControllerTransition {
            from: "Any".into(),
            to: "S0".into(),
            conditions: vec![],
            has_exit_time: true,
        }];
        let a = nodes[0].center();
        let b = nodes[3].center();
        let mid = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
        assert_eq!(
            hit_transition(&nodes, &transitions, mid[0], mid[1], TRANSITION_SLOP),
            Some(0)
        );
    }
}
