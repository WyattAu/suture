use std::path::Path;

pub fn safe_path(repo_root: &Path, path: &Path) -> Option<std::path::PathBuf> {
    let full = repo_root.join(path);
    let resolved = std::fs::canonicalize(&full).ok()?;
    let root = std::fs::canonicalize(repo_root).ok()?;
    if resolved.starts_with(&root) {
        Some(resolved)
    } else {
        None
    }
}

pub fn is_path_within_repo(repo_root: &Path, path: &Path) -> bool {
    for component in path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return false;
        }
    }
    let full = repo_root.join(path);
    if let (Ok(resolved), Ok(root)) =
        (std::fs::canonicalize(&full), std::fs::canonicalize(repo_root))
    {
        return resolved.starts_with(&root);
    }
    !path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
}
