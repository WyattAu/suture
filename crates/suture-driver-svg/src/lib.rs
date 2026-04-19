use std::collections::{HashMap, HashSet};

use suture_driver::{DriverError, SemanticChange, SutureDriver};

pub struct SvgDriver;

impl SvgDriver {
    pub fn new() -> Self {
        Self
    }

    fn escape_xml(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }

    fn node_id<'a, 'b>(node: roxmltree::Node<'a, 'b>) -> Option<&'a str> {
        node.attribute("id")
    }

    fn node_path(node: roxmltree::Node) -> String {
        let mut parts: Vec<String> = Vec::new();
        let mut current = node;

        while current.is_element() {
            let tag = current.tag_name().name();
            if let Some(id) = Self::node_id(current) {
                parts.push(format!("{tag}[@id='{id}']"));
            } else if let Some(p) = current.parent() {
                if p.is_element() {
                    let mut idx = 0u32;
                    for child in p.children() {
                        if child.is_element() && child.tag_name().name() == tag {
                            idx += 1;
                            if child == current {
                                break;
                            }
                        }
                    }
                    parts.push(format!("{tag}[{idx}]"));
                } else {
                    parts.push(tag.to_string());
                }
            } else {
                parts.push(tag.to_string());
            }
            current = match current.parent() {
                Some(p) if p.is_element() => p,
                _ => break,
            };
        }

        parts.reverse();
        format!("/{}", parts.join("/"))
    }

    fn element_to_string(node: roxmltree::Node, indent: usize) -> String {
        let pad = "  ".repeat(indent);
        let tag = node.tag_name().name();

        let attrs: Vec<String> = node
            .attributes()
            .map(|a| format!("{}=\"{}\"", a.name(), Self::escape_xml(a.value())))
            .collect();

        let attr_str = if attrs.is_empty() {
            String::new()
        } else {
            format!(" {}", attrs.join(" "))
        };

        let text = node.text().unwrap_or("").trim();
        let element_children: Vec<roxmltree::Node> =
            node.children().filter(|n| n.is_element()).collect();

        if element_children.is_empty() && text.is_empty() {
            format!("{pad}<{tag}{attr_str}/>")
        } else if element_children.is_empty() {
            format!("{pad}<{tag}{attr_str}>{}</{tag}>", Self::escape_xml(text))
        } else {
            let mut result = format!("{pad}<{tag}{attr_str}>\n");
            if !text.is_empty() {
                result.push_str(&format!(
                    "{}{}\n",
                    "  ".repeat(indent + 1),
                    Self::escape_xml(text)
                ));
            }
            for child in &element_children {
                result.push_str(&Self::element_to_string(*child, indent + 1));
                result.push('\n');
            }
            result.push_str(&format!("{pad}</{tag}>"));
            result
        }
    }

    fn diff_nodes(old: roxmltree::Node, new: roxmltree::Node) -> Vec<SemanticChange> {
        let mut changes = Vec::new();

        if old.tag_name().name() != new.tag_name().name() {
            changes.push(SemanticChange::Modified {
                path: Self::node_path(new),
                old_value: old.tag_name().name().to_string(),
                new_value: new.tag_name().name().to_string(),
            });
            return changes;
        }

        let path = Self::node_path(new);

        let old_attrs: HashMap<&str, &str> =
            old.attributes().map(|a| (a.name(), a.value())).collect();
        let new_attrs: HashMap<&str, &str> =
            new.attributes().map(|a| (a.name(), a.value())).collect();

        let old_keys: HashSet<&str> = old_attrs.keys().copied().collect();
        let new_keys: HashSet<&str> = new_attrs.keys().copied().collect();

        for key in &old_keys {
            if !new_keys.contains(key) {
                changes.push(SemanticChange::Removed {
                    path: format!("{path}@{key}"),
                    old_value: old_attrs[key].to_string(),
                });
            }
        }

        for key in &new_keys {
            if !old_keys.contains(key) {
                changes.push(SemanticChange::Added {
                    path: format!("{path}@{key}"),
                    value: new_attrs[key].to_string(),
                });
            }
        }

        for key in &old_keys {
            if let Some(&new_val) = new_attrs.get(key)
                && old_attrs[key] != new_val
            {
                changes.push(SemanticChange::Modified {
                    path: format!("{path}@{key}"),
                    old_value: old_attrs[key].to_string(),
                    new_value: new_val.to_string(),
                });
            }
        }

        let old_text = old.text().unwrap_or("").trim();
        let new_text = new.text().unwrap_or("").trim();
        if old_text != new_text {
            changes.push(SemanticChange::Modified {
                path: format!("{path}#text"),
                old_value: old_text.to_string(),
                new_value: new_text.to_string(),
            });
        }

        let old_children: Vec<roxmltree::Node> =
            old.children().filter(|n| n.is_element()).collect();
        let new_children: Vec<roxmltree::Node> =
            new.children().filter(|n| n.is_element()).collect();

        let old_id_map: HashMap<String, usize> = old_children
            .iter()
            .enumerate()
            .filter_map(|(i, c)| Self::node_id(*c).map(|id: &str| (id.to_string(), i)))
            .collect();
        let new_id_map: HashMap<String, usize> = new_children
            .iter()
            .enumerate()
            .filter_map(|(i, c)| Self::node_id(*c).map(|id: &str| (id.to_string(), i)))
            .collect();

        let mut new_matched: HashSet<usize> = HashSet::new();
        let mut old_matched: HashSet<usize> = HashSet::new();

        for (id, new_idx) in &new_id_map {
            if let Some(&old_idx) = old_id_map.get(id) {
                changes.extend(Self::diff_nodes(old_children[old_idx], new_children[*new_idx]));
                new_matched.insert(*new_idx);
                old_matched.insert(old_idx);
            }
        }

        for (i, new_c) in new_children.iter().enumerate() {
            if new_matched.contains(&i) {
                continue;
            }
            if old_id_map.contains_key(&Self::node_id(*new_c).unwrap_or_default().to_string()) {
                continue;
            }
            changes.push(SemanticChange::Added {
                path: Self::node_path(*new_c),
                value: Self::element_to_string(*new_c, 0),
            });
        }

        for (i, old_c) in old_children.iter().enumerate() {
            if old_matched.contains(&i) {
                continue;
            }
            if new_id_map.contains_key(&Self::node_id(*old_c).unwrap_or_default().to_string()) {
                continue;
            }
            changes.push(SemanticChange::Removed {
                path: Self::node_path(*old_c),
                old_value: Self::element_to_string(*old_c, 0),
            });
        }

        changes
    }

    fn merge_elements(
        base: roxmltree::Node,
        ours: roxmltree::Node,
        theirs: roxmltree::Node,
        indent: usize,
    ) -> Result<Option<String>, DriverError> {
        let ours_tag = ours.tag_name().name();
        let theirs_tag = theirs.tag_name().name();

        if ours_tag != theirs_tag {
            return Ok(None);
        }
        let tag = ours_tag;

        let base_text = base.text().unwrap_or("").trim();
        let ours_text = ours.text().unwrap_or("").trim();
        let theirs_text = theirs.text().unwrap_or("").trim();

        let merged_text = if ours_text == theirs_text {
            ours_text.to_string()
        } else if ours_text == base_text {
            theirs_text.to_string()
        } else if theirs_text == base_text {
            ours_text.to_string()
        } else {
            return Ok(None);
        };

        let base_attrs: HashMap<&str, &str> =
            base.attributes().map(|a| (a.name(), a.value())).collect();
        let ours_attrs: HashMap<&str, &str> =
            ours.attributes().map(|a| (a.name(), a.value())).collect();
        let theirs_attrs: HashMap<&str, &str> =
            theirs.attributes().map(|a| (a.name(), a.value())).collect();

        let all_attr_keys: HashSet<&str> = base_attrs
            .keys()
            .chain(ours_attrs.keys())
            .chain(theirs_attrs.keys())
            .copied()
            .collect();

        let mut merged_attrs: Vec<(String, String)> = Vec::new();

        for key in &all_attr_keys {
            let bv = base_attrs.get(key).copied();
            let ov = ours_attrs.get(key).copied();
            let tv = theirs_attrs.get(key).copied();

            match (bv, ov, tv) {
                (_, Some(o), Some(t)) if o == t => {
                    merged_attrs.push((key.to_string(), o.to_string()));
                }
                (Some(b), Some(o), Some(t)) if o == b => {
                    merged_attrs.push((key.to_string(), t.to_string()));
                }
                (Some(b), Some(o), Some(t)) if t == b => {
                    merged_attrs.push((key.to_string(), o.to_string()));
                }
                (Some(_), Some(_), Some(_)) => return Ok(None),
                (Some(_), None, Some(t)) => {
                    merged_attrs.push((key.to_string(), t.to_string()));
                }
                (Some(_), Some(o), None) => {
                    merged_attrs.push((key.to_string(), o.to_string()));
                }
                (Some(_), None, None) => {}
                (None, Some(o), Some(t)) if o == t => {
                    merged_attrs.push((key.to_string(), o.to_string()));
                }
                (None, Some(_), Some(_)) => return Ok(None),
                (None, Some(o), None) => {
                    merged_attrs.push((key.to_string(), o.to_string()));
                }
                (None, None, Some(t)) => {
                    merged_attrs.push((key.to_string(), t.to_string()));
                }
                (None, None, None) => {}
            }
        }

        let base_children: Vec<roxmltree::Node> =
            base.children().filter(|n| n.is_element()).collect();
        let ours_children: Vec<roxmltree::Node> =
            ours.children().filter(|n| n.is_element()).collect();
        let theirs_children: Vec<roxmltree::Node> =
            theirs.children().filter(|n| n.is_element()).collect();

        let base_id_map: HashMap<String, usize> = base_children
            .iter()
            .enumerate()
            .filter_map(|(i, c)| Self::node_id(*c).map(|id: &str| (id.to_string(), i)))
            .collect();
        let ours_id_map: HashMap<String, usize> = ours_children
            .iter()
            .enumerate()
            .filter_map(|(i, c)| Self::node_id(*c).map(|id: &str| (id.to_string(), i)))
            .collect();
        let theirs_id_map: HashMap<String, usize> = theirs_children
            .iter()
            .enumerate()
            .filter_map(|(i, c)| Self::node_id(*c).map(|id: &str| (id.to_string(), i)))
            .collect();

        let mut merged_children: Vec<String> = Vec::new();
        let mut ours_used: HashSet<usize> = HashSet::new();
        let mut theirs_used: HashSet<usize> = HashSet::new();

        for (id, &base_idx) in &base_id_map {
            let ours_idx = ours_id_map.get(id).copied();
            let theirs_idx = theirs_id_map.get(id).copied();

            match (ours_idx, theirs_idx) {
                (Some(oi), Some(ti)) => {
                    ours_used.insert(oi);
                    theirs_used.insert(ti);
                    if let Some(merged) =
                        Self::merge_elements(base_children[base_idx], ours_children[oi], theirs_children[ti], indent + 1)?
                    {
                        merged_children.push(merged);
                    } else {
                        return Ok(None);
                    }
                }
                (Some(oi), None) => {
                    ours_used.insert(oi);
                }
                (None, Some(ti)) => {
                    theirs_used.insert(ti);
                }
                (None, None) => {}
            }
        }

        for (id, &oi) in &ours_id_map {
            if ours_used.contains(&oi) {
                continue;
            }
            if base_id_map.contains_key(id) {
                continue;
            }
            if let Some(&ti) = theirs_id_map.get(id) {
                theirs_used.insert(ti);
                if ours_children[oi].tag_name().name() == theirs_children[ti].tag_name().name()
                    && Self::element_to_string(ours_children[oi], 0)
                        == Self::element_to_string(theirs_children[ti], 0)
                {
                    merged_children.push(Self::element_to_string(ours_children[oi], indent + 1));
                } else {
                    merged_children
                        .push(Self::element_to_string(ours_children[oi], indent + 1));
                    merged_children
                        .push(Self::element_to_string(theirs_children[ti], indent + 1));
                }
            } else {
                merged_children.push(Self::element_to_string(ours_children[oi], indent + 1));
            }
        }

        for (id, &ti) in &theirs_id_map {
            if theirs_used.contains(&ti) {
                continue;
            }
            if base_id_map.contains_key(id) {
                continue;
            }
            if ours_id_map.contains_key(id) {
                continue;
            }
            merged_children.push(Self::element_to_string(theirs_children[ti], indent + 1));
        }

        let pad = "  ".repeat(indent);
        let attr_str = if merged_attrs.is_empty() {
            String::new()
        } else {
            let attrs: Vec<String> = merged_attrs
                .iter()
                .map(|(k, v)| format!("{k}=\"{}\"", Self::escape_xml(v)))
                .collect();
            format!(" {}", attrs.join(" "))
        };

        if merged_children.is_empty() && merged_text.is_empty() {
            Ok(Some(format!("{pad}<{tag}{attr_str}/>")))
        } else if merged_children.is_empty() {
            Ok(Some(format!(
                "{pad}<{tag}{attr_str}>{}</{tag}>",
                Self::escape_xml(&merged_text)
            )))
        } else {
            let mut result = format!("{pad}<{tag}{attr_str}>\n");
            if !merged_text.is_empty() {
                result.push_str(&format!(
                    "{}{}\n",
                    "  ".repeat(indent + 1),
                    Self::escape_xml(&merged_text)
                ));
            }
            for child in &merged_children {
                result.push_str(child);
                result.push('\n');
            }
            result.push_str(&format!("{pad}</{tag}>"));
            Ok(Some(result))
        }
    }

    fn format_change(change: &SemanticChange) -> String {
        match change {
            SemanticChange::Added { path, value } => {
                format!("  ADDED     {path}: {value}")
            }
            SemanticChange::Removed { path, old_value } => {
                format!("  REMOVED   {path}: {old_value}")
            }
            SemanticChange::Modified {
                path,
                old_value,
                new_value,
            } => {
                format!("  MODIFIED  {path}: {old_value} -> {new_value}")
            }
            SemanticChange::Moved {
                old_path,
                new_path,
                value,
            } => {
                format!("  MOVED     {old_path} -> {new_path}: {value}")
            }
        }
    }
}

impl Default for SvgDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl SutureDriver for SvgDriver {
    fn name(&self) -> &str {
        "SVG"
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".svg"]
    }

    fn diff(
        &self,
        base_content: Option<&str>,
        new_content: &str,
    ) -> Result<Vec<SemanticChange>, DriverError> {
        let new_doc = roxmltree::Document::parse(new_content)
            .map_err(|e| DriverError::ParseError(e.to_string()))?;

        match base_content {
            None => {
                let mut changes = Vec::new();
                collect_all_paths(new_doc.root_element(), &mut changes);
                Ok(changes)
            }
            Some(base) => {
                let old_doc = roxmltree::Document::parse(base)
                    .map_err(|e| DriverError::ParseError(e.to_string()))?;
                Ok(Self::diff_nodes(old_doc.root_element(), new_doc.root_element()))
            }
        }
    }

    fn format_diff(
        &self,
        base_content: Option<&str>,
        new_content: &str,
    ) -> Result<String, DriverError> {
        let changes = self.diff(base_content, new_content)?;

        if changes.is_empty() {
            return Ok("no changes".to_string());
        }

        let lines: Vec<String> = changes.iter().map(Self::format_change).collect();
        Ok(lines.join("\n"))
    }

    fn merge(&self, base: &str, ours: &str, theirs: &str) -> Result<Option<String>, DriverError> {
        let base_doc =
            roxmltree::Document::parse(base).map_err(|e| DriverError::ParseError(e.to_string()))?;
        let ours_doc =
            roxmltree::Document::parse(ours).map_err(|e| DriverError::ParseError(e.to_string()))?;
        let theirs_doc = roxmltree::Document::parse(theirs)
            .map_err(|e| DriverError::ParseError(e.to_string()))?;

        let mut result = String::new();
        result.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        if let Some(merged) =
            Self::merge_elements(base_doc.root_element(), ours_doc.root_element(), theirs_doc.root_element(), 0)?
        {
            result.push_str(&merged);
            result.push('\n');
            Ok(Some(result))
        } else {
            Ok(None)
        }
    }
}

fn collect_all_paths(node: roxmltree::Node, out: &mut Vec<SemanticChange>) {
    if !node.is_element() {
        return;
    }

    let path = SvgDriver::node_path(node);

    for attr in node.attributes() {
        out.push(SemanticChange::Added {
            path: format!("{path}@{}", attr.name()),
            value: attr.value().to_string(),
        });
    }

    let text = node.text().unwrap_or("").trim();
    if !text.is_empty() {
        out.push(SemanticChange::Added {
            path: format!("{path}#text"),
            value: text.to_string(),
        });
    }

    for child in node.children() {
        if child.is_element() {
            collect_all_paths(child, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_svg_file() {
        let driver = SvgDriver::new();
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect id="box1" x="10" y="10" width="80" height="80" fill="red"/></svg>"#;
        let changes = driver.diff(None, svg).unwrap();
        assert!(!changes.is_empty());
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Added { path, .. } if path.contains("svg") && path.ends_with("@width")
        )));
    }

    #[test]
    fn test_simple_attribute_change() {
        let driver = SvgDriver::new();
        let base = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect id="box1" x="10" y="10" width="80" height="80" fill="red"/></svg>"#;
        let modified = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect id="box1" x="10" y="10" width="80" height="80" fill="blue"/></svg>"#;
        let changes = driver.diff(Some(base), modified).unwrap();
        assert!(changes.contains(&SemanticChange::Modified {
            path: "/svg/rect[@id='box1']@fill".to_string(),
            old_value: "red".to_string(),
            new_value: "blue".to_string(),
        }));
    }

    #[test]
    fn test_element_addition() {
        let driver = SvgDriver::new();
        let base = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect id="box1" x="10" y="10" width="80" height="80" fill="red"/></svg>"#;
        let modified = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect id="box1" x="10" y="10" width="80" height="80" fill="red"/><circle id="circ1" cx="50" cy="50" r="20" fill="green"/></svg>"#;
        let changes = driver.diff(Some(base), modified).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Added { path, .. } if path == "/svg/circle[@id='circ1']"
        )));
    }

    #[test]
    fn test_element_removal() {
        let driver = SvgDriver::new();
        let base = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect id="box1" x="10" y="10" width="80" height="80" fill="red"/><circle id="circ1" cx="50" cy="50" r="20" fill="green"/></svg>"#;
        let modified = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect id="box1" x="10" y="10" width="80" height="80" fill="red"/></svg>"#;
        let changes = driver.diff(Some(base), modified).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Removed { path, .. } if path == "/svg/circle[@id='circ1']"
        )));
    }

    #[test]
    fn test_merge_clean_different_attributes() {
        let driver = SvgDriver::new();
        let base = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect id="box1" x="10" y="10" width="80" height="80" fill="red"/></svg>"#;
        let ours = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect id="box1" x="10" y="10" width="80" height="80" fill="blue"/></svg>"#;
        let theirs = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><rect id="box1" x="10" y="10" width="80" height="80" fill="red"/></svg>"#;
        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("fill=\"blue\""));
        assert!(merged.contains("width=\"200\""));
    }

    #[test]
    fn test_merge_conflict_same_attribute() {
        let driver = SvgDriver::new();
        let base = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect id="box1" x="10" y="10" width="80" height="80" fill="red"/></svg>"#;
        let ours = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect id="box1" x="10" y="10" width="80" height="80" fill="blue"/></svg>"#;
        let theirs = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect id="box1" x="10" y="10" width="80" height="80" fill="green"/></svg>"#;
        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_nested_group_element_diff() {
        let driver = SvgDriver::new();
        let base = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><g id="group1"><rect id="box1" x="0" y="0" width="50" height="50" fill="red"/></g></svg>"#;
        let modified = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><g id="group1" opacity="0.5"><rect id="box1" x="0" y="0" width="50" height="50" fill="red"/></g></svg>"#;
        let changes = driver.diff(Some(base), modified).unwrap();
        assert!(changes.contains(&SemanticChange::Added {
            path: "/svg/g[@id='group1']@opacity".to_string(),
            value: "0.5".to_string(),
        }));
    }

    #[test]
    fn test_path_element_d_attribute_change() {
        let driver = SvgDriver::new();
        let base = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><path id="p1" d="M10 10 L20 20"/></svg>"#;
        let modified = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><path id="p1" d="M10 10 L20 20 L30 10 Z"/></svg>"#;
        let changes = driver.diff(Some(base), modified).unwrap();
        assert!(changes.contains(&SemanticChange::Modified {
            path: "/svg/path[@id='p1']@d".to_string(),
            old_value: "M10 10 L20 20".to_string(),
            new_value: "M10 10 L20 20 L30 10 Z".to_string(),
        }));
    }

    #[test]
    fn test_text_element_content_change() {
        let driver = SvgDriver::new();
        let base = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><text id="label1" x="10" y="20">Hello</text></svg>"#;
        let modified = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><text id="label1" x="10" y="20">World</text></svg>"#;
        let changes = driver.diff(Some(base), modified).unwrap();
        assert!(changes.contains(&SemanticChange::Modified {
            path: "/svg/text[@id='label1']#text".to_string(),
            old_value: "Hello".to_string(),
            new_value: "World".to_string(),
        }));
    }

    #[test]
    fn test_multiple_changes_in_one_diff() {
        let driver = SvgDriver::new();
        let base = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect id="box1" x="10" y="10" width="80" height="80" fill="red"/></svg>"#;
        let modified = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200"><rect id="box1" x="20" y="20" width="60" height="60" fill="blue"/><circle id="circ1" cx="50" cy="50" r="10" fill="green"/></svg>"#;
        let changes = driver.diff(Some(base), modified).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Modified { path, .. } if path == "/svg@width"
        )));
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Modified { path, .. } if path == "/svg@height"
        )));
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Modified { path, .. } if path == "/svg/rect[@id='box1']@fill"
        )));
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Added { path, .. } if path == "/svg/circle[@id='circ1']"
        )));
    }

    #[test]
    fn test_driver_name_and_extensions() {
        let driver = SvgDriver::new();
        assert_eq!(driver.name(), "SVG");
        assert_eq!(driver.supported_extensions(), &[".svg"]);
    }

    #[test]
    fn test_format_diff_output() {
        let driver = SvgDriver::new();
        let base = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect id="box1" fill="red"/></svg>"#;
        let modified = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect id="box1" fill="blue"/></svg>"#;
        let output = driver.format_diff(Some(base), modified).unwrap();
        assert!(output.contains("MODIFIED"));
        assert!(output.contains("/svg/rect[@id='box1']@fill"));
        assert!(output.contains("red"));
        assert!(output.contains("blue"));
    }

    #[test]
    fn test_merge_new_elements_both_sides() {
        let driver = SvgDriver::new();
        let base = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"></svg>"#;
        let ours = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect id="r1" fill="red"/></svg>"#;
        let theirs = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><circle id="c1" fill="blue"/></svg>"#;
        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("rect"));
        assert!(merged.contains("circle"));
    }

    #[test]
    fn test_merge_element_removed_by_one_modified_by_other_conflict() {
        let driver = SvgDriver::new();
        let base = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect id="box1" x="10" y="10" width="80" height="80" fill="red"/></svg>"#;
        let ours = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"></svg>"#;
        let theirs = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect id="box1" x="10" y="10" width="80" height="80" fill="blue"/></svg>"#;
        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn test_no_changes_diff() {
        let driver = SvgDriver::new();
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect id="box1" fill="red"/></svg>"#;
        let changes = driver.diff(Some(svg), svg).unwrap();
        assert!(changes.is_empty());
    }
}
