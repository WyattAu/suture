use std::collections::{HashMap, HashSet};

use suture_driver::{DriverError, SemanticChange, SutureDriver};

pub struct XmlDriver;

impl XmlDriver {
    pub fn new() -> Self {
        Self
    }

    fn has_xml_declaration(content: &str) -> bool {
        content.trim_start().starts_with("<?xml")
    }

    fn escape_xml(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }

    fn node_path(node: roxmltree::Node) -> String {
        let mut parts: Vec<String> = Vec::new();
        let mut current = node;

        while current.is_element() {
            match current.parent() {
                Some(p) if p.is_element() => {
                    let tag = current.tag_name().name();
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
                    current = p;
                }
                _ => {
                    parts.push(current.tag_name().name().to_string());
                    break;
                }
            }
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
                (None, Some(o), None) => {
                    merged_children.push(Self::element_to_string(o, indent + 1));
                }
                (None, None, Some(t)) => {
                    merged_children.push(Self::element_to_string(t, indent + 1));
                }
                (None, Some(o), Some(t)) => {
                    if o.tag_name().name() == t.tag_name().name()
                        && Self::element_to_string(o, 0) == Self::element_to_string(t, 0)
                    {
                        merged_children.push(Self::element_to_string(o, indent + 1));
                    } else {
                        // Both sides added different children at the same position.
                        // Include both — additions from both sides should be preserved.
                        merged_children.push(Self::element_to_string(o, indent + 1));
                        merged_children.push(Self::element_to_string(t, indent + 1));
                    }
                }
                (None, None, _) => {}
                (Some(_), Some(o), None) => {
                    merged_children.push(Self::element_to_string(o, indent + 1));
                }
                (Some(_), None, Some(t)) => {
                    merged_children.push(Self::element_to_string(t, indent + 1));
                }
                (Some(_), None, None) => {}
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

impl Default for XmlDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl SutureDriver for XmlDriver {
    fn name(&self) -> &str {
        "XML"
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".xml"]
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
                Ok(Self::diff_nodes(
                    old_doc.root_element(),
                    new_doc.root_element(),
                ))
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

        let has_declaration = Self::has_xml_declaration(base)
            || Self::has_xml_declaration(ours)
            || Self::has_xml_declaration(theirs);

        let mut result = String::new();
        if has_declaration {
            result.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        }
        if let Some(merged) = Self::merge_elements(
            base_doc.root_element(),
            ours_doc.root_element(),
            theirs_doc.root_element(),
            0,
        )? {
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

    let path = XmlDriver::node_path(node);

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
    fn test_xml_driver_name() {
        let driver = XmlDriver::new();
        assert_eq!(driver.name(), "XML");
    }

    #[test]
    fn test_xml_driver_extensions() {
        let driver = XmlDriver::new();
        assert_eq!(driver.supported_extensions(), &[".xml"]);
    }

    #[test]
    fn test_xml_diff_modified_text() {
        let driver = XmlDriver::new();
        let old = r#"<root><name>Alice</name></root>"#;
        let new = r#"<root><name>Bob</name></root>"#;

        let changes = driver.diff(Some(old), new).unwrap();
        assert!(changes.contains(&SemanticChange::Modified {
            path: "/root/name[1]#text".to_string(),
            old_value: "Alice".to_string(),
            new_value: "Bob".to_string(),
        }));
    }

    #[test]
    fn test_xml_diff_added_element() {
        let driver = XmlDriver::new();
        let old = r#"<root><name>Alice</name></root>"#;
        let new = r#"<root><name>Alice</name><email>alice@example.com</email></root>"#;

        let changes = driver.diff(Some(old), new).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Added { path, .. } if path == "/root/email[1]"
        )));
    }

    #[test]
    fn test_xml_diff_removed_element() {
        let driver = XmlDriver::new();
        let old = r#"<root><name>Alice</name><email>alice@example.com</email></root>"#;
        let new = r#"<root><name>Alice</name></root>"#;

        let changes = driver.diff(Some(old), new).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Removed { path, .. } if path == "/root/email[1]"
        )));
    }

    #[test]
    fn test_xml_diff_attribute_change() {
        let driver = XmlDriver::new();
        let old = r#"<root><item id="1">foo</item></root>"#;
        let new = r#"<root><item id="2">foo</item></root>"#;

        let changes = driver.diff(Some(old), new).unwrap();
        assert!(changes.contains(&SemanticChange::Modified {
            path: "/root/item[1]@id".to_string(),
            old_value: "1".to_string(),
            new_value: "2".to_string(),
        }));
    }

    #[test]
    fn test_xml_format_diff() {
        let driver = XmlDriver::new();
        let old = r#"<root><name>Alice</name></root>"#;
        let new = r#"<root><name>Bob</name><email>bob@example.com</email></root>"#;

        let output = driver.format_diff(Some(old), new).unwrap();
        assert!(output.contains("MODIFIED"));
        assert!(output.contains("ADDED"));
    }

    #[test]
    fn test_xml_merge_no_conflict() {
        let driver = XmlDriver::new();
        let base = r#"<root><a>1</a><b>2</b><c>3</c></root>"#;
        let ours = r#"<root><a>10</a><b>2</b><c>3</c></root>"#;
        let theirs = r#"<root><a>1</a><b>2</b><c>30</c></root>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains(">10<"));
        assert!(merged.contains(">30<"));
    }

    #[test]
    fn test_xml_merge_conflict() {
        let driver = XmlDriver::new();
        let base = r#"<root><key>original</key></root>"#;
        let ours = r#"<root><key>ours</key></root>"#;
        let theirs = r#"<root><key>theirs</key></root>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_correctness_merge_determinism() {
        let driver = XmlDriver::new();
        let base = r#"<root><a>1</a><b>2</b><c>3</c></root>"#;
        let ours = r#"<root><a>10</a><b>2</b><d>4</d></root>"#;
        let theirs = r#"<root><a>1</a><b>20</b><e>5</e></root>"#;

        let r1 = driver.merge(base, ours, theirs).unwrap();
        let r2 = driver.merge(base, theirs, ours).unwrap();
        assert_eq!(r1.is_some(), r2.is_some());
        if let (Some(m1), Some(m2)) = (r1, r2) {
            let d1 = roxmltree::Document::parse(&m1).unwrap();
            let d2 = roxmltree::Document::parse(&m2).unwrap();
            let s1 = XmlDriver::element_to_string(d1.root(), 0);
            let s2 = XmlDriver::element_to_string(d2.root(), 0);
            assert_eq!(s1, s2, "merge must be commutative");
        }
    }

    #[test]
    fn test_correctness_merge_idempotency() {
        let driver = XmlDriver::new();
        let base = r#"<root><a>1</a><b>2</b></root>"#;
        let ours = r#"<root><a>10</a><b>2</b><c>3</c></root>"#;

        let result = driver.merge(base, ours, ours).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains(">10<"));
        assert!(merged.contains(">3<"));
    }

    #[test]
    fn test_correctness_base_equals_ours() {
        let driver = XmlDriver::new();
        let base = r#"<root><a>1</a><b>2</b></root>"#;
        let theirs = r#"<root><a>10</a><b>2</b><c>3</c></root>"#;

        let result = driver.merge(base, base, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains(">10<"));
        assert!(merged.contains(">3<"));
    }

    #[test]
    fn test_correctness_base_equals_theirs() {
        let driver = XmlDriver::new();
        let base = r#"<root><a>1</a><b>2</b></root>"#;
        let ours = r#"<root><a>10</a><b>2</b><c>3</c></root>"#;

        let result = driver.merge(base, ours, base).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains(">10<"));
        assert!(merged.contains(">3<"));
    }

    #[test]
    fn test_correctness_all_equal() {
        let driver = XmlDriver::new();
        let content = r#"<root><x>42</x><y>hello</y></root>"#;

        let result = driver.merge(content, content, content).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains(">42<"));
        assert!(merged.contains(">hello<"));
    }

    #[test]
    fn test_correctness_both_add_different_elements() {
        let driver = XmlDriver::new();
        let base = r#"<root><shared>true</shared></root>"#;
        let ours = r#"<root><shared>true</shared><from_ours>100</from_ours></root>"#;
        let theirs = r#"<root><shared>true</shared><from_theirs>200</from_theirs></root>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains(">100<"), "ours element should be present");
        assert!(merged.contains(">200<"), "theirs element should be present");
        assert!(merged.contains(">true<"));
    }

    #[test]
    fn test_correctness_both_modify_different_elements() {
        let driver = XmlDriver::new();
        let base = r#"<root><a>1</a><b>2</b><c>3</c></root>"#;
        let ours = r#"<root><a>10</a><b>2</b><c>3</c></root>"#;
        let theirs = r#"<root><a>1</a><b>2</b><c>30</c></root>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains(">10<"), "ours change to a");
        assert!(merged.contains(">30<"), "theirs change to c");
        assert!(merged.contains(">2<"), "unchanged b");
    }

    #[test]
    fn test_correctness_both_modify_same_element_same_value() {
        let driver = XmlDriver::new();
        let base = r#"<root><key>original</key></root>"#;
        let ours = r#"<root><key>changed</key></root>"#;
        let theirs = r#"<root><key>changed</key></root>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some(), "identical changes should not conflict");
        let merged = result.unwrap();
        assert!(merged.contains(">changed<"));
    }

    #[test]
    fn test_correctness_both_modify_same_element_different_value() {
        let driver = XmlDriver::new();
        let base = r#"<root><key>original</key></root>"#;
        let ours = r#"<root><key>ours</key></root>"#;
        let theirs = r#"<root><key>theirs</key></root>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_correctness_deeply_nested_merge() {
        let driver = XmlDriver::new();
        let base = r#"<root><l1><l2><l3><a>1</a><b>2</b><c>3</c></l3></l2></l1></root>"#;
        let ours = r#"<root><l1><l2><l3><a>10</a><b>2</b><c>3</c></l3></l2></l1></root>"#;
        let theirs = r#"<root><l1><l2><l3><a>1</a><b>2</b><c>30</c></l3></l2></l1></root>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains(">10<"));
        assert!(merged.contains(">30<"));
        assert!(merged.contains(">2<"));
    }

    #[test]
    fn test_correctness_unicode_text() {
        let driver = XmlDriver::new();
        let base = r#"<root><名前>太郎</名前><age>30</age></root>"#;
        let ours = r#"<root><名前>太郎</名前><age>31</age></root>"#;
        let theirs = r#"<root><名前>次郎</名前><age>30</age></root>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("次郎"));
        assert!(merged.contains(">31<"));
    }

    #[test]
    fn test_correctness_large_file() {
        let driver = XmlDriver::new();
        let mut base_children = String::new();
        let mut ours_children = String::new();
        let mut theirs_children = String::new();

        for i in 0..100 {
            let tag = format!("item{i}");
            let val = format!("value_{i}");
            let ours_val = if i == 50 {
                "modified_by_ours".to_string()
            } else {
                val.clone()
            };
            let theirs_val = if i == 80 {
                "modified_by_theirs".to_string()
            } else {
                val.clone()
            };

            base_children.push_str(&format!("<{tag}>{val}</{tag}>"));
            ours_children.push_str(&format!("<{tag}>{ours_val}</{tag}>"));
            theirs_children.push_str(&format!("<{tag}>{theirs_val}</{tag}>"));
        }

        let base = format!("<root>{base_children}</root>");
        let ours = format!("<root>{ours_children}</root>");
        let theirs = format!("<root>{theirs_children}</root>");

        let result = driver.merge(&base, &ours, &theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("modified_by_ours"));
        assert!(merged.contains("modified_by_theirs"));
        assert!(merged.contains("value_0"));
        assert!(merged.contains("value_99"));
    }

    #[test]
    fn test_correctness_output_validity() {
        let driver = XmlDriver::new();
        let base = r#"<?xml version="1.0"?><root><a>1</a><b>2</b></root>"#;
        let ours = r#"<?xml version="1.0"?><root><a>10</a><b>2</b></root>"#;
        let theirs = r#"<?xml version="1.0"?><root><a>1</a><b>20</b></root>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged_str = result.unwrap();
        assert!(merged_str.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(merged_str.contains(">10<"));
        assert!(merged_str.contains(">20<"));
        assert!(merged_str.contains("<root"));
        assert!(merged_str.contains("</root>"));
    }

    #[test]
    fn test_correctness_attribute_merge_same_element() {
        let driver = XmlDriver::new();
        let base = r#"<root><item id="1" name="a">text</item></root>"#;
        let ours = r#"<root><item id="2" name="a">text</item></root>"#;
        let theirs = r#"<root><item id="1" name="b">text</item></root>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("id=\"2\""), "ours attr change");
        assert!(merged.contains("name=\"b\""), "theirs attr change");
    }

    #[test]
    fn test_correctness_attribute_conflict() {
        let driver = XmlDriver::new();
        let base = r#"<root><item id="1">text</item></root>"#;
        let ours = r#"<root><item id="2">text</item></root>"#;
        let theirs = r#"<root><item id="3">text</item></root>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(
            result.is_none(),
            "conflicting attribute changes should conflict"
        );
    }

    #[test]
    fn test_correctness_attribute_added_by_one_side() {
        let driver = XmlDriver::new();
        let base = r#"<root><item id="1">text</item></root>"#;
        let ours = r#"<root><item id="1" color="red">text</item></root>"#;
        let theirs = r#"<root><item id="1">text</item></root>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("color=\"red\""));
    }

    #[test]
    fn test_correctness_attribute_removed_by_one_side() {
        let driver = XmlDriver::new();
        let base = r#"<root><item id="1" color="red">text</item></root>"#;
        let ours = r#"<root><item id="1">text</item></root>"#;
        let theirs = r#"<root><item id="1" color="red">text</item></root>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(
            merged.contains("color=\"red\""),
            "theirs kept the attribute since ours removed it but theirs didn't"
        );
    }

    #[test]
    fn test_correctness_namespace_handling() {
        let driver = XmlDriver::new();
        let base = r#"<root xmlns:ns="http://example.com"><ns:item>text</ns:item></root>"#;
        let ours = r#"<root xmlns:ns="http://example.com"><ns:item>modified</ns:item></root>"#;
        let theirs = r#"<root xmlns:ns="http://example.com"><ns:item>text</ns:item></root>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("modified"));
    }

    #[test]
    fn test_correctness_cdata_section() {
        let driver = XmlDriver::new();
        let base = r#"<root><data><![CDATA[original]]></data></root>"#;
        let ours = r#"<root><data><![CDATA[ours]]></data></root>"#;
        let theirs = r#"<root><data><![CDATA[original]]></data></root>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("ours"));
    }

    #[test]
    fn test_correctness_empty_elements() {
        let driver = XmlDriver::new();
        let base = r#"<root><a/><b/></root>"#;
        let ours = r#"<root><a>filled</a><b/></root>"#;
        let theirs = r#"<root><a/><b>filled</b></root>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains(">filled<"));
    }

    #[test]
    fn test_correctness_both_add_same_element_different_content() {
        let driver = XmlDriver::new();
        let base = r#"<root><existing>keep</existing></root>"#;
        let ours = r#"<root><existing>keep</existing><new>ours</new></root>"#;
        let theirs = r#"<root><existing>keep</existing><new>theirs</new></root>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(
            result.is_some(),
            "both adding same element at same position with different content should include both"
        );
        let merged = result.unwrap();
        assert!(merged.contains(">ours<"));
        assert!(merged.contains(">theirs<"));
    }

    #[test]
    fn test_correctness_mixed_text_and_child_modifications() {
        let driver = XmlDriver::new();
        let base = r#"<root><parent>base_text<child>1</child></parent></root>"#;
        let ours = r#"<root><parent>ours_text<child>1</child></parent></root>"#;
        let theirs = r#"<root><parent>base_text<child>10</child></parent></root>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("ours_text"), "ours text change");
        assert!(merged.contains(">10<"), "theirs child change");
    }
}
