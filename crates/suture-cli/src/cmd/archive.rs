use crate::ref_utils::resolve_ref;
use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs::File;
use std::path::Path;

pub(crate) async fn cmd_archive(
    commit: Option<&str>,
    output: &str,
    format: Option<&str>,
    prefix: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(Path::new("."))?;
    let patches = repo.all_patches();

    let tree = match commit {
        Some(ref_str) => {
            let patch = resolve_ref(&repo, ref_str, &patches)?;
            repo.snapshot(&patch.id)?
        }
        None => repo.snapshot_head()?,
    };

    let repo_name = repo
        .root()
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("suture-archive");
    let prefix_str = prefix.unwrap_or(repo_name);

    let fmt = match format {
        Some(f) => f,
        None => detect_format(output),
    };

    match fmt {
        "tar" => write_tar(&repo, &tree, output, prefix_str)?,
        "tar.gz" | "targz" | "tgz" => write_tar_gz(&repo, &tree, output, prefix_str)?,
        "zip" => write_zip(&repo, &tree, output, prefix_str)?,
        other => {
            return Err(format!(
                "unsupported archive format: '{}' (supported: tar, tar.gz, tgz, zip)",
                other
            )
            .into());
        }
    }

    let file_count = tree
        .iter()
        .filter(|(p, _)| !p.starts_with(".suture/"))
        .count();
    println!("Archived {} files to {}", file_count, output);
    Ok(())
}

fn detect_format(output: &str) -> &'static str {
    if output.ends_with(".tar.gz") || output.ends_with(".tgz") {
        "tar.gz"
    } else if output.ends_with(".tar") {
        "tar"
    } else if output.ends_with(".zip") {
        "zip"
    } else {
        "tar.gz"
    }
}

fn collect_entries<'a>(
    repo: &'a suture_core::repository::Repository,
    tree: &'a suture_core::engine::tree::FileTree,
) -> Vec<(String, std::io::Result<Vec<u8>>)> {
    let mut entries: Vec<(String, std::io::Result<Vec<u8>>)> = Vec::new();
    for (path, hash) in tree.iter() {
        if path.starts_with(".suture/") {
            continue;
        }
        let data = repo
            .cas()
            .get_blob(hash)
            .map_err(|e| std::io::Error::other(e.to_string()));
        entries.push((path.clone(), data));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

fn write_tar(
    repo: &suture_core::repository::Repository,
    tree: &suture_core::engine::tree::FileTree,
    output: &str,
    prefix: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create(output)?;
    let mut builder = tar::Builder::new(file);

    let entries = collect_entries(repo, tree);
    for (path, data_result) in &entries {
        let data = data_result.as_ref().map_err(|e| e.to_string())?;
        let archive_path = format!("{}/{}", prefix, path);
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, &archive_path, data.as_slice())?;
    }

    builder.into_inner()?;
    Ok(())
}

fn write_tar_gz(
    repo: &suture_core::repository::Repository,
    tree: &suture_core::engine::tree::FileTree,
    output: &str,
    prefix: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create(output)?;
    let encoder = GzEncoder::new(file, Compression::default());
    let mut builder = tar::Builder::new(encoder);

    let entries = collect_entries(repo, tree);
    for (path, data_result) in &entries {
        let data = data_result.as_ref().map_err(|e| e.to_string())?;
        let archive_path = format!("{}/{}", prefix, path);
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, &archive_path, data.as_slice())?;
    }

    let encoder = builder.into_inner()?;
    encoder.finish()?;
    Ok(())
}

fn write_zip(
    repo: &suture_core::repository::Repository,
    tree: &suture_core::engine::tree::FileTree,
    output: &str,
    prefix: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir().join(format!("suture_archive_{}", std::process::id()));
    let prefix_dir = temp_dir.join(prefix);

    if prefix_dir.exists() {
        std::fs::remove_dir_all(&prefix_dir)?;
    }
    std::fs::create_dir_all(&prefix_dir)?;

    let entries = collect_entries(repo, tree);
    for (path, data_result) in &entries {
        let data = data_result.as_ref().map_err(|e| e.to_string())?;
        let full_path = prefix_dir.join(path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&full_path, data)?;
    }

    let status = std::process::Command::new("zip")
        .arg("-r")
        .arg(output)
        .arg(prefix)
        .current_dir(&temp_dir)
        .status()?;

    let _ = std::fs::remove_dir_all(&temp_dir);

    if !status.success() {
        return Err("zip command failed".into());
    }
    Ok(())
}
