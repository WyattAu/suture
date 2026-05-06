use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

pub struct PathTranslator {
    dirs: BTreeSet<String>,
    files: BTreeMap<String, String>,
}

impl PathTranslator {
    #[must_use]
    pub fn build(file_paths: &[&str]) -> Self {
        let mut dirs: BTreeSet<String> = BTreeSet::new();
        let mut files: BTreeMap<String, String> = BTreeMap::new();

        dirs.insert(String::new());

        for &path in file_paths {
            let parts: Vec<&str> = path.split('/').collect();
            let mut prefix = String::new();
            for (i, part) in parts.iter().enumerate() {
                if i > 0 {
                    prefix.push('/');
                }
                prefix.push_str(part);
                if i < parts.len() - 1 {
                    dirs.insert(prefix.clone());
                }
            }
            let file_name = parts
                .last()
                .map(std::string::ToString::to_string)
                .unwrap_or_default();
            files.insert(path.to_owned(), file_name);
        }

        Self { dirs, files }
    }

    #[must_use]
    pub fn list_dir(&self, dir_path: &str) -> Vec<DirEntry> {
        let mut entries = Vec::new();

        for d in &self.dirs {
            if d == dir_path {
                continue;
            }
            let parent = parent_of(d).unwrap_or_default();
            if parent == dir_path {
                let name = d
                    .rsplit('/')
                    .next()
                    .map(std::borrow::ToOwned::to_owned)
                    .unwrap_or_default();
                entries.push(DirEntry {
                    name,
                    path: d.clone(),
                    is_dir: true,
                });
            }
        }

        for path in self.files.keys() {
            let parent = parent_of(path).unwrap_or_default();
            if parent == dir_path {
                let name = path
                    .rsplit('/')
                    .next()
                    .map(std::borrow::ToOwned::to_owned)
                    .unwrap_or_default();
                entries.push(DirEntry {
                    name,
                    path: path.clone(),
                    is_dir: false,
                });
            }
        }

        entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        });

        entries
    }

    #[must_use]
    pub fn is_dir(&self, path: &str) -> bool {
        self.dirs.contains(path)
    }

    #[must_use]
    pub fn is_file(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }

    #[must_use]
    pub fn all_dirs(&self) -> &BTreeSet<String> {
        &self.dirs
    }

    #[must_use]
    pub fn all_files(&self) -> &BTreeMap<String, String> {
        &self.files
    }
}

fn parent_of(path: &str) -> Option<String> {
    let pos = path.rfind('/')?;
    Some(path[..pos].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_translation() {
        let paths = ["src/main.rs", "src/lib.rs", "README.md"];
        let t = PathTranslator::build(&paths);

        assert!(t.is_dir(""));
        assert!(t.is_dir("src"));
        assert!(!t.is_dir("README.md"));
        assert!(t.is_file("src/main.rs"));
        assert!(t.is_file("src/lib.rs"));
        assert!(t.is_file("README.md"));
        assert!(!t.is_file("src"));

        let root_entries = t.list_dir("");
        assert_eq!(root_entries.len(), 2);
        assert_eq!(root_entries[0].name, "src");
        assert!(root_entries[0].is_dir);
        assert_eq!(root_entries[0].name, "src");
        assert_eq!(root_entries[1].name, "README.md");
        assert!(!root_entries[1].is_dir);

        let src_entries = t.list_dir("src");
        assert_eq!(src_entries.len(), 2);
        assert_eq!(src_entries[0].name, "lib.rs");
        assert_eq!(src_entries[1].name, "main.rs");
    }

    #[test]
    fn test_path_translation_nested() {
        let paths = ["a/b/c/deep.rs", "a/b/shallow.rs", "top.rs", "x/y/z/w.rs"];
        let t = PathTranslator::build(&paths);

        let expected_dirs: Vec<String> = vec![
            "".into(),
            "a".into(),
            "a/b".into(),
            "a/b/c".into(),
            "x".into(),
            "x/y".into(),
            "x/y/z".into(),
        ];
        let actual_dirs: Vec<String> = t.all_dirs().iter().cloned().collect();
        assert_eq!(actual_dirs, expected_dirs);

        assert_eq!(t.list_dir("a").len(), 1);
        assert_eq!(t.list_dir("a/b").len(), 2);
        assert_eq!(t.list_dir("a/b/c").len(), 1);
        assert_eq!(t.list_dir("x").len(), 1);
        assert_eq!(t.list_dir("x/y").len(), 1);
        assert_eq!(t.list_dir("x/y/z").len(), 1);
    }

    #[test]
    fn test_path_translation_empty() {
        let t = PathTranslator::build(&[]);
        assert!(t.is_dir(""));
        assert!(t.list_dir("").is_empty());
    }

    #[test]
    fn test_path_translation_single_file() {
        let t = PathTranslator::build(&["file.txt"]);
        assert!(t.is_file("file.txt"));
        assert!(t.is_dir(""));
        assert_eq!(t.list_dir("").len(), 1);
        assert_eq!(t.list_dir("").first().unwrap().name, "file.txt");
    }
}
