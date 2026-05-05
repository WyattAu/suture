// SPDX-License-Identifier: MIT OR Apache-2.0
use std::collections::{BTreeSet, HashMap};

use suture_driver::{DriverError, SemanticChange, SutureDriver};

use std::fmt::Write;
pub struct HtmlDriver;

impl HtmlDriver {
    #[must_use] 
    pub fn new() -> Self {
        Self
    }

    fn node_path(node: roxmltree::Node) -> String {
        let mut parts: Vec<String> = Vec::new();
        let mut current = node;

        while current.is_element() {
            let tag = current.tag_name().name();
            let mut part = tag.to_owned();

            if let Some(id) = current.attribute("id") {
                part.push('#');
                part.push_str(id);
            }

            if let Some(parent) = current.parent()
                && parent.is_element()
            {
                let mut idx = 0u32;
                let mut same_tag_count = 0u32;
                for child in parent.children() {
                    if child.is_element() && child.tag_name().name() == tag {
                        same_tag_count += 1;
                        if child == current {
                            idx = same_tag_count;
                            break;
                        }
                    }
                }
                if same_tag_count > 1 {
                    let _ = write!(part, "[{idx}]");
                }
            }

            parts.push(part);
            current = match current.parent() {
                Some(p) if p.is_element() => p,
                _ => break,
            };
        }

        parts.reverse();
        parts.join(" > ")
    }

    fn escape_xml(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
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
            node.children().filter(roxmltree::Node::is_element).collect();

        if element_children.is_empty() && text.is_empty() {
            format!("{pad}<{tag}{attr_str}/>")
        } else if element_children.is_empty() {
            format!("{pad}<{tag}{attr_str}>{}</{tag}>", Self::escape_xml(text))
        } else {
            let mut result = format!("{pad}<{tag}{attr_str}>\n");
            if !text.is_empty() {
                let _ = writeln!(result, 
                    "{}{}",
                    "  ".repeat(indent + 1),
                    Self::escape_xml(text)
                );
            }
            for child in &element_children {
                result.push_str(&Self::element_to_string(*child, indent + 1));
                result.push('\n');
            }
            let _ = write!(result, "{pad}</{tag}>");
            result
        }
    }

    fn diff_nodes(old: roxmltree::Node, new: roxmltree::Node) -> Vec<SemanticChange> {
        let mut changes = Vec::new();

        if old.tag_name().name() != new.tag_name().name() {
            changes.push(SemanticChange::Modified {
                path: Self::node_path(new),
                old_value: old.tag_name().name().to_owned(),
                new_value: new.tag_name().name().to_owned(),
            });
            return changes;
        }

        let path = Self::node_path(new);

        let old_attrs: HashMap<&str, &str> =
            old.attributes().map(|a| (a.name(), a.value())).collect();
        let new_attrs: HashMap<&str, &str> =
            new.attributes().map(|a| (a.name(), a.value())).collect();

        let old_keys: BTreeSet<&str> = old_attrs.keys().copied().collect();
        let new_keys: BTreeSet<&str> = new_attrs.keys().copied().collect();

        for key in &old_keys {
            if !new_keys.contains(key) {
                changes.push(SemanticChange::Removed {
                    path: format!("{path}@{key}"),
                    old_value: old_attrs[key].to_owned(),
                });
            }
        }

        for key in &new_keys {
            if !old_keys.contains(key) {
                changes.push(SemanticChange::Added {
                    path: format!("{path}@{key}"),
                    value: new_attrs[key].to_owned(),
                });
            }
        }

        for key in &old_keys {
            if let Some(&new_val) = new_attrs.get(key)
                && old_attrs[key] != new_val
            {
                changes.push(SemanticChange::Modified {
                    path: format!("{path}@{key}"),
                    old_value: old_attrs[key].to_owned(),
                    new_value: new_val.to_owned(),
                });
            }
        }

        let old_text = old.text().unwrap_or("").trim();
        let new_text = new.text().unwrap_or("").trim();
        if old_text != new_text {
            changes.push(SemanticChange::Modified {
                path: format!("{path}#text"),
                old_value: old_text.to_owned(),
                new_value: new_text.to_owned(),
            });
        }

        let old_children: Vec<roxmltree::Node> =
            old.children().filter(roxmltree::Node::is_element).collect();
        let new_children: Vec<roxmltree::Node> =
            new.children().filter(roxmltree::Node::is_element).collect();

        let max_len = old_children.len().max(new_children.len());
        for i in 0..max_len {
            match (old_children.get(i), new_children.get(i)) {
                (None, Some(new_c)) => {
                    changes.push(SemanticChange::Added {
                        path: Self::node_path(*new_c),
                        value: Self::element_to_string(*new_c, 0),
                    });
                }
                (Some(old_c), None) => {
                    changes.push(SemanticChange::Removed {
                        path: Self::node_path(*old_c),
                        old_value: Self::element_to_string(*old_c, 0),
                    });
                }
                (Some(old_c), Some(new_c)) => {
                    changes.extend(Self::diff_nodes(*old_c, *new_c));
                }
                (None, None) => {}
            }
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
            ours_text.to_owned()
        } else if ours_text == base_text {
            theirs_text.to_owned()
        } else if theirs_text == base_text {
            ours_text.to_owned()
        } else {
            return Ok(None);
        };

        let base_attrs: HashMap<&str, &str> =
            base.attributes().map(|a| (a.name(), a.value())).collect();
        let ours_attrs: HashMap<&str, &str> =
            ours.attributes().map(|a| (a.name(), a.value())).collect();
        let theirs_attrs: HashMap<&str, &str> =
            theirs.attributes().map(|a| (a.name(), a.value())).collect();

        let all_attr_keys: BTreeSet<&str> = base_attrs
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
                    merged_attrs.push((key.to_string(), o.to_owned()));
                }
                (Some(b), Some(o), Some(t)) if o == b => {
                    merged_attrs.push((key.to_string(), t.to_owned()));
                }
                (Some(b), Some(o), Some(t)) if t == b => {
                    merged_attrs.push((key.to_string(), o.to_owned()));
                }
                (Some(_) | None, Some(_), Some(_)) => return Ok(None),
                (Some(_) | None, None, Some(t)) => {
                    merged_attrs.push((key.to_string(), t.to_owned()));
                }
                (Some(_) | None, Some(o), None) => {
                    merged_attrs.push((key.to_string(), o.to_owned()));
                }
                (Some(_) | None, None, None) => {}
            }
        }

        let base_children: Vec<roxmltree::Node> =
            base.children().filter(roxmltree::Node::is_element).collect();
        let ours_children: Vec<roxmltree::Node> =
            ours.children().filter(roxmltree::Node::is_element).collect();
        let theirs_children: Vec<roxmltree::Node> =
            theirs.children().filter(roxmltree::Node::is_element).collect();

        let max_len = base_children
            .len()
            .max(ours_children.len())
            .max(theirs_children.len());
        let mut merged_children = Vec::new();

        for i in 0..max_len {
            let b = base_children.get(i).copied();
            let o = ours_children.get(i).copied();
            let t = theirs_children.get(i).copied();

            match (b, o, t) {
                (None | Some(_), Some(o), None) => {
                    merged_children.push(Self::element_to_string(o, indent + 1));
                }
                (None | Some(_), None, Some(t)) => {
                    merged_children.push(Self::element_to_string(t, indent + 1));
                }
                (None, Some(o), Some(t)) => {
                    merged_children.push(Self::element_to_string(o, indent + 1));
                    if o.tag_name().name() != t.tag_name().name()
                        || Self::element_to_string(o, 0) != Self::element_to_string(t, 0)
                    {
                        merged_children.push(Self::element_to_string(t, indent + 1));
                    }
                }
                (None, None, _) | (Some(_), None, None) => {}
                (Some(b), Some(o), Some(t)) => {
                    let bt = b.tag_name().name();
                    let ot = o.tag_name().name();
                    let tt = t.tag_name().name();

                    if ot == tt {
                        if ot == bt {
                            if let Some(merged) = Self::merge_elements(b, o, t, indent + 1)? {
                                merged_children.push(merged);
                            } else {
                                return Ok(None);
                            }
                        } else {
                            merged_children.push(Self::element_to_string(o, indent + 1));
                        }
                    } else if ot == bt {
                        merged_children.push(Self::element_to_string(t, indent + 1));
                    } else if tt == bt {
                        merged_children.push(Self::element_to_string(o, indent + 1));
                    } else {
                        return Ok(None);
                    }
                }
            }
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
                let _ = writeln!(result, 
                    "{}{}",
                    "  ".repeat(indent + 1),
                    Self::escape_xml(&merged_text)
                );
            }
            for child in &merged_children {
                result.push_str(child);
                result.push('\n');
            }
            let _ = write!(result, "{pad}</{tag}>");
            Ok(Some(result))
        }
    }

    fn format_change(change: &SemanticChange) -> String {
        match change {
            SemanticChange::Added { path, value } => {
                format!("  Added: {path}: {value}")
            }
            SemanticChange::Removed { path, old_value } => {
                format!("  Removed: {path}: {old_value}")
            }
            SemanticChange::Modified {
                path,
                old_value,
                new_value,
            } => {
                format!("  Modified: {path}: \"{old_value}\" -> \"{new_value}\"")
            }
            SemanticChange::Moved {
                old_path,
                new_path,
                value,
            } => {
                format!("  Moved: {old_path} -> {new_path}: {value}")
            }
        }
    }
}

impl Default for HtmlDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl SutureDriver for HtmlDriver {
    fn name(&self) -> &'static str {
        "HTML"
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".html", ".htm"]
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
                for child in new_doc.root().children() {
                    if child.is_element() {
                        collect_all_paths(child, &mut changes);
                    }
                }
                Ok(changes)
            }
            Some(base) => {
                let old_doc = roxmltree::Document::parse(base)
                    .map_err(|e| DriverError::ParseError(e.to_string()))?;
                Ok(Self::diff_nodes(old_doc.root(), new_doc.root()))
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
            return Ok("no changes".to_owned());
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
        result.push_str("<!DOCTYPE html>\n");
        Self::merge_elements(base_doc.root(), ours_doc.root(), theirs_doc.root(), 0)?
            .map_or_else(
                || Ok(None),
                |merged| {
                    result.push_str(&merged);
                    result.push('\n');
                    Ok(Some(result))
                },
            )
    }
}

fn collect_all_paths(node: roxmltree::Node, out: &mut Vec<SemanticChange>) {
    if !node.is_element() {
        return;
    }

    let path = HtmlDriver::node_path(node);

    for attr in node.attributes() {
        out.push(SemanticChange::Added {
            path: format!("{path}@{}", attr.name()),
            value: attr.value().to_owned(),
        });
    }

    let text = node.text().unwrap_or("").trim();
    if !text.is_empty() {
        out.push(SemanticChange::Added {
            path: format!("{path}#text"),
            value: text.to_owned(),
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
    fn test_new_html_file() {
        let driver = HtmlDriver::new();
        let content = r#"<?xml version="1.0"?><html><head><title>Hello</title></head><body><h1>World</h1></body></html>"#;

        let changes = driver.diff(None, content).unwrap();
        assert!(!changes.is_empty());
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Added { path, .. } if path.contains("title")
        )));
    }

    #[test]
    fn test_title_change() {
        let driver = HtmlDriver::new();
        let base = r#"<?xml version="1.0"?><html><head><title>Old Title</title></head></html>"#;
        let new = r#"<?xml version="1.0"?><html><head><title>New Title</title></head></html>"#;

        let changes = driver.diff(Some(base), new).unwrap();
        assert!(changes.contains(&SemanticChange::Modified {
            path: "html > head > title#text".to_string(),
            old_value: "Old Title".to_string(),
            new_value: "New Title".to_string(),
        }));
    }

    #[test]
    fn test_heading_text_change() {
        let driver = HtmlDriver::new();
        let base = r#"<?xml version="1.0"?><html><body><h1>Old Heading</h1></body></html>"#;
        let new = r#"<?xml version="1.0"?><html><body><h1>New Heading</h1></body></html>"#;

        let changes = driver.diff(Some(base), new).unwrap();
        assert!(changes.contains(&SemanticChange::Modified {
            path: "html > body > h1#text".to_string(),
            old_value: "Old Heading".to_string(),
            new_value: "New Heading".to_string(),
        }));
    }

    #[test]
    fn test_link_href_change() {
        let driver = HtmlDriver::new();
        let base =
            r#"<?xml version="1.0"?><html><body><a href="http://old.com">link</a></body></html>"#;
        let new =
            r#"<?xml version="1.0"?><html><body><a href="http://new.com">link</a></body></html>"#;

        let changes = driver.diff(Some(base), new).unwrap();
        assert!(changes.contains(&SemanticChange::Modified {
            path: "html > body > a@href".to_string(),
            old_value: "http://old.com".to_string(),
            new_value: "http://new.com".to_string(),
        }));
    }

    #[test]
    fn test_image_src_change() {
        let driver = HtmlDriver::new();
        let base =
            r#"<?xml version="1.0"?><html><body><img src="old.png" alt="pic"/></body></html>"#;
        let new =
            r#"<?xml version="1.0"?><html><body><img src="new.png" alt="pic"/></body></html>"#;

        let changes = driver.diff(Some(base), new).unwrap();
        assert!(changes.contains(&SemanticChange::Modified {
            path: "html > body > img@src".to_string(),
            old_value: "old.png".to_string(),
            new_value: "new.png".to_string(),
        }));
    }

    #[test]
    fn test_paragraph_addition() {
        let driver = HtmlDriver::new();
        let base = r#"<?xml version="1.0"?><html><body><h1>Title</h1></body></html>"#;
        let new =
            r#"<?xml version="1.0"?><html><body><h1>Title</h1><p>New paragraph</p></body></html>"#;

        let changes = driver.diff(Some(base), new).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Added { path, .. } if path.contains("p")
        )));
    }

    #[test]
    fn test_div_removal() {
        let driver = HtmlDriver::new();
        let base =
            r#"<?xml version="1.0"?><html><body><div id="content"><p>text</p></div></body></html>"#;
        let new = r#"<?xml version="1.0"?><html><body></body></html>"#;

        let changes = driver.diff(Some(base), new).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Removed { path, .. } if path.contains("div#content")
        )));
    }

    #[test]
    fn test_class_attribute_modification() {
        let driver = HtmlDriver::new();
        let base =
            r#"<?xml version="1.0"?><html><body><div class="old-class">text</div></body></html>"#;
        let new =
            r#"<?xml version="1.0"?><html><body><div class="new-class">text</div></body></html>"#;

        let changes = driver.diff(Some(base), new).unwrap();
        assert!(changes.contains(&SemanticChange::Modified {
            path: "html > body > div@class".to_string(),
            old_value: "old-class".to_string(),
            new_value: "new-class".to_string(),
        }));
    }

    #[test]
    fn test_clean_merge_different_sections() {
        let driver = HtmlDriver::new();
        let base = r#"<?xml version="1.0"?><html><body><h1>Title</h1><p>Body</p></body></html>"#;
        let ours =
            r#"<?xml version="1.0"?><html><body><h1>Our Title</h1><p>Body</p></body></html>"#;
        let theirs =
            r#"<?xml version="1.0"?><html><body><h1>Title</h1><p>Our Body</p></body></html>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("Our Title"));
        assert!(merged.contains("Our Body"));
    }

    #[test]
    fn test_conflict_merge_same_heading() {
        let driver = HtmlDriver::new();
        let base = r#"<?xml version="1.0"?><html><body><h1>Original</h1></body></html>"#;
        let ours = r#"<?xml version="1.0"?><html><body><h1>Ours</h1></body></html>"#;
        let theirs = r#"<?xml version="1.0"?><html><body><h1>Theirs</h1></body></html>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_table_cell_modification() {
        let driver = HtmlDriver::new();
        let base = r#"<?xml version="1.0"?><html><body><table><tr><td>old</td></tr></table></body></html>"#;
        let new = r#"<?xml version="1.0"?><html><body><table><tr><td>new</td></tr></table></body></html>"#;

        let changes = driver.diff(Some(base), new).unwrap();
        assert!(changes.contains(&SemanticChange::Modified {
            path: "html > body > table > tr > td#text".to_string(),
            old_value: "old".to_string(),
            new_value: "new".to_string(),
        }));
    }

    #[test]
    fn test_meta_tag_addition() {
        let driver = HtmlDriver::new();
        let base = r#"<?xml version="1.0"?><html><head><title>Test</title></head></html>"#;
        let new = r#"<?xml version="1.0"?><html><head><title>Test</title><meta name="description" content="desc"/></head></html>"#;

        let changes = driver.diff(Some(base), new).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Added { path, .. } if path.contains("meta")
        )));
    }
}
