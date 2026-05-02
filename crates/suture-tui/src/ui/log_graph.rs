//! Log graph computation — ASCII branch/merge visualization.

use crate::app::LogEntry;

pub struct GraphRow {
    pub commit_prefix: String,
    pub info_prefix: String,
    pub extra_lines: Vec<String>,
}

pub fn compute_graph(entries: &[LogEntry]) -> Vec<GraphRow> {
    let n = entries.len();
    if n == 0 {
        return Vec::new();
    }

    let mut result = Vec::new();

    for (i, entry) in entries.iter().enumerate() {
        let is_last = i == n - 1;

        let marker = if entry.is_merge { "\u{25c6}" } else { "\u{25cf}" };
        let connector = if is_last { "\u{2514}" } else { "\u{2502}" };

        let commit_prefix = format!("{connector} {marker} ");
        let info_prefix = if is_last {
            "    ".to_owned()
        } else {
            "  \u{2502} ".to_owned()
        };

        let mut extra_lines = Vec::new();

        if entry.is_merge {
            for (pi, parent_hash) in entry.parents.iter().enumerate().skip(1) {
                let short = if parent_hash.len() >= 12 {
                    format!("{}…", &parent_hash[..12])
                } else {
                    parent_hash.clone()
                };
                if is_last {
                    extra_lines.push(format!("    ├─ parent {}: {}", pi + 1, short));
                } else {
                    extra_lines.push(format!("  ├─ parent {}: {}", pi + 1, short));
                }
            }
            if !extra_lines.is_empty() && !is_last {
                extra_lines.push("  \u{2502}".to_owned());
            }
        }

        result.push(GraphRow {
            commit_prefix,
            info_prefix,
            extra_lines,
        });
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(id: &str, parents: Vec<&str>, is_merge: bool) -> LogEntry {
        LogEntry {
            id: id.to_string(),
            short_id: id.to_string(),
            author: "test".to_string(),
            message: "msg".to_string(),
            timestamp: "2024-01-01 00:00".to_string(),
            parents: parents.into_iter().map(String::from).collect(),
            branch_heads: Vec::new(),
            is_merge,
        }
    }

    #[test]
    fn test_empty_entries() {
        assert!(compute_graph(&[]).is_empty());
    }

    #[test]
    fn test_single_entry() {
        let entries = vec![make_entry("aaaaaaaa", vec![], false)];
        let graph = compute_graph(&entries);
        assert_eq!(graph.len(), 1);
        assert!(graph[0].commit_prefix.contains('└'));
        assert!(graph[0].commit_prefix.contains('●'));
    }

    #[test]
    fn test_linear_chain() {
        let entries = vec![
            make_entry("aaaaaaaa", vec!["bbbbbbbb"], false),
            make_entry("bbbbbbbb", vec![], false),
        ];
        let graph = compute_graph(&entries);
        assert_eq!(graph.len(), 2);
        assert!(graph[0].commit_prefix.contains('│'));
        assert!(graph[1].commit_prefix.contains('└'));
    }

    #[test]
    fn test_merge_entry() {
        let entries = vec![
            make_entry("aaaaaaaa", vec!["bbbbbbbb", "cccccccc"], true),
            make_entry("bbbbbbbb", vec![], false),
        ];
        let graph = compute_graph(&entries);
        assert_eq!(graph.len(), 2);
        assert!(graph[0].commit_prefix.contains('◆'));
        assert_eq!(graph[0].extra_lines.len(), 2);
    }

    #[test]
    fn test_last_merge_entry() {
        let entries = vec![make_entry("aaaaaaaa", vec!["bbbbbbbb", "cccccccc"], true)];
        let graph = compute_graph(&entries);
        assert_eq!(graph.len(), 1);
        assert!(graph[0].commit_prefix.contains('└'));
        assert!(graph[0].commit_prefix.contains('◆'));
        assert_eq!(graph[0].extra_lines.len(), 1);
    }
}
