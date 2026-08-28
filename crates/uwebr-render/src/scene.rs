use anyhow::Result;

/// Scene graph for rendering
pub struct Scene {
    nodes: Vec<SceneNode>,
}

#[derive(Debug, Clone)]
pub struct SceneNode {
    pub id: u64,
    pub kind: SceneNodeKind,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone)]
pub enum SceneNodeKind {
    Rect { color: [f32; 4] },
    Text { content: String, font_size: f32 },
    Image { url: String },
}

impl Scene {
    pub fn new() -> Self {
        Self { nodes: vec![] }
    }

    pub fn add_node(&mut self, node: SceneNode) {
        self.nodes.push(node);
    }

    pub fn clear(&mut self) {
        self.nodes.clear();
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scene_add_node() {
        let mut scene = Scene::new();
        scene.add_node(SceneNode {
            id: 1,
            kind: SceneNodeKind::Rect {
                color: [1.0, 0.0, 0.0, 1.0],
            },
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 50.0,
        });
        assert_eq!(scene.node_count(), 1);
    }
}
