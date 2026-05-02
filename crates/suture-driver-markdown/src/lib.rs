// SPDX-License-Identifier: MIT OR Apache-2.0
use suture_driver::{DriverError, SemanticChange, SutureDriver};

#[derive(Debug, Clone, PartialEq)]
enum BlockType {
    Heading,
    CodeBlock,
    ListItem,
    Table,
    Paragraph,
}

#[derive(Debug, Clone)]
#[allow(clippy::struct_field_names)]
struct Block {
    block_type: BlockType,
    heading: Option<String>,
    lines: Vec<String>,
}

impl PartialEq for Block {
    fn eq(&self, other: &Self) -> bool {
        self.block_type == other.block_type
            && self.heading == other.heading
            && self.lines == other.lines
    }
}

impl Block {
    fn path(&self) -> String {
        self.heading.as_ref().map_or_else(
            || match self.block_type {
                BlockType::CodeBlock => "/code".to_owned(),
                BlockType::ListItem => "/list".to_owned(),
                BlockType::Table => "/table".to_owned(),
                BlockType::Paragraph => "/paragraph".to_owned(),
                BlockType::Heading => "/".to_owned(),
            },
            |h| format!("/{h}"),
        )
    }

    fn content_str(&self) -> String {
        self.lines.join("\n")
    }
}

fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

fn detect_block_type(line: &str) -> BlockType {
    if line.starts_with('#') {
        BlockType::Heading
    } else if line.trim_start().starts_with("```") {
        BlockType::CodeBlock
    } else if line.trim_start().starts_with("- ")
        || line.trim_start().starts_with("* ")
        || line.trim_start().starts_with("+ ")
        || regex_is_numbered_list(line)
    {
        BlockType::ListItem
    } else if line.trim_start().starts_with('|') {
        BlockType::Table
    } else {
        BlockType::Paragraph
    }
}

fn regex_is_numbered_list(line: &str) -> bool {
    let trimmed = line.trim_start();
    let mut chars = trimmed.chars();
    let mut has_digit = false;
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            has_digit = true;
        } else if c == '.' && has_digit && chars.next().is_none_or(char::is_whitespace) {
            return true;
        } else {
            break;
        }
    }
    false
}

fn parse_blocks(content: &str) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut current_block: Option<Block> = None;
    let mut current_heading: Option<String> = None;
    let mut in_code_block = false;

    /// Flush the current block into `blocks` if one exists.
    /// Returns `true` if a block was flushed.
    macro_rules! flush_block {
        () => {
            if let Some(block) = current_block.take() {
                blocks.push(block);
            }
        };
    }

    for line in content.lines() {
        if in_code_block {
            if let Some(ref mut block) = current_block {
                block.lines.push(line.to_owned());
            }
            if line.trim_start().starts_with("```") {
                in_code_block = false;
                flush_block!();
            }
            continue;
        }

        if line.trim_start().starts_with("```") {
            flush_block!();
            in_code_block = true;
            current_block = Some(Block {
                block_type: BlockType::CodeBlock,
                heading: current_heading.clone(),
                lines: vec![line.to_owned()],
            });
            continue;
        }

        if is_blank(line) {
            flush_block!();
            continue;
        }

        let bt = detect_block_type(line);

        if bt == BlockType::Heading {
            flush_block!();
            let heading_text = line.trim_start_matches('#').trim().to_owned();
            current_heading = Some(heading_text.clone());
            current_block = Some(Block {
                block_type: BlockType::Heading,
                heading: Some(heading_text),
                lines: vec![line.to_owned()],
            });
        } else {
            if let Some(ref mut block) = current_block {
                if block.block_type == bt {
                    block.lines.push(line.to_owned());
                    continue;
                }
                flush_block!();
            }
            current_block = Some(Block {
                block_type: bt,
                heading: current_heading.clone(),
                lines: vec![line.to_owned()],
            });
        }
    }

    flush_block!();
    blocks
}

fn blocks_to_markdown(blocks: &[Block]) -> String {
    let mut result = String::new();
    for (i, block) in blocks.iter().enumerate() {
        if i > 0 {
            result.push_str("\n\n");
        }
        result.push_str(&block.lines.join("\n"));
    }
    result
}

fn match_blocks(base: &[Block], other: &[Block]) -> Vec<(Option<usize>, Option<usize>)> {
    let mut pairs: Vec<(Option<usize>, Option<usize>)> = Vec::new();
    let mut used_base: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut used_other: std::collections::HashSet<usize> = std::collections::HashSet::new();

    for (oi, ob) in other.iter().enumerate() {
        let mut found = None;
        for (idx, bb) in base.iter().enumerate() {
            if bb.heading.is_some() && bb.heading == ob.heading && !used_base.contains(&idx) {
                found = Some(idx);
                break;
            }
        }
        if let Some(idx) = found {
            pairs.push((Some(idx), Some(oi)));
            used_base.insert(idx);
            used_other.insert(oi);
        }
    }

    for bi in 0..base.len() {
        if !used_base.contains(&bi) {
            pairs.push((Some(bi), None));
        }
    }

    for oi in 0..other.len() {
        if !used_other.contains(&oi) {
            pairs.push((None, Some(oi)));
        }
    }

    pairs.sort_by(|a, b| {
        let a_key = a.0.unwrap_or_else(|| a.1.unwrap_or(0));
        let b_key = b.0.unwrap_or_else(|| b.1.unwrap_or(0));
        a_key.cmp(&b_key)
    });

    pairs
}

pub struct MarkdownDriver;

impl MarkdownDriver {
    #[must_use] 
    pub fn new() -> Self {
        Self
    }
}

impl Default for MarkdownDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl SutureDriver for MarkdownDriver {
    fn name(&self) -> &'static str {
        "Markdown"
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".md", ".markdown", ".mdown", ".mkd"]
    }

    fn diff(
        &self,
        base_content: Option<&str>,
        new_content: &str,
    ) -> Result<Vec<SemanticChange>, DriverError> {
        let new_blocks = parse_blocks(new_content);

        base_content.map_or_else(
            || {
                let mut changes = Vec::new();
                for block in &new_blocks {
                    changes.push(SemanticChange::Added {
                        path: block.path(),
                        value: block.content_str(),
                    });
                }
                Ok(changes)
            },
            |base| {
                let base_blocks = parse_blocks(base);
                let pairs = match_blocks(&base_blocks, &new_blocks);
                let mut changes = Vec::new();

                for (base_idx, new_idx) in &pairs {
                    match (*base_idx, *new_idx) {
                        (Some(bi), None) => {
                            let block = &base_blocks[bi];
                            changes.push(SemanticChange::Removed {
                                path: block.path(),
                                old_value: block.content_str(),
                            });
                        }
                        (None, Some(ni)) => {
                            let block = &new_blocks[ni];
                            changes.push(SemanticChange::Added {
                                path: block.path(),
                                value: block.content_str(),
                            });
                        }
                        (Some(bi), Some(ni)) => {
                            let base_block = &base_blocks[bi];
                            let new_block = &new_blocks[ni];
                            if base_block.lines != new_block.lines {
                                changes.push(SemanticChange::Modified {
                                    path: new_block.path(),
                                    old_value: base_block.content_str(),
                                    new_value: new_block.content_str(),
                                });
                            }
                        }
                        (None, None) => {}
                    }
                }

                Ok(changes)
            },
        )
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

        let lines: Vec<String> = changes
            .iter()
            .map(|change| match change {
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
                    format!("  MODIFIED  {path}: {old_value} → {new_value}")
                }
                SemanticChange::Moved {
                    old_path,
                    new_path,
                    value,
                } => {
                    format!("  MOVED     {old_path} → {new_path}: {value}")
                }
            })
            .collect();

        Ok(lines.join("\n"))
    }

    fn merge(&self, base: &str, ours: &str, theirs: &str) -> Result<Option<String>, DriverError> {
        let base_blocks = parse_blocks(base);
        let ours_blocks = parse_blocks(ours);
        let theirs_blocks = parse_blocks(theirs);

        let ours_pairs = match_blocks(&base_blocks, &ours_blocks);
        let theirs_pairs = match_blocks(&base_blocks, &theirs_blocks);

        let mut ours_by_base: std::collections::HashMap<usize, usize> =
            std::collections::HashMap::new();
        let mut ours_only: std::vec::Vec<usize> = Vec::new();
        for (bi, oi) in &ours_pairs {
            match (*bi, *oi) {
                (Some(b), Some(o)) => {
                    ours_by_base.insert(b, o);
                }
                (None, Some(o)) => {
                    ours_only.push(o);
                }
                _ => {}
            }
        }

        let mut theirs_by_base: std::collections::HashMap<usize, usize> =
            std::collections::HashMap::new();
        let mut theirs_only: std::vec::Vec<usize> = Vec::new();
        for (bi, ti) in &theirs_pairs {
            match (*bi, *ti) {
                (Some(b), Some(t)) => {
                    theirs_by_base.insert(b, t);
                }
                (None, Some(t)) => {
                    theirs_only.push(t);
                }
                _ => {}
            }
        }

        let mut merged_blocks: std::vec::Vec<Block> = Vec::new();

        for (bi, base_block) in base_blocks.iter().enumerate() {
            let in_ours = ours_by_base.get(&bi).copied();
            let in_theirs = theirs_by_base.get(&bi).copied();

            match (in_ours, in_theirs) {
                (Some(oi), Some(ti)) => {
                    let ours_block = &ours_blocks[oi];
                    let theirs_block = &theirs_blocks[ti];

                    if ours_block == theirs_block {
                        merged_blocks.push(ours_block.clone());
                    } else if ours_block == base_block {
                        merged_blocks.push(theirs_block.clone());
                    } else if theirs_block == base_block {
                        merged_blocks.push(ours_block.clone());
                    } else {
                        return Ok(None);
                    }
                }
                (Some(oi), None) => {
                    merged_blocks.push(ours_blocks[oi].clone());
                }
                (None, Some(ti)) => {
                    merged_blocks.push(theirs_blocks[ti].clone());
                }
                (None, None) => {
                    // removed by both — skip
                }
            }
        }

        let mut matched_added: std::collections::HashSet<usize> = std::collections::HashSet::new();
        let mut matched_theirs_added: std::collections::HashSet<usize> =
            std::collections::HashSet::new();

        for &oi in &ours_only {
            if ours_blocks[oi].heading.is_none() {
                continue;
            }
            for &ti in &theirs_only {
                if matched_added.contains(&oi) || matched_theirs_added.contains(&ti) {
                    continue;
                }
                if theirs_blocks[ti].heading.is_some()
                    && ours_blocks[oi].heading == theirs_blocks[ti].heading
                {
                    if ours_blocks[oi] == theirs_blocks[ti] {
                        merged_blocks.push(ours_blocks[oi].clone());
                    } else {
                        return Ok(None);
                    }
                    matched_added.insert(oi);
                    matched_theirs_added.insert(ti);
                    break;
                }
            }
        }

        for &oi in &ours_only {
            if !matched_added.contains(&oi) {
                merged_blocks.push(ours_blocks[oi].clone());
            }
        }
        for &ti in &theirs_only {
            if !matched_theirs_added.contains(&ti) {
                merged_blocks.push(theirs_blocks[ti].clone());
            }
        }

        Ok(Some(blocks_to_markdown(&merged_blocks)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_driver_name() {
        let driver = MarkdownDriver::new();
        assert_eq!(driver.name(), "Markdown");
    }

    #[test]
    fn test_driver_extensions() {
        let driver = MarkdownDriver::new();
        assert_eq!(
            driver.supported_extensions(),
            &[".md", ".markdown", ".mdown", ".mkd"]
        );
    }

    #[test]
    fn test_parse_headings_and_paragraphs() {
        let content = "# Title\n\nSome text.\n\n## Section\n\nMore text.";
        let blocks = parse_blocks(content);
        assert_eq!(blocks.len(), 4);
        assert_eq!(blocks[0].block_type, BlockType::Heading);
        assert_eq!(blocks[0].heading.as_deref(), Some("Title"));
        assert_eq!(blocks[1].block_type, BlockType::Paragraph);
        assert_eq!(blocks[2].block_type, BlockType::Heading);
        assert_eq!(blocks[2].heading.as_deref(), Some("Section"));
        assert_eq!(blocks[3].block_type, BlockType::Paragraph);
    }

    #[test]
    fn test_parse_code_block() {
        let content = "# Intro\n\n```rust\nfn main() {}\n```\n\nEnd.";
        let blocks = parse_blocks(content);
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].block_type, BlockType::Heading);
        assert_eq!(blocks[1].block_type, BlockType::CodeBlock);
        assert_eq!(blocks[1].lines.len(), 3);
        assert_eq!(blocks[2].block_type, BlockType::Paragraph);
    }

    #[test]
    fn test_parse_list() {
        let content = "# Items\n\n- one\n- two\n- three";
        let blocks = parse_blocks(content);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[1].block_type, BlockType::ListItem);
        assert_eq!(blocks[1].lines.len(), 3);
    }

    #[test]
    fn test_parse_table() {
        let content = "| Col1 | Col2 |\n| --- | --- |\n| a | b |";
        let blocks = parse_blocks(content);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_type, BlockType::Table);
    }

    #[test]
    fn test_diff_new_file() {
        let driver = MarkdownDriver::new();
        let content = "# Hello\n\nWorld.";
        let changes = driver.diff(None, content).unwrap();
        assert_eq!(changes.len(), 2);
        assert!(matches!(&changes[0], SemanticChange::Added { .. }));
    }

    #[test]
    fn test_diff_no_changes() {
        let driver = MarkdownDriver::new();
        let content = "# Hello\n\nWorld.";
        let changes = driver.diff(Some(content), content).unwrap();
        assert!(changes.is_empty());
    }

    #[test]
    fn test_diff_added_section() {
        let driver = MarkdownDriver::new();
        let base = "# Intro\n\nHello.";
        let new = "# Intro\n\nHello.\n\n## New Section\n\nContent.";
        let changes = driver.diff(Some(base), new).unwrap();
        let added: Vec<_> = changes
            .iter()
            .filter(|c| matches!(c, SemanticChange::Added { .. }))
            .collect();
        assert_eq!(added.len(), 2);
    }

    #[test]
    fn test_diff_removed_section() {
        let driver = MarkdownDriver::new();
        let base = "# Intro\n\nHello.\n\n## Remove Me\n\nGone.";
        let new = "# Intro\n\nHello.";
        let changes = driver.diff(Some(base), new).unwrap();
        let removed: Vec<_> = changes
            .iter()
            .filter(|c| matches!(c, SemanticChange::Removed { .. }))
            .collect();
        assert_eq!(removed.len(), 2);
    }

    #[test]
    fn test_diff_modified_paragraph() {
        let driver = MarkdownDriver::new();
        let base = "# Title\n\nOld text.";
        let new = "# Title\n\nNew text.";
        let changes = driver.diff(Some(base), new).unwrap();
        assert_eq!(changes.len(), 1);
        assert!(matches!(&changes[0], SemanticChange::Modified { .. }));
    }

    #[test]
    fn test_format_diff() {
        let driver = MarkdownDriver::new();
        let base = "# Title\n\nOld.";
        let new = "# Title\n\nNew.\n\n## Added\n\nContent.";
        let output = driver.format_diff(Some(base), new).unwrap();
        assert!(output.contains("MODIFIED"));
        assert!(output.contains("ADDED"));
    }

    #[test]
    fn test_format_diff_empty() {
        let driver = MarkdownDriver::new();
        let content = "# Title\n\nHello.";
        let output = driver.format_diff(Some(content), content).unwrap();
        assert_eq!(output, "no changes");
    }

    #[test]
    fn test_merge_no_conflict() {
        let driver = MarkdownDriver::new();
        let base = "# Title\n\nBase paragraph.\n\n## Section A\n\nBase A.";
        let ours = "# Title\n\nOur paragraph.\n\n## Section A\n\nBase A.";
        let theirs = "# Title\n\nBase paragraph.\n\n## Section A\n\nTheir A.";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("Our paragraph."));
        assert!(merged.contains("Their A."));
    }

    #[test]
    fn test_merge_conflict() {
        let driver = MarkdownDriver::new();
        let base = "# Title\n\nBase text.";
        let ours = "# Title\n\nOur text.";
        let theirs = "# Title\n\nTheir text.";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_merge_identical() {
        let driver = MarkdownDriver::new();
        let content = "# Title\n\nParagraph.";
        let result = driver.merge(content, content, content).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), content);
    }

    #[test]
    fn test_merge_both_add_different_sections() {
        let driver = MarkdownDriver::new();
        let base = "# Title\n\nShared.";
        let ours = "# Title\n\nShared.\n\n## Ours\n\nOurs content.";
        let theirs = "# Title\n\nShared.\n\n## Theirs\n\nTheirs content.";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("Ours content."));
        assert!(merged.contains("Theirs content."));
    }

    #[test]
    fn test_merge_both_add_same_section_conflict() {
        let driver = MarkdownDriver::new();
        let base = "# Title\n\nShared.";
        let ours = "# Title\n\nShared.\n\n## New\n\nOurs version.";
        let theirs = "# Title\n\nShared.\n\n## New\n\nTheirs version.";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_blocks_to_markdown_roundtrip() {
        let content = "# Title\n\nParagraph one.\n\n## Section\n\nParagraph two.";
        let blocks = parse_blocks(content);
        let output = blocks_to_markdown(&blocks);
        let blocks2 = parse_blocks(&output);
        assert_eq!(blocks.len(), blocks2.len());
        for (a, b) in blocks.iter().zip(blocks2.iter()) {
            assert_eq!(a.lines, b.lines);
        }
    }

    #[test]
    fn test_diff_with_code_block() {
        let driver = MarkdownDriver::new();
        let base = "# Example\n\n```rust\nfn old() {}\n```";
        let new = "# Example\n\n```rust\nfn new() {}\n```";
        let changes = driver.diff(Some(base), new).unwrap();
        assert_eq!(changes.len(), 1);
        assert!(matches!(&changes[0], SemanticChange::Modified { .. }));
    }

    #[test]
    fn test_numbered_list_detection() {
        assert!(regex_is_numbered_list("1. item"));
        assert!(regex_is_numbered_list("  10. item"));
        assert!(!regex_is_numbered_list("abc. not a list"));
        assert!(!regex_is_numbered_list("just text"));
    }

    #[test]
    fn test_correctness_merge_determinism() {
        let driver = MarkdownDriver::new();
        let base = "# Title\n\nBase paragraph.\n\n## Section A\n\nBase A.";
        let ours = "# Title\n\nOur paragraph.\n\n## Section A\n\nBase A.";
        let theirs = "# Title\n\nBase paragraph.\n\n## Section A\n\nTheir A.";

        let r1 = driver.merge(base, ours, theirs).unwrap();
        let r2 = driver.merge(base, theirs, ours).unwrap();
        assert_eq!(r1.is_some(), r2.is_some());
        if let (Some(m1), Some(m2)) = (r1, r2) {
            let b1 = parse_blocks(&m1);
            let b2 = parse_blocks(&m2);
            assert_eq!(b1.len(), b2.len(), "block count must match");
            for (a, b) in b1.iter().zip(b2.iter()) {
                assert_eq!(a.lines, b.lines, "blocks must match in order");
            }
        }
    }

    #[test]
    fn test_correctness_merge_idempotency() {
        let driver = MarkdownDriver::new();
        let base = "# Title\n\nBase paragraph.";
        let ours = "# Title\n\nOur paragraph.";

        let result = driver.merge(base, ours, ours).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("Our paragraph."));
    }

    #[test]
    fn test_correctness_base_equals_ours() {
        let driver = MarkdownDriver::new();
        let base = "# Title\n\nBase paragraph.";
        let theirs = "# Title\n\nTheir paragraph.";

        let result = driver.merge(base, base, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert_eq!(
            merged, theirs,
            "merge(base, base, theirs) should equal theirs"
        );
    }

    #[test]
    fn test_correctness_base_equals_theirs() {
        let driver = MarkdownDriver::new();
        let base = "# Title\n\nBase paragraph.";
        let ours = "# Title\n\nOur paragraph.";

        let result = driver.merge(base, ours, base).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert_eq!(merged, ours, "merge(base, ours, base) should equal ours");
    }

    #[test]
    fn test_correctness_all_equal() {
        let driver = MarkdownDriver::new();
        let content = "# Title\n\nParagraph.";

        let result = driver.merge(content, content, content).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), content);
    }

    #[test]
    fn test_correctness_both_add_different_sections() {
        let driver = MarkdownDriver::new();
        let base = "# Title\n\nShared.";
        let ours = "# Title\n\nShared.\n\n## Ours\n\nOurs content.";
        let theirs = "# Title\n\nShared.\n\n## Theirs\n\nTheirs content.";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("## Ours"));
        assert!(merged.contains("Ours content."));
        assert!(merged.contains("## Theirs"));
        assert!(merged.contains("Theirs content."));
        assert!(merged.contains("Shared."));
    }

    #[test]
    fn test_correctness_both_modify_same_section_same_content() {
        let driver = MarkdownDriver::new();
        let base = "# Title\n\nBase text.";
        let ours = "# Title\n\nChanged text.";
        let theirs = "# Title\n\nChanged text.";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some(), "identical changes should not conflict");
        let merged = result.unwrap();
        assert!(merged.contains("Changed text."));
    }

    #[test]
    fn test_correctness_both_modify_same_section_different_content() {
        let driver = MarkdownDriver::new();
        let base = "# Title\n\nBase text.";
        let ours = "# Title\n\nOur text.";
        let theirs = "# Title\n\nTheir text.";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_correctness_deeply_nested_structure() {
        let driver = MarkdownDriver::new();
        let base =
            "# Title\n\nIntro.\n\n## Section A\n\nA content.\n\n### Subsection\n\nSub content.";
        let ours = "# Title\n\nModified intro.\n\n## Section A\n\nA content.\n\n### Subsection\n\nSub content.";
        let theirs = "# Title\n\nIntro.\n\n## Section A\n\nA content.\n\n### Subsection\n\nModified sub content.";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("Modified intro."));
        assert!(merged.contains("Modified sub content."));
        assert!(merged.contains("A content."));
    }

    #[test]
    fn test_correctness_unicode_content() {
        let driver = MarkdownDriver::new();
        let base = "# タイトル\n\n基本テキスト。\n\n## セクション\n\nセクション内容。";
        let ours = "# タイトル\n\n修正テキスト。\n\n## セクション\n\nセクション内容。";
        let theirs = "# タイトル\n\n基本テキスト。\n\n## セクション\n\n修正セクション。";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("修正テキスト。"));
        assert!(merged.contains("修正セクション。"));
    }

    #[test]
    fn test_correctness_large_file() {
        let driver = MarkdownDriver::new();
        let mut base_sections = vec!["# Title".to_string()];
        let mut ours_sections = vec!["# Title".to_string()];
        let mut theirs_sections = vec!["# Title".to_string()];

        for i in 0..100 {
            let section = format!("## Section {i}\n\nContent for section {i}.");
            base_sections.push(section.clone());
            ours_sections.push(if i == 50 {
                format!("## Section {i}\n\nModified by ours.")
            } else {
                section.clone()
            });
            theirs_sections.push(if i == 80 {
                format!("## Section {i}\n\nModified by theirs.")
            } else {
                section
            });
        }

        let base = base_sections.join("\n\n");
        let ours = ours_sections.join("\n\n");
        let theirs = theirs_sections.join("\n\n");

        let result = driver.merge(&base, &ours, &theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("Modified by ours."));
        assert!(merged.contains("Modified by theirs."));
        assert!(merged.contains("Content for section 0."));
        assert!(merged.contains("Content for section 99."));
    }

    #[test]
    fn test_correctness_heading_renumbering_merge() {
        let driver = MarkdownDriver::new();
        let base = "# Intro\n\nText.\n\n## Section\n\nContent.";
        let ours = "# Introduction\n\nText.\n\n## Section\n\nContent.";
        let theirs = "# Intro\n\nText.\n\n## Details\n\nContent.";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("# Introduction"), "ours heading change");
        assert!(merged.contains("## Details"), "theirs heading change");
    }

    #[test]
    fn test_correctness_code_block_merge() {
        let driver = MarkdownDriver::new();
        let base = "# Example\n\n```rust\nfn main() {\n    println!(\"Hello\");\n}\n```";
        let ours = "# Example\n\n```rust\nfn main() {\n    println!(\"World\");\n}\n```";
        let theirs = "# Example\n\n```rust\nfn main() {\n    println!(\"Hello\");\n}\n```";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(
            merged.contains("World"),
            "ours change should be applied when theirs unchanged"
        );
    }

    #[test]
    fn test_correctness_code_block_merge_conflict() {
        let driver = MarkdownDriver::new();
        let base = "# Example\n\n```rust\nfn main() {}\n```";
        let ours = "# Example\n\n```rust\nfn ours() {}\n```";
        let theirs = "# Example\n\n```rust\nfn theirs() {}\n```";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(
            result.is_none(),
            "conflicting code block changes should conflict"
        );
    }

    #[test]
    fn test_correctness_link_reference_merge() {
        let driver = MarkdownDriver::new();
        let base = "# Links\n\n[home](https://example.com)\n\n[about](https://example.com/about)";
        let ours = "# Links\n\n[home](https://example.com)\n\n[about](https://example.com/about)\n\n[contact](https://example.com/contact)";
        let theirs = "# Links\n\n[home](https://example.com)\n\n[about](https://example.com/about)";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(
            merged.contains("[contact]"),
            "ours added link should be present"
        );
    }

    #[test]
    fn test_correctness_list_merge() {
        let driver = MarkdownDriver::new();
        let base = "# Items\n\n- one\n- two\n- three";
        let ours = "# Items\n\n- one\n- modified\n- three";
        let theirs = "# Items\n\n- one\n- two\n- three\n\n## New Section\n\nAdded.";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("modified"));
        assert!(merged.contains("New Section"));
    }

    #[test]
    fn test_correctness_section_deletion_by_ours() {
        let driver = MarkdownDriver::new();
        let base = "# Title\n\nKeep.\n\n## Remove\n\nGone.";
        let ours = "# Title\n\nKeep.";
        let theirs = "# Title\n\nKeep.\n\n## Remove\n\nGone.";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(
            merged.contains("Remove"),
            "ours deleted the section but theirs kept it → theirs preserved"
        );
    }

    #[test]
    fn test_correctness_multiple_paragraphs_under_same_heading() {
        let driver = MarkdownDriver::new();
        let base = "# Section\n\nParagraph one.\n\nParagraph two.";
        let ours = "# Section\n\nModified one.\n\nParagraph two.";
        let theirs = "# Section\n\nParagraph one.\n\nModified two.";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("Modified one."));
        assert!(merged.contains("Modified two."));
    }

    #[test]
    fn test_correctness_table_merge() {
        let driver = MarkdownDriver::new();
        let base = "# Data\n\n| Col1 | Col2 |\n| --- | --- |\n| a | b |";
        let ours = "# Data\n\n| Col1 | Col2 |\n| --- | --- |\n| x | b |";
        let theirs = "# Data\n\n| Col1 | Col2 |\n| --- | --- |\n| a | y |";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(
            result.is_none(),
            "table modifications at same position conflict"
        );
    }

    #[test]
    fn test_correctness_empty_document() {
        let driver = MarkdownDriver::new();
        let base = "";
        let ours = "# New\n\nContent.";
        let theirs = "# Other\n\nOther content.";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("New") || merged.contains("Other"));
    }
}
