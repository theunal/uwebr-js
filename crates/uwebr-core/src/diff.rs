use crate::component::{Element, NodeType};

/// A patch operation to apply to the DOM
#[derive(Debug, Clone, PartialEq)]
pub enum Patch {
    /// Replace node at path with a new element
    Replace { path: Vec<usize>, new: Element },
    /// Update attributes on a node at path
    UpdateProps {
        path: Vec<usize>,
        props: Vec<(String, crate::component::PropValue)>,
    },
    /// Insert a child at a specific index
    Insert {
        path: Vec<usize>,
        index: usize,
        child: Element,
    },
    /// Remove a child at a specific index
    Remove { path: Vec<usize>, index: usize },
    /// Move a child from one index to another
    Move {
        path: Vec<usize>,
        from: usize,
        to: usize,
    },
    /// Update text content
    UpdateText { path: Vec<usize>, text: String },
}

/// Diff two Element trees and return a list of patches
pub fn diff(old: &Element, new: &Element) -> Vec<Patch> {
    let mut patches = Vec::new();
    diff_node(old, new, &mut vec![], &mut patches);
    patches
}

fn diff_node(old: &Element, new: &Element, path: &mut Vec<usize>, patches: &mut Vec<Patch>) {
    match (&old.node_type, &new.node_type) {
        // Both are elements — same tag
        (NodeType::Element(old_tag), NodeType::Element(new_tag)) if old_tag == new_tag => {
            diff_props(old, new, path, patches);
            diff_children(old, new, path, patches);
        }

        // Both are elements — different tag: replace
        (NodeType::Element(_), NodeType::Element(_)) => {
            patches.push(Patch::Replace {
                path: path.clone(),
                new: new.clone(),
            });
        }

        // Both are text nodes
        (NodeType::Text(old_text), NodeType::Text(new_text)) => {
            if old_text != new_text {
                patches.push(Patch::UpdateText {
                    path: path.clone(),
                    text: new_text.clone(),
                });
            }
            diff_props(old, new, path, patches);
        }

        // Both are component nodes
        (NodeType::Component(old_name), NodeType::Component(new_name)) => {
            if old_name == new_name {
                diff_props(old, new, path, patches);
                diff_children(old, new, path, patches);
            } else {
                patches.push(Patch::Replace {
                    path: path.clone(),
                    new: new.clone(),
                });
            }
        }

        // Both raw HTML
        (NodeType::Raw(_), NodeType::Raw(_)) => {
            if old != new {
                patches.push(Patch::Replace {
                    path: path.clone(),
                    new: new.clone(),
                });
            }
        }

        // Mixed types: replace
        _ => {
            patches.push(Patch::Replace {
                path: path.clone(),
                new: new.clone(),
            });
        }
    }
}

fn diff_props(old: &Element, new: &Element, path: &[usize], patches: &mut Vec<Patch>) {
    let old_props: std::collections::HashMap<_, _> = old.props.iter().cloned().collect();
    let new_props: std::collections::HashMap<_, _> = new.props.iter().cloned().collect();

    let mut changed = Vec::new();

    // Find updated or new props
    for (key, new_val) in &new_props {
        match old_props.get(key) {
            Some(old_val) if old_val == new_val => {} // unchanged
            _ => changed.push((key.clone(), new_val.clone())),
        }
    }

    // Find removed props (set to empty/default)
    for key in old_props.keys() {
        if !new_props.contains_key(key) {
            // Mark as removed by setting to empty string
            // In a real impl, we'd have a RemoveProp variant
            changed.push((
                key.clone(),
                crate::component::PropValue::String(String::new()),
            ));
        }
    }

    if !changed.is_empty() {
        patches.push(Patch::UpdateProps {
            path: path.to_vec(),
            props: changed,
        });
    }
}

fn diff_children(old: &Element, new: &Element, path: &mut Vec<usize>, patches: &mut Vec<Patch>) {
    let old_len = old.children.len();
    let new_len = new.children.len();
    let min_len = old_len.min(new_len);

    // Diff existing children
    for i in 0..min_len {
        path.push(i);
        diff_node(&old.children[i], &new.children[i], path, patches);
        path.pop();
    }

    // Remove extra old children
    if old_len > new_len {
        for i in (new_len..old_len).rev() {
            patches.push(Patch::Remove {
                path: path.clone(),
                index: i,
            });
        }
    }

    // Add new children
    if new_len > old_len {
        for i in old_len..new_len {
            patches.push(Patch::Insert {
                path: path.clone(),
                index: i,
                child: new.children[i].clone(),
            });
        }
    }
}

/// Apply a list of patches to an element tree
pub fn apply_patches(root: &mut Element, patches: &[Patch]) -> bool {
    let mut changed = false;
    for patch in patches {
        match patch {
            Patch::UpdateText { path, text } => {
                if let Some(node) = navigate_to(root, path) {
                    node.node_type = NodeType::Text(text.clone());
                    changed = true;
                }
            }
            Patch::UpdateProps { path, props } => {
                if let Some(node) = navigate_to(root, path) {
                    for (key, value) in props {
                        if value == &crate::component::PropValue::String(String::new()) {
                            node.props.retain(|(k, _)| k != key);
                        } else {
                            // Update or insert prop
                            if let Some(existing) = node.props.iter_mut().find(|(k, _)| k == key) {
                                existing.1 = value.clone();
                            } else {
                                node.props.push((key.clone(), value.clone()));
                            }
                        }
                    }
                    changed = true;
                }
            }
            Patch::Replace { path, new } => {
                if path.is_empty() {
                    *root = new.clone();
                    changed = true;
                } else if let Some(parent) = navigate_to(root, &path[..path.len() - 1]) {
                    let idx = *path.last().unwrap();
                    if idx < parent.children.len() {
                        parent.children[idx] = new.clone();
                        changed = true;
                    }
                }
            }
            Patch::Remove { path, index } => {
                if let Some(node) = navigate_to(root, path) {
                    if *index < node.children.len() {
                        node.children.remove(*index);
                        changed = true;
                    }
                }
            }
            Patch::Insert { path, index, child } => {
                if let Some(node) = navigate_to(root, path) {
                    let idx = (*index).min(node.children.len());
                    node.children.insert(idx, child.clone());
                    changed = true;
                }
            }
            Patch::Move { path, from, to } => {
                if let Some(node) = navigate_to(root, path) {
                    if *from < node.children.len() && *to < node.children.len() {
                        let child = node.children.remove(*from);
                        node.children.insert(*to, child);
                        changed = true;
                    }
                }
            }
        }
    }
    changed
}

/// Navigate to a node at a given path
fn navigate_to<'a>(root: &'a mut Element, path: &[usize]) -> Option<&'a mut Element> {
    let mut current = root;
    for &idx in path {
        current = current.children.get_mut(idx)?;
    }
    Some(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::{NodeType, PropValue};

    fn text_elem(text: &str) -> Element {
        Element {
            node_type: NodeType::Text(text.to_string()),
            props: vec![],
            children: vec![],
        }
    }

    fn div_elem(class: &str, children: Vec<Element>) -> Element {
        Element {
            node_type: NodeType::Element("div".to_string()),
            props: vec![("class".to_string(), PropValue::String(class.to_string()))],
            children,
        }
    }

    #[test]
    fn test_diff_same_text() {
        let old = text_elem("hello");
        let new = text_elem("hello");
        let patches = diff(&old, &new);
        assert!(patches.is_empty());
    }

    #[test]
    fn test_diff_different_text() {
        let old = text_elem("hello");
        let new = text_elem("world");
        let patches = diff(&old, &new);
        assert_eq!(patches.len(), 1);
        assert!(matches!(&patches[0], Patch::UpdateText { text, .. } if text == "world"));
    }

    #[test]
    fn test_diff_add_child() {
        let old = div_elem("container", vec![]);
        let new = div_elem("container", vec![text_elem("child")]);
        let patches = diff(&old, &new);
        assert_eq!(patches.len(), 1);
        assert!(matches!(&patches[0], Patch::Insert { index: 0, .. }));
    }

    #[test]
    fn test_diff_remove_child() {
        let old = div_elem("container", vec![text_elem("child")]);
        let new = div_elem("container", vec![]);
        let patches = diff(&old, &new);
        assert_eq!(patches.len(), 1);
        assert!(matches!(&patches[0], Patch::Remove { index: 0, .. }));
    }

    #[test]
    fn test_diff_update_prop() {
        let old = Element {
            node_type: NodeType::Element("div".to_string()),
            props: vec![("class".to_string(), PropValue::String("old".to_string()))],
            children: vec![],
        };
        let new = Element {
            node_type: NodeType::Element("div".to_string()),
            props: vec![("class".to_string(), PropValue::String("new".to_string()))],
            children: vec![],
        };
        let patches = diff(&old, &new);
        assert_eq!(patches.len(), 1);
        assert!(matches!(&patches[0], Patch::UpdateProps { .. }));
    }

    #[test]
    fn test_apply_text_update() {
        let mut root = text_elem("hello");
        let patches = vec![Patch::UpdateText {
            path: vec![],
            text: "world".to_string(),
        }];
        let changed = apply_patches(&mut root, &patches);
        assert!(changed);
        assert_eq!(root, text_elem("world"));
    }

    #[test]
    fn test_apply_insert_child() {
        let mut root = div_elem("container", vec![]);
        let patches = vec![Patch::Insert {
            path: vec![],
            index: 0,
            child: text_elem("new child"),
        }];
        let changed = apply_patches(&mut root, &patches);
        assert!(changed);
        assert_eq!(root.children.len(), 1);
    }

    #[test]
    fn test_nested_diff() {
        let old = div_elem("outer", vec![div_elem("inner", vec![text_elem("old")])]);
        let new = div_elem("outer", vec![div_elem("inner", vec![text_elem("new")])]);
        let patches = diff(&old, &new);
        assert_eq!(patches.len(), 1);
        match &patches[0] {
            Patch::UpdateText { path, text } => {
                assert_eq!(path, &[0, 0]);
                assert_eq!(text, "new");
            }
            _ => panic!("Expected UpdateText"),
        }
    }
}
