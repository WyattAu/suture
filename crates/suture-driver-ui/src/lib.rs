// SPDX-License-Identifier: MIT OR Apache-2.0
use std::collections::{HashMap, HashSet};

use suture_driver::{DriverError, SemanticChange, SutureDriver};

use std::fmt::Write;
pub struct UiDriver;

impl UiDriver {
    #[must_use]
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
                    parts.push(current.tag_name().name().to_owned());
                    break;
                }
            }
        }

        parts.reverse();
        format!("/{}", parts.join("/"))
    }

    /// 风格化序列化：匹配 Actions IDE 文件的固定格式——
    /// 4 空格缩进、属性保持文档顺序（`name=` 在前）、自闭合带空格 ` />`。
    /// 由于 .ui 由软件生成、格式固定，未修改元素经此序列化后与原文逐字节一致，
    /// 语义合并的文本 diff 只包含真正变化的部分。
    fn element_to_string(node: roxmltree::Node, indent: usize) -> String {
        let pad = "    ".repeat(indent);
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
        let element_children: Vec<roxmltree::Node> = node
            .children()
            .filter(roxmltree::Node::is_element)
            .collect();

        if element_children.is_empty() && text.is_empty() {
            format!("{pad}<{tag}{attr_str} />")
        } else if element_children.is_empty() {
            format!("{pad}<{tag}{attr_str}>{}</{tag}>", Self::escape_xml(text))
        } else {
            let mut result = format!("{pad}<{tag}{attr_str}>\n");
            if !text.is_empty() {
                let _ = writeln!(
                    result,
                    "{}{}",
                    "    ".repeat(indent + 1),
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

    /// 局部降级合并：当某子树无法语义合并（属性冲突、元素 tag 三方不同、
    /// 文本三方不同等）时，对该子树做行级三方合并，输出冲突标记。
    ///
    /// 关键设计：**不再让整个文件 decline 到行级合并**——否则两个分支在
    /// 同一位置新增的不同元素（如新场景）会被行级合并误判为冲突。只把
    /// 冲突限制在最小子树内，其余部分继续走语义合并。
    ///
    /// 输出规则：内容行按 `indent` 缩进（与兄弟元素对齐）；冲突标记行
    /// （`<<<<<<<`/`=======`/`>>>>>>>`）**顶格输出**——VS Code/git 识别
    /// 冲突标记要求标记位于行首。用户解决冲突删除标记后文件恢复合法。
    fn merge_subtree_lines(
        base: roxmltree::Node,
        ours: roxmltree::Node,
        theirs: roxmltree::Node,
        indent: usize,
    ) -> String {
        let b = Self::element_to_string(base, 0);
        let o = Self::element_to_string(ours, 0);
        let t = Self::element_to_string(theirs, 0);
        let b_lines: Vec<&str> = b.lines().collect();
        let o_lines: Vec<&str> = o.lines().collect();
        let t_lines: Vec<&str> = t.lines().collect();
        let result = suture_core::engine::merge::three_way_merge_lines(
            &b_lines,
            &o_lines,
            &t_lines,
            "ours",
            "theirs",
        );
        let pad = "    ".repeat(indent);
        result
            .lines
            .iter()
            .map(|l| {
                if l.starts_with("<<<<<<<")
                    || l == "======="
                    || l.starts_with(">>>>>>>")
                {
                    l.clone()
                } else {
                    format!("{pad}{l}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
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

        let old_keys: HashSet<&str> = old_attrs.keys().copied().collect();
        let new_keys: HashSet<&str> = new_attrs.keys().copied().collect();

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
            // 两分支把同一元素改成不同 tag：无法语义合并 → 局部行级合并
            return Ok(Some(Self::merge_subtree_lines(base, ours, theirs, indent)));
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
            // 文本三方各不同：无法语义合并 → 局部行级合并
            return Ok(Some(Self::merge_subtree_lines(base, ours, theirs, indent)));
        };

        let base_attrs: HashMap<&str, &str> =
            base.attributes().map(|a| (a.name(), a.value())).collect();
        let ours_attrs: HashMap<&str, &str> =
            ours.attributes().map(|a| (a.name(), a.value())).collect();
        let theirs_attrs: HashMap<&str, &str> =
            theirs.attributes().map(|a| (a.name(), a.value())).collect();

        // 属性顺序：base 原始顺序优先，新增属性按 ours 后 theirs 追加
        // （保证未修改元素序列化后与原文逐字节一致）
        let mut attr_order: Vec<&str> = base.attributes().map(|a| a.name()).collect();
        for a in ours.attributes() {
            if !attr_order.contains(&a.name()) {
                attr_order.push(a.name());
            }
        }
        for a in theirs.attributes() {
            if !attr_order.contains(&a.name()) {
                attr_order.push(a.name());
            }
        }

        let mut merged_attrs: Vec<(String, String)> = Vec::new();

        for key in attr_order {
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
                (Some(_) | None, Some(_), Some(_)) => {
                    // 同一属性两边改成不同值（如场景的 x 坐标冲突）：
                    // 局部降级为该元素子树的行级合并，只对冲突行输出标记。
                    // 修复前此处 return Ok(None) 会让整个文件 decline 到行级
                    // 合并，导致同位置新增的不同场景被误判为冲突。
                    return Ok(Some(Self::merge_subtree_lines(
                        base, ours, theirs, indent,
                    )));
                }
                (Some(_) | None, None, Some(t)) => {
                    merged_attrs.push((key.to_string(), t.to_owned()));
                }
                (Some(_) | None, Some(o), None) => {
                    merged_attrs.push((key.to_string(), o.to_owned()));
                }
                (Some(_) | None, None, None) => {}
            }
        }

        let base_children: Vec<roxmltree::Node> = base
            .children()
            .filter(roxmltree::Node::is_element)
            .collect();
        let ours_children: Vec<roxmltree::Node> = ours
            .children()
            .filter(roxmltree::Node::is_element)
            .collect();
        let theirs_children: Vec<roxmltree::Node> = theirs
            .children()
            .filter(roxmltree::Node::is_element)
            .collect();

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
                    // 两个分支在 base 末尾之外（同位置）各自新增了元素：
                    // o == t → 同一新增（去重）；o != t → 两个独立新增，都保留。
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
                                // 子元素内部无法语义合并（属性/文本冲突）→
                                // 对该子元素子树做局部行级合并，不向上传播
                                merged_children.push(Self::merge_subtree_lines(
                                    b, o, t, indent + 1,
                                ));
                            }
                        } else {
                            merged_children.push(Self::element_to_string(o, indent + 1));
                            // 如果两个第一个元素的内容不同，则都保留
                            if Self::element_to_string(o, 0)
                                != Self::element_to_string(t, 0)
                            {
                                merged_children.push(Self::element_to_string(t, indent + 1));
                            }
                        }
                    } else if ot == bt {
                        merged_children.push(Self::element_to_string(t, indent + 1));
                    } else if tt == bt {
                        merged_children.push(Self::element_to_string(o, indent + 1));
                    } else {
                        // 三方 tag 各不相同：无法语义合并 → 局部行级合并
                        merged_children.push(Self::merge_subtree_lines(b, o, t, indent + 1));
                    }
                }
            }
        }

        let pad = "    ".repeat(indent);
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
            Ok(Some(format!("{pad}<{tag}{attr_str} />")))
        } else if merged_children.is_empty() {
            Ok(Some(format!(
                "{pad}<{tag}{attr_str}>{}</{tag}>",
                Self::escape_xml(&merged_text)
            )))
        } else {
            let mut result = format!("{pad}<{tag}{attr_str}>\n");
            if !merged_text.is_empty() {
                let _ = writeln!(
                    result,
                    "{}{}",
                    "    ".repeat(indent + 1),
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

impl Default for UiDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl SutureDriver for UiDriver {
    fn name(&self) -> &'static str {
        "UI"
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".ui"]
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
            return Ok("no changes".to_owned());
        }

        let lines: Vec<String> = changes.iter().map(Self::format_change).collect();
        Ok(lines.join("\n"))
    }

    /// 语义三方合并。与 trait 默认契约（`None` = 冲突）不同：本 driver 采用
    /// **局部降级**策略——无法语义合并的子树降级为行级合并（输出冲突标记），
    /// 其余部分继续语义合并，因此只要三方可解析就返回 `Some`。
    ///
    /// 输出可能含 `<<<<<<<`/`=======`/`>>>>>>>` 冲突标记（标记行顶格）。
    /// 调用方（`suture merge-file`）必须检测标记：含标记视为冲突，
    /// 写入输出文件后以非 0 退出码结束，git 会保留该文件为冲突状态。
    fn merge(&self, base: &str, ours: &str, theirs: &str) -> Result<Option<String>, DriverError> {
        let base_doc =
            roxmltree::Document::parse(base).map_err(|e| DriverError::ParseError(e.to_string()))?;
        let ours_doc =
            roxmltree::Document::parse(ours).map_err(|e| DriverError::ParseError(e.to_string()))?;
        let theirs_doc = roxmltree::Document::parse(theirs)
            .map_err(|e| DriverError::ParseError(e.to_string()))?;

        // 文档级前导内容（XML 声明、注释、空白）：保留原文（优先 base）。
        // root 元素之前的文本不属于合并语义范围，原样保留即可。
        let base_leading = &base[..base_doc.root_element().range().start];
        let ours_leading = &ours[..ours_doc.root_element().range().start];
        let theirs_leading = &theirs[..theirs_doc.root_element().range().start];
        let leading = if !base_leading.trim().is_empty() {
            base_leading
        } else if !ours_leading.trim().is_empty() {
            ours_leading
        } else {
            theirs_leading
        };

        // 尾随换行：仅当某一输入文件以换行结尾时保留（风格保持）
        let trailing_newline = base.ends_with('\n')
            || ours.ends_with('\n')
            || theirs.ends_with('\n');

        let mut result = String::new();
        result.push_str(leading);
        Self::merge_elements(
            base_doc.root_element(),
            ours_doc.root_element(),
            theirs_doc.root_element(),
            0,
        )?
        .map_or_else(
            || Ok(None),
            |merged| {
                // .ui 固定 CRLF：String::replace 返回新 String（不改原值），
                // 直接把转换结果推入输出。
                result.push_str(&merged.replace('\n', "\r\n"));
                if trailing_newline {
                    // 尾随换行也需与输出行尾一致（.ui 固定 CRLF，不能 push 裸 \n）
                    result.push_str("\r\n");
                }
                Ok(Some(result))
            },
        )
    }
}

fn collect_all_paths(node: roxmltree::Node, out: &mut Vec<SemanticChange>) {
    if !node.is_element() {
        return;
    }

    let path = UiDriver::node_path(node);

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
    use proptest::prop_assert;
    use proptest::proptest;

    #[test]
    fn test_ui_driver_name() {
        let driver = UiDriver::new();
        assert_eq!(driver.name(), "UI");
    }

    #[test]
    fn test_ui_driver_extensions() {
        let driver = UiDriver::new();
        assert_eq!(driver.supported_extensions(), &[".ui"]);
    }

    #[test]
    fn test_xml_diff_modified_text() {
        let driver = UiDriver::new();
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
        let driver = UiDriver::new();
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
        let driver = UiDriver::new();
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
        let driver = UiDriver::new();
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
        let driver = UiDriver::new();
        let old = r#"<root><name>Alice</name></root>"#;
        let new = r#"<root><name>Bob</name><email>bob@example.com</email></root>"#;

        let output = driver.format_diff(Some(old), new).unwrap();
        assert!(output.contains("MODIFIED"));
        assert!(output.contains("ADDED"));
    }

    #[test]
    fn test_xml_merge_no_conflict() {
        let driver = UiDriver::new();
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
        let driver = UiDriver::new();
        let base = r#"<root><key>original</key></root>"#;
        let ours = r#"<root><key>ours</key></root>"#;
        let theirs = r#"<root><key>theirs</key></root>"#;

        // 局部降级：返回带冲突标记的部分合并结果，而非整体 decline
        let result = driver.merge(base, ours, theirs).unwrap();
        let merged = result.expect("partial merge produces conflict-marked output");
        assert!(merged.contains("<<<<<<< ours"));
        assert!(merged.contains(">>>>>>> theirs"));
        assert!(merged.contains("ours"));
        assert!(merged.contains("theirs"));
    }

    #[test]
    fn test_correctness_merge_determinism() {
        let driver = UiDriver::new();
        // 无冲突场景：ours 改 <a>，theirs 改 <b> → 合并可交换
        let base = r#"<root><a>1</a><b>2</b><c>3</c></root>"#;
        let ours = r#"<root><a>10</a><b>2</b><c>3</c></root>"#;
        let theirs = r#"<root><a>1</a><b>20</b><c>3</c></root>"#;

        let r1 = driver.merge(base, ours, theirs).unwrap();
        let r2 = driver.merge(base, theirs, ours).unwrap();
        assert_eq!(r1.is_some(), r2.is_some());
        if let (Some(m1), Some(m2)) = (r1, r2) {
            let d1 = roxmltree::Document::parse(&m1).unwrap();
            let d2 = roxmltree::Document::parse(&m2).unwrap();
            let s1 = UiDriver::element_to_string(d1.root(), 0);
            let s2 = UiDriver::element_to_string(d2.root(), 0);
            assert_eq!(s1, s2, "clean merge must be commutative");
        }
    }

    #[test]
    fn test_correctness_merge_idempotency() {
        let driver = UiDriver::new();
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
        let driver = UiDriver::new();
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
        let driver = UiDriver::new();
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
        let driver = UiDriver::new();
        let content = r#"<root><x>42</x><y>hello</y></root>"#;

        let result = driver.merge(content, content, content).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains(">42<"));
        assert!(merged.contains(">hello<"));
    }

    #[test]
    fn test_correctness_both_add_different_elements() {
        let driver = UiDriver::new();
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
        let driver = UiDriver::new();
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
        let driver = UiDriver::new();
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
        let driver = UiDriver::new();
        let base = r#"<root><key>original</key></root>"#;
        let ours = r#"<root><key>ours</key></root>"#;
        let theirs = r#"<root><key>theirs</key></root>"#;

        // 局部降级：文本三方各不同 → 带冲突标记的部分合并结果
        let result = driver.merge(base, ours, theirs).unwrap();
        let merged = result.expect("conflicting text changes produce marked output");
        assert!(merged.contains("ours"));
        assert!(merged.contains("theirs"));
        assert!(merged.contains("<<<<<<< ours"));
    }

    #[test]
    fn test_correctness_deeply_nested_merge() {
        let driver = UiDriver::new();
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
        let driver = UiDriver::new();
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
        let driver = UiDriver::new();
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

            let _ = write!(base_children, "<{tag}>{val}</{tag}>");
            let _ = write!(ours_children, "<{tag}>{ours_val}</{tag}>");
            let _ = write!(theirs_children, "<{tag}>{theirs_val}</{tag}>");
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
        let driver = UiDriver::new();
        let base = r#"<?xml version="1.0"?><root><a>1</a><b>2</b></root>"#;
        let ours = r#"<?xml version="1.0"?><root><a>10</a><b>2</b></root>"#;
        let theirs = r#"<?xml version="1.0"?><root><a>1</a><b>20</b></root>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged_str = result.unwrap();
        assert!(
            merged_str.contains("<?xml version=\"1.0\"?>"),
            "original XML declaration must be preserved"
        );
        assert!(merged_str.contains(">10<"));
        assert!(merged_str.contains(">20<"));
        assert!(merged_str.contains("<root"));
        assert!(merged_str.contains("</root>"));
    }

    #[test]
    fn test_correctness_attribute_merge_same_element() {
        let driver = UiDriver::new();
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
        let driver = UiDriver::new();
        let base = r#"<root><item id="1">text</item></root>"#;
        let ours = r#"<root><item id="2">text</item></root>"#;
        let theirs = r#"<root><item id="3">text</item></root>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        // 局部降级：同一属性两边改不同值 → 带冲突标记的部分合并结果
        let merged = result.expect("conflicting attrs produce marked output");
        assert!(merged.contains("id=\"2\""));
        assert!(merged.contains("id=\"3\""));
        assert!(merged.contains("<<<<<<< ours"));
    }

    #[test]
    fn test_correctness_attribute_added_by_one_side() {
        let driver = UiDriver::new();
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
        let driver = UiDriver::new();
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
        let driver = UiDriver::new();
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
        let driver = UiDriver::new();
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
        let driver = UiDriver::new();
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
        let driver = UiDriver::new();
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
    fn test_correctness_both_insert_different_elements_at_middle() {
        // 两个分支在 base 中间位置插入同 tag 的不同元素（真实丢失场景的最小复现）：
        // ours/theirs 都在 <b> 之后插入 <new>，base 的 <c> 被挤到后续索引。
        let driver = UiDriver::new();
        let base = r#"<root><a>1</a><b>2</b><c>3</c></root>"#;
        let ours = r#"<root><a>1</a><b>2</b><new name="ours">x</new><c>3</c></root>"#;
        let theirs = r#"<root><a>1</a><b>2</b><new name="theirs">y</new><c>3</c></root>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(
            merged.contains(r#"name="ours""#),
            "ours inserted element must be present"
        );
        assert!(
            merged.contains(r#"name="theirs""#),
            "theirs inserted element must not be lost"
        );
        assert!(merged.contains(">3<"), "base element <c> must be preserved");
        assert!(merged.contains(">2<"), "base element <b> must be preserved");
    }

    #[test]
    fn test_correctness_both_insert_same_element_at_middle() {
        // 两个分支在相同位置插入完全相同的内容 → 去重，只保留一个。
        let driver = UiDriver::new();
        let base = r#"<root><a>1</a><b>2</b><c>3</c></root>"#;
        let ours = r#"<root><a>1</a><b>2</b><new>dup</new><c>3</c></root>"#;
        let theirs = r#"<root><a>1</a><b>2</b><new>dup</new><c>3</c></root>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert_eq!(merged.matches("<new>").count(), 1, "identical insert dedup");
        assert!(merged.contains(">dup<"));
        assert!(merged.contains(">3<"));
    }

    #[test]
    fn test_ui_style_output() {
        // 风格化输出：4 空格缩进、自闭合带空格 ` />`、属性保序（name 在前）
        let driver = UiDriver::new();
        let base = r#"<ui-rad><property name="w" value="0x1" /><scene><property name="name" value="S1" /></scene></ui-rad>"#;
        let ours = r#"<ui-rad><property name="w" value="0x1" /><scene><property name="name" value="S1" /></scene><scene><property name="name" value="SCENE_A" /></scene></ui-rad>"#;
        let theirs = r#"<ui-rad><property name="w" value="0x1" /><scene><property name="name" value="S1" /></scene><scene><property name="name" value="SCENE_B" /></scene></ui-rad>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        // 4 空格缩进 + 属性顺序（name 在前 value 在后）+ 自闭合 ` />`
        assert!(merged.contains("    <property name=\"w\" value=\"0x1\" />"));
        assert!(merged.contains("    <scene>"));
        assert!(merged.contains("        <property name=\"name\" value=\"S1\" />"));
        assert!(merged.contains("        <property name=\"name\" value=\"SCENE_A\" />"));
        assert!(merged.contains("        <property name=\"name\" value=\"SCENE_B\" />"));
        assert!(merged.contains("    </scene>"));
    }

    #[test]
    fn test_ui_attr_order_preserved() {
        // 未修改元素的属性顺序应保持 base 顺序（序列化后与原文一致）
        let driver = UiDriver::new();
        let base = r#"<ui-rad><property name="x" value="1" /><property name="y" value="2" /></ui-rad>"#;
        let ours = r#"<ui-rad><property name="x" value="1" /><property name="y" value="2" /></ui-rad>"#;
        let theirs = r#"<ui-rad><property name="x" value="1" /><property name="y" value="2" /></ui-rad>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(
            merged.contains("name=\"x\" value=\"1\""),
            "attr order preserved (name first)"
        );
    }

    #[test]
    fn test_correctness_mixed_text_and_child_modifications() {
        let driver = UiDriver::new();
        let base = r#"<root><parent>base_text<child>1</child></parent></root>"#;
        let ours = r#"<root><parent>ours_text<child>1</child></parent></root>"#;
        let theirs = r#"<root><parent>base_text<child>10</child></parent></root>"#;

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("ours_text"), "ours text change");
        assert!(merged.contains(">10<"), "theirs child change");
    }

    #[test]
    fn test_correctness_merge_associativity() {
        let driver = UiDriver::new();
        let base = r#"<root><a>1</a><b>2</b><c>3</c><d>4</d></root>"#;
        let a = r#"<root><a>10</a><b>2</b><c>3</c><d>4</d></root>"#;
        let b = r#"<root><a>1</a><b>20</b><c>3</c><d>4</d></root>"#;
        let c = r#"<root><a>1</a><b>2</b><c>30</c><d>4</d></root>"#;

        let ab = driver
            .merge(base, a, b)
            .unwrap()
            .expect("merge(base, A, B) should succeed");
        let merge_left = driver
            .merge(base, &ab, c)
            .unwrap()
            .expect("merge(base, merge(A,B), C) should succeed");

        let bc = driver
            .merge(base, b, c)
            .unwrap()
            .expect("merge(base, B, C) should succeed");
        let merge_right = driver
            .merge(base, a, &bc)
            .unwrap()
            .expect("merge(base, A, merge(B,C)) should succeed");

        let d_left = roxmltree::Document::parse(&merge_left).unwrap();
        let d_right = roxmltree::Document::parse(&merge_right).unwrap();
        let s_left = UiDriver::element_to_string(d_left.root(), 0);
        let s_right = UiDriver::element_to_string(d_right.root(), 0);

        assert_eq!(
            s_left, s_right,
            "merge(base, merge(A,B), C) must equal merge(base, A, merge(B,C))"
        );
        assert!(merge_left.contains(">10<"));
        assert!(merge_left.contains(">20<"));
        assert!(merge_left.contains(">30<"));
        assert!(merge_left.contains(">4<"));
    }

    /// 构造 Actions IDE 风格 .ui 文件（模拟真实 bt_watch.ui 结构）：
    /// - CRLF 行尾 + 尾随换行
    /// - 多个 scene，每个 scene 含大量跨 scene 完全重复的 `<property>` 行
    ///   （模拟真实文件的重复缩进/属性行，是触发行级 diff 错位的必要条件）
    /// - 所有 scene 的 string_resource 属性值完全相同（x=0x0014/y=0x0106/
    ///   width/height），使冲突行 `x=0x0014` 在 base 中**重复出现多次**，
    ///   且 drink 场景插在**中间**（该行不是 0x0014 的最后一次出现）——
    ///   这是旧贪心 diff 远距离匹配、连锁错位的触发条件
    /// - 每 scene 有唯一锚点行（name/id），供 patience diff 定位
    /// - `STR_DRINK_SOME_WATER` 的 x/y 属性值可定制（真实问题场景）
    /// - 总行数 > 2000，确保行级 fallback 走 linear（patience）路径而非 DP
    fn build_ui_file(scene_count: usize, drink_x: &str, drink_y: &str) -> String {
        let mut lines: Vec<String> = vec![
            "<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"no\" ?>".to_owned(),
            "<!--Actions IDE, Rapid Application Development-->".to_owned(),
            "<ui-rad>".to_owned(),
        ];
        for s in 0..scene_count {
            lines.push("    <scene>".to_owned());
            lines.push(format!("        <property name=\"name\" value=\"SCENE_{s}\" />"));
            lines.push(format!(
                "        <property name=\"id\" value=\"{}\" />",
                s + 30000
            ));
            // 20 行跨 scene 完全相同的重复属性（模拟真实文件的重复行）
            for k in 0..20 {
                lines.push(format!("        <property name=\"key{k}\" value=\"{k}\" />"));
            }
            lines.push("        <element class=\"string_resource\">".to_owned());
            lines.push(format!(
                "            <property name=\"name\" value=\"STR_{s}\" />"
            ));
            lines.push(format!(
                "            <property name=\"id\" value=\"{}\" />",
                s + 1000
            ));
            // 所有 scene 使用完全相同的属性值 → 制造大量重复行
            lines.push("            <property name=\"x\" value=\"0x0014\" />".to_owned());
            lines.push("            <property name=\"y\" value=\"0x0106\" />".to_owned());
            lines.push("            <property name=\"width\" value=\"0x0118\" />".to_owned());
            lines.push("            <property name=\"height\" value=\"0x0078\" />".to_owned());
            lines.push("        </element>".to_owned());
            lines.push("    </scene>".to_owned());

            // 把真实问题场景（SCENE_COLOR / STR_DRINK_SOME_WATER）插在中间：
            // 它后面的 scene 仍含同值 `x=0x0014`，是触发旧贪心连锁错位的必要条件
            if s == scene_count / 2 {
                lines.push("    <scene>".to_owned());
                lines.push("        <property name=\"name\" value=\"SCENE_COLOR\" />".to_owned());
                lines.push("        <property name=\"id\" value=\"32811\" />".to_owned());
                lines.push("        <element class=\"string_resource\">".to_owned());
                lines
                    .push("            <property name=\"name\" value=\"STR_DRINK_SOME_WATER\" />"
                        .to_owned());
                lines.push("            <property name=\"id\" value=\"31915\" />".to_owned());
                lines.push(format!("            <property name=\"x\" value=\"{drink_x}\" />"));
                lines.push(format!("            <property name=\"y\" value=\"{drink_y}\" />"));
                lines.push("            <property name=\"width\" value=\"0x0118\" />".to_owned());
                lines.push("            <property name=\"height\" value=\"0x0078\" />".to_owned());
                lines.push("        </element>".to_owned());
                lines.push("    </scene>".to_owned());
            }
        }
        lines.push("</ui-rad>".to_owned());

        let mut content = lines.join("\r\n");
        content.push_str("\r\n");
        content
    }

    /// 在 `</ui-rad>` 前追加一个完整场景：模拟分支在 scene 列表末尾
    /// （文件末尾、`<resource>` 之前）新增场景——与真实 bt_watch.ui 中
    /// 两个分支各自新增场景的插入位置一致。
    fn with_extra_scene(content: &str, scene_name: &str, scene_id: u32) -> String {
        let scene = format!(
            "    <scene>\r\n        <property name=\"name\" value=\"{scene_name}\" />\r\n        <property name=\"id\" value=\"{scene_id}\" />\r\n        <element class=\"string_resource\">\r\n            <property name=\"name\" value=\"STR_{scene_name}\" />\r\n            <property name=\"x\" value=\"0x0014\" />\r\n            <property name=\"y\" value=\"0x0106\" />\r\n        </element>\r\n    </scene>\r\n"
        );
        content.replace("</ui-rad>", &format!("{scene}</ui-rad>"))
    }

    /// 回归测试（2026-08-17，真实 bt_watch.ui 场景）：两个分支**同时**做三件事：
    /// ① 修改同一场景同一属性为不同值（对应 STR_DAY.x：0x0038 vs 0x0044）；
    /// ② 各自在 scene 列表末尾新增一个不同的场景
    ///    （SCENE_EMERGENCY_CONNECT_COPY vs SCENE_SPORT_RECORD_NO_DATA_COPY）；
    /// ③ 各自做一些不冲突的修改。
    ///
    /// 修复前：① 使语义层整体 decline 到行级合并 → 行级合并把 ② 的
    /// "同一位置插入不同场景"误判为冲突 → 输出 **2 个冲突区域**
    /// （x 属性 + 整个新场景块，实测真实文件）。
    /// 修复后：语义层局部降级——② 的场景新增走语义合并（两个都保留），
    /// 只有 ① 的属性行输出冲突标记 → **1 个冲突区域**。
    #[test]
    fn test_ui_partial_merge_keeps_both_new_scenes() {
        let base = build_ui_file(80, "0x0014", "0x0106");
        let ours = with_extra_scene(
            &build_ui_file(80, "0x0012", "0x0106"),
            "SCENE_EMERGENCY_CONNECT_COPY",
            53021,
        );
        let theirs = with_extra_scene(
            &build_ui_file(80, "0x0016", "0x0106"),
            "SCENE_SPORT_RECORD_NO_DATA_COPY",
            53021,
        );

        let driver = UiDriver::new();
        let merged = driver
            .merge(&base, &ours, &theirs)
            .unwrap()
            .expect("partial merge must produce output");

        // 两个新场景都在（不冲突，语义层保留）——这是本次修复的核心
        assert!(
            merged.contains("SCENE_EMERGENCY_CONNECT_COPY"),
            "ours new scene must be kept"
        );
        assert!(
            merged.contains("SCENE_SPORT_RECORD_NO_DATA_COPY"),
            "theirs new scene must be kept"
        );

        // 恰好 1 个冲突区域（只有 x 属性）
        let starts = merged.matches("<<<<<<< ").count();
        let divs = merged.matches("=======").count();
        let ends = merged.matches(">>>>>>> ").count();
        assert_eq!(
            (starts, divs, ends),
            (1, 1, 1),
            "only the x-attr conflict should remain"
        );

        // 冲突两侧内容正确（ours 在前、theirs 在后）
        let ours_idx = merged.find("0x0012").expect("ours value in conflict");
        let theirs_idx = merged.find("0x0016").expect("theirs value in conflict");
        assert!(ours_idx < theirs_idx, "ours block before theirs block");

        // 冲突标记行顶格（VS Code/git 识别要求行首）
        let markers: Vec<&str> = merged
            .lines()
            .filter(|l| {
                l.starts_with("<<<<<<< ") || *l == "=======" || l.starts_with(">>>>>>> ")
            })
            .collect();
        assert_eq!(markers.len(), 3, "3 marker lines total");
        assert!(
            markers.iter().all(|l| !l.starts_with(' ')),
            "conflict markers must be at line start"
        );

        // 移除冲突标记后 XML 可解析，且两个新场景仍在（内容完整）
        let cleaned = merged
            .lines()
            .filter(|l| !l.starts_with("<<<<<<< ") && *l != "=======" && !l.starts_with(">>>>>>> "))
            .collect::<Vec<_>>()
            .join("\r\n");
        let doc = roxmltree::Document::parse(&cleaned).unwrap();
        assert_eq!(doc.root_element().tag_name().name(), "ui-rad");
        assert!(cleaned.contains("SCENE_EMERGENCY_CONNECT_COPY"));
        assert!(cleaned.contains("SCENE_SPORT_RECORD_NO_DATA_COPY"));
    }

    /// 回归测试（2026-08-14 + 2026-08-17 更新）：两个分支**同时修改同一个场景
    /// 的同一处属性**（`SCENE_COLOR` 场景 `STR_DRINK_SOME_WATER` 的 `x` 属性）
    /// 且内容不同。
    /// - 2026-08-14 修复（patience diff）：行级 fallback 在重复行上不再错位，
    ///   只产生 1 行真实冲突、无膨胀。
    /// - 2026-08-17 修复（局部降级）：语义层不再整体 decline——属性冲突改为
    ///   对该元素子树做行级合并，输出带冲突标记的部分合并结果，其余部分
    ///   继续语义合并（否则同位置新增的不同场景会被行级合并误判为冲突）。
    #[test]
    fn test_ui_same_scene_same_attr_conflict_single_line() {
        let base = build_ui_file(80, "0x0014", "0x0106");
        let ours = build_ui_file(80, "0x0012", "0x0106");
        let theirs = build_ui_file(80, "0x0016", "0x0106");
        assert!(base.lines().count() > 2000, "must exceed DP threshold");

        let driver = UiDriver::new();

        // ① 语义层：同一属性两边值不同（0x0012 vs 0x0016）→ 局部降级，
        //    返回含恰好 1 组冲突标记的部分合并结果（不再整体 decline）。
        let semantic = driver.merge(&base, &ours, &theirs).unwrap();
        let merged = semantic.expect("partial merge must produce output");
        assert_eq!(
            merged.matches("<<<<<<< ").count(),
            1,
            "exactly one conflict region in partial merge"
        );
        assert!(merged.contains("0x0012"), "ours value in conflict");
        assert!(merged.contains("0x0016"), "theirs value in conflict");

        // ② 行级 fallback（merge_subtree_lines 底层）：只应产生 1 行冲突。
        //    替换型冲突输出 5 行（3 行标记 + ours 1 行 + theirs 1 行），
        //    消耗 base 1 行，净增 +4（实测真实 bt_watch.ui：108933 → 108937）。
        let b: Vec<&str> = base.lines().collect();
        let o: Vec<&str> = ours.lines().collect();
        let t: Vec<&str> = theirs.lines().collect();
        let result =
            suture_core::engine::merge::three_way_merge_lines(&b, &o, &t, "ours", "theirs");

        assert!(!result.is_clean);
        assert_eq!(result.conflicts, 1, "exactly one real conflict expected");
        assert_eq!(
            result.lines.len(),
            b.len() + 4,
            "no spurious lines: base {} + 4 (1 replaced by 3 markers + 2 sides) = {}, got {}",
            b.len(),
            b.len() + 4,
            result.lines.len()
        );
        // 恰好一组冲突标记
        let starts = result
            .lines
            .iter()
            .filter(|l| l.starts_with("<<<<<<<"))
            .count();
        let divs = result
            .lines
            .iter()
            .filter(|l| l.as_str() == "=======")
            .count();
        let ends = result
            .lines
            .iter()
            .filter(|l| l.starts_with(">>>>>>>"))
            .count();
        assert_eq!((starts, divs, ends), (1, 1, 1), "one conflict region only");

        // 冲突两侧内容正确（ours 在前、theirs 在后）
        let ours_idx = result
            .lines
            .iter()
            .position(|l| l.contains("0x0012"))
            .expect("ours value in conflict");
        let theirs_idx = result
            .lines
            .iter()
            .position(|l| l.contains("0x0016"))
            .expect("theirs value in conflict");
        assert!(ours_idx < theirs_idx, "ours block before theirs block");

        // 冲突区域之外的行必须与 base 完全一致：不重复、不丢失
        let mut count: std::collections::HashMap<&str, usize> =
            std::collections::HashMap::new();
        for l in &b {
            *count.entry(l).or_insert(0) += 1;
        }
        // base 的 0x0014 行被两边同时替换掉，从计数中扣除
        let x14 = b
            .iter()
            .find(|l| l.contains("0x0014"))
            .expect("base x line");
        if let Some(c) = count.get_mut(x14) {
            *c -= 1;
        }
        let mut unmatched = 0usize;
        for l in &result.lines {
            if l.starts_with("<<<<<<<")
                || l.starts_with("=======")
                || l.starts_with(">>>>>>>")
                || l.contains("0x0012")
                || l.contains("0x0016")
            {
                continue;
            }
            match count.get_mut(l.as_str()) {
                Some(c) if *c > 0 => *c -= 1,
                _ => unmatched += 1,
            }
        }
        assert_eq!(
            unmatched, 0,
            "merged output must not contain spurious/duplicated lines"
        );
    }

    /// .ui 固定输出 CRLF 行尾：成功合并（无冲突）时，输出必须全 CRLF
    /// （含尾随换行），不得出现裸 LF——否则 `-text` 下 git 逐字节比较，
    /// 行尾不一致会导致解决冲突后全文件 diff。
    #[test]
    fn test_ui_merge_success_fixed_crlf() {
        let driver = UiDriver::new();
        // ours 改 STR_DRINK_SOME_WATER.x，theirs 改同一元素的 y（不同属性）
        // → 语义层可自动合并，验证成功路径的行尾
        let base = build_ui_file(80, "0x0014", "0x0106");
        let ours = build_ui_file(80, "0x0012", "0x0106");
        let theirs = build_ui_file(80, "0x0014", "0x0122");

        let merged = driver
            .merge(&base, &ours, &theirs)
            .unwrap()
            .expect("different attrs must auto-merge");
        assert!(merged.contains("0x0012"), "ours x change applied");
        assert!(merged.contains("0x0122"), "theirs y change applied");

        // 全 CRLF：去掉 \r\n 后不得残留任何 \n（即无裸 LF）
        assert!(
            !merged.replace("\r\n", "").contains('\n'),
            "output must use CRLF line endings only"
        );
        // 尾随换行也必须是 CRLF
        assert!(merged.ends_with("\r\n"), "trailing CRLF preserved");
        // 可被 XML 解析（输出有效）
        let doc = roxmltree::Document::parse(&merged).unwrap();
        assert_eq!(doc.root_element().tag_name().name(), "ui-rad");
    }

    proptest! {
        #[test]
        fn test_merge_identity(content in "[a-z0-9]+") {
            let xml = format!("<root>{}</root>", content);
            let driver = UiDriver::new();
            let result = driver.merge(&xml, &xml, &xml).unwrap();
            prop_assert!(result.is_some());
            let merged = result.unwrap();
            prop_assert!(merged.contains(&content));
        }

        #[test]
        fn test_merge_idempotence(
            base in "[a-z0-9]+",
            modified in "[a-z0-9]+",
        ) {
            let base_xml = format!("<root>{}</root>", base);
            let modified_xml = format!("<root>{}</root>", modified);
            let driver = UiDriver::new();
            let result = driver.merge(&base_xml, &modified_xml, &modified_xml).unwrap();
            prop_assert!(result.is_some());
            let merged = result.unwrap();
            prop_assert!(merged.contains(&modified));
        }

        #[test]
        fn test_xml_merge_non_overlapping_elements(
            tag in "[a-z][a-z0-9]*",
            attr1 in "[a-z][a-z0-9]*",
            child1 in "[a-z][a-z0-9]*",
            child2 in "[a-z][a-z0-9]*",
            val1 in "[a-z0-9]+",
            val2 in "[a-z0-9]+",
            val3 in "[a-z0-9]+",
        ) {
            let base = format!("<{tag}><{child1}>{val1}</{child1}><{child2}>{val2}</{child2}></{tag}>");
            let ours = format!("<{tag} {attr1}=\"1\"><{child1}>{val1}</{child1}><{child2}>{val2}</{child2}></{tag}>");
            let theirs = format!("<{tag}><{child1}>{val1}</{child1}><{child2}>{val3}</{child2}></{tag}>");

            let driver = UiDriver::new();
            let result = driver.merge(&base, &ours, &theirs);
            prop_assert!(result.is_ok());
            prop_assert!(result.unwrap().is_some());
        }
    }
}

#[cfg(test)]
mod fuzz {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn proptest_crash_resistance(input in proptest::string::string_regex("[a-zA-Z0-9 \\t\\n.,;:!\\-\\+\\*\\/\\(\\)\\[\\]\\{\\}<>\"'=&#~_@]{0,1000}").unwrap()) {
            let driver = UiDriver::new();
            let _ = driver.diff(None, &input);
        }

        #[test]
        fn proptest_arbitrary_bytes_dont_panic(input in proptest::collection::vec(proptest::arbitrary::any::<u8>(), 0..5000)) {
            let driver = UiDriver::new();
            let s = String::from_utf8_lossy(&input);
            let _ = driver.diff(None, &s);
        }

        #[test]
        fn proptest_deterministic(input in proptest::string::string_regex("[a-zA-Z0-9 \\t\\n]{0,500}").unwrap()) {
            let driver = UiDriver::new();
            let r1 = driver.diff(None, &input);
            let r2 = driver.diff(None, &input);
            assert_eq!(format!("{:?}", r1), format!("{:?}", r2))
        }
    }
}
