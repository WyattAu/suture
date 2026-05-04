use crate::ref_utils::resolve_ref;
use std::path::Path;

struct ExportContext<'a> {
    date: &'a str,
    version: &'a str,
    branch: &'a str,
    client: &'a str,
}

pub async fn cmd_export(
    output: &str,
    at: Option<&str>,
    zip: bool,
    template: Option<&str>,
    include_meta: bool,
    client: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let repo = suture_core::repository::Repository::open(Path::new("."))?;
    let patches = repo.all_patches();

    let ref_str = at.unwrap_or("HEAD");
    let tree = {
        let patch = resolve_ref(&repo, ref_str, &patches)?;
        repo.snapshot(&patch.id)?
    };

    let effective_output =
        client.as_ref().map_or_else(|| output.to_owned(), |name| format!("{output}/{name}"));

    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let version = ref_str;
    let branch_name = repo.head().ok().map(|(b, _)| b).unwrap_or_default();
    let client_name = client.unwrap_or("");

    let ctx = ExportContext {
        date: &date,
        version,
        branch: &branch_name,
        client: client_name,
    };

    if zip {
        export_as_zip(
            &repo,
            &tree,
            &effective_output,
            template,
            include_meta,
            &ctx,
        )?;
    } else {
        export_as_dir(
            &repo,
            &tree,
            &effective_output,
            template,
            include_meta,
            &ctx,
        )?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn export_as_dir(
    repo: &suture_core::repository::Repository,
    tree: &suture_core::engine::tree::FileTree,
    destination: &str,
    template: Option<&str>,
    include_meta: bool,
    ctx: &ExportContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let dest = Path::new(destination);
    if dest.exists() {
        let err_msg = format!("destination '{destination}' already exists");
        return Err(err_msg.into());
    }
    std::fs::create_dir_all(dest)?;

    let mut file_count = 0usize;
    for (path, hash) in tree.iter() {
        if path.starts_with(".suture/") && !include_meta {
            continue;
        }
        if path.contains("..") {
            continue;
        }
        let full_path = dest.join(path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = repo.cas().get_blob(hash).map_err(|e| e.to_string())?;
        std::fs::write(&full_path, data)?;
        file_count += 1;
    }

    if let Some(template_dir) = template {
        let tpl_path = Path::new(template_dir);
        if tpl_path.exists() {
            let tpl_count = copy_template_dir(tpl_path, dest, ctx)?;
            file_count += tpl_count;
        }
    }

    println!("Exported {file_count} files to {destination}");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn export_as_zip(
    repo: &suture_core::repository::Repository,
    tree: &suture_core::engine::tree::FileTree,
    output: &str,
    template: Option<&str>,
    include_meta: bool,
    ctx: &ExportContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir().join(format!("suture_export_{}", std::process::id()));

    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir)?;
    }
    std::fs::create_dir_all(&temp_dir)?;

    let mut file_count = 0usize;
    for (path, hash) in tree.iter() {
        if path.starts_with(".suture/") && !include_meta {
            continue;
        }
        if path.contains("..") {
            continue;
        }
        let data = repo.cas().get_blob(hash).map_err(|e| e.to_string())?;
        let full_path = temp_dir.join(path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&full_path, data)?;
        file_count += 1;
    }

    if let Some(template_dir) = template {
        let tpl_path = Path::new(template_dir);
        if tpl_path.exists() {
            let tpl_count = copy_template_dir(tpl_path, &temp_dir, ctx)?;
            file_count += tpl_count;
        }
    }

    let status = std::process::Command::new("zip")
        .arg("-r")
        .arg(output)
        .arg(".")
        .current_dir(&temp_dir)
        .status()?;

    let _ = std::fs::remove_dir_all(&temp_dir);

    if !status.success() {
        return Err("zip command failed".into());
    }

    println!("Exported {file_count} files to {output}");
    Ok(())
}

fn copy_template_dir(
    template_dir: &Path,
    dest: &Path,
    ctx: &ExportContext,
) -> Result<usize, Box<dyn std::error::Error>> {
    let mut count = 0usize;
    copy_template_recursive(template_dir, dest, ctx, &mut count)?;
    Ok(count)
}

fn copy_template_recursive(
    src: &Path,
    dest: &Path,
    ctx: &ExportContext,
    count: &mut usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let entries: Vec<_> = std::fs::read_dir(src)?.filter_map(std::result::Result::ok).collect();

    for entry in &entries {
        let src_path = entry.path();
        let file_name = entry.file_name();
        let dest_path = dest.join(&file_name);

        if src_path.is_dir() {
            std::fs::create_dir_all(&dest_path)?;
            copy_template_recursive(&src_path, &dest_path, ctx, count)?;
        } else if src_path.is_file() {
            let mut content = std::fs::read_to_string(&src_path).unwrap_or_default();
            content = content.replace("{date}", ctx.date);
            content = content.replace("{version}", ctx.version);
            content = content.replace("{branch}", ctx.branch);
            content = content.replace("{client}", ctx.client);

            let name_str = file_name.to_string_lossy();
            let renamed = name_str
                .replace("{date}", ctx.date)
                .replace("{version}", ctx.version)
                .replace("{branch}", ctx.branch)
                .replace("{client}", ctx.client);
            let final_path = dest.join(&renamed);

            if let Some(parent) = final_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&final_path, content)?;
            *count += 1;
        }
    }

    Ok(())
}
